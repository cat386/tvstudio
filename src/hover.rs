//! Mouse observation and non-focusable LSP hover rendering.
use crate::codeedit::CodeEditor;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
use tvision::{
    platform::{Backend, RawEvent},
    *,
};

#[derive(Clone, Debug)]
pub struct Pointer {
    pub pos: Option<Point>,
    pub generation: u64,
    pub since: Instant,
}
impl Default for Pointer {
    fn default() -> Self {
        Self { pos: None, generation: 0, since: Instant::now() }
    }
}
pub type SharedPointer = Rc<RefCell<Pointer>>;
pub struct HoverBackend {
    inner: Box<dyn Backend>,
    pointer: SharedPointer,
}
impl HoverBackend {
    pub fn new(inner: Box<dyn Backend>, pointer: SharedPointer) -> Self {
        Self { inner, pointer }
    }
}
impl Backend for HoverBackend {
    fn size(&self) -> Point {
        self.inner.size()
    }
    fn read(&mut self, timeout: Duration) -> Option<RawEvent> {
        let ev = self.inner.read(timeout);
        if let Some(ev) = &ev {
            let pos = match ev {
                RawEvent::Mouse { pos, buttons: 0, wheel: 0, .. } => Some(*pos),
                _ => None,
            };
            let mut p = self.pointer.borrow_mut();
            if pos != p.pos || pos.is_none() {
                p.pos = pos;
                p.generation = p.generation.wrapping_add(1);
                p.since = Instant::now();
            }
        }
        ev
    }
    fn flush(&mut self, s: &ScreenBuffer, c: Option<Point>, i: bool) {
        self.inner.flush(s, c, i);
    }
    fn suspend(&mut self) {
        let mut p = self.pointer.borrow_mut();
        p.pos = None;
        p.generation = p.generation.wrapping_add(1);
        self.inner.suspend();
    }
    fn resume(&mut self) {
        self.inner.resume();
    }
    fn system_clipboard(&self) -> bool {
        self.inner.system_clipboard()
    }
    fn last_frame(&self) -> Option<&ScreenBuffer> {
        self.inner.last_frame()
    }
}

/// Only the topmost view beneath the pointer can supply hover information.
pub fn hit(group: &Group, point: Point) -> Option<(ViewId, Pos, Point)> {
    let child = group.children.iter().rev().find(|v| v.vs().is_visible() && v.vs().bounds().contains(point))?;
    let local = point - child.vs().origin;
    if let Some(ed) = child.as_any().downcast_ref::<CodeEditor>() {
        return symbol_at(ed, local).map(|at| (ed.vs().id, at, local));
    }
    hit(child.as_group()?, local)
}

pub fn symbol_at(ed: &CodeEditor, local: Point) -> Option<Pos> {
    let e = &ed.fe.editor;
    if local.x < 0 || local.y < 0 || local.x >= e.vs.size.x || local.y >= e.vs.size.y {
        return None;
    }
    let line = (local.y + e.delta.y) as usize;
    if line >= e.line_count() {
        return None;
    }
    let x = (local.x + e.delta.x) as usize;
    let mut left = 0;
    for (col, ch) in e.line(line).chars().enumerate() {
        let width = if ch == '\t' {
            tvision::widgets::editor::TAB_SIZE - left % tvision::widgets::editor::TAB_SIZE
        } else {
            tvision::text::char_width(ch).max(1)
        };
        if x < left + width {
            return (ch.is_alphanumeric() || ch == '_').then_some(Pos::new(line, col));
        }
        left += width;
    }
    None
}

pub struct Popup {
    pub pointer: SharedPointer,
    pub generation: u64,
    pub at: Point,
    pub text: String,
}
impl Popup {
    pub fn rows(&self, size: Point) -> Vec<(i32, i32, String, bool)> {
        if self.pointer.borrow().generation != self.generation || size.x < 6 || size.y < 3 {
            return Vec::new();
        }
        let width = (size.x - 2).min(76) as usize;
        let mut lines = Vec::new();
        for line in self.text.lines().filter(|l| !l.trim_start().starts_with("```")) {
            let mut row = String::new();
            let mut cols = 0;
            for ch in line.chars().map(|c| if c.is_control() { ' ' } else { c }) {
                let n = tvision::text::char_width(ch).max(1);
                if cols + n > width {
                    lines.push(row);
                    row = String::new();
                    cols = 0;
                }
                row.push(ch);
                cols += n;
            }
            lines.push(row);
        }
        let limit = (size.y - 2).min(12) as usize;
        if lines.len() > limit {
            lines.truncate(limit);
            lines[limit - 1] = "…".into();
        }
        if lines.is_empty() {
            return Vec::new();
        }
        let height = lines.len() as i32 + 2;
        let x = self.at.x.min(size.x - width as i32 - 2).max(0);
        let y = if self.at.y + height < size.y { self.at.y + 1 } else { (self.at.y - height).max(0) };
        let mut result = vec![(y, x, format!("┌{}┐", "─".repeat(width)), false)];
        for (i, line) in lines.iter().enumerate() {
            let used: usize = line.chars().map(|c| tvision::text::char_width(c).max(1)).sum();
            result.push((y + 1 + i as i32, x, format!("│{}{}│", line, " ".repeat(width.saturating_sub(used))), false));
        }
        result.push((y + height - 1, x, format!("└{}┘", "─".repeat(width)), false));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{json, lsp::parse_hover};
    use tvision::platform::HeadlessBackend;

    #[test]
    fn hover_contents_variants() {
        for (input, expected) in [
            (r#"{"contents":{"kind":"markdown","value":"fn foo()\nDocs"}}"#, "fn foo()\nDocs"),
            (r#"{"contents":[{"language":"rust","value":"fn foo()"},"Docs"]}"#, "fn foo()\n\nDocs"),
            (r#"{"contents":"Docs"}"#, "Docs"),
            ("null", ""),
            (r#"{"contents":[]}"#, ""),
        ] {
            assert_eq!(parse_hover(&json::parse(input).unwrap()), expected);
        }
    }

    #[test]
    fn hover_symbol_hit_test_handles_tabs_unicode_and_scroll() {
        let mut ed = CodeEditor::new(Rect::new(0, 0, 30, 8), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("\t变量 foo\n\nbar", false);
        let tab = tvision::widgets::editor::TAB_SIZE as i32;
        assert_eq!(symbol_at(&ed, Point::new(0, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(tab + 1, 0)), Some(Pos::new(0, 1)));
        assert_eq!(symbol_at(&ed, Point::new(tab + 4, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(25, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(0, 1)), None);
        assert_eq!(symbol_at(&ed, Point::new(0, 5)), None);
        ed.fe.editor.delta = Point::new(tab + 5, 0);
        assert_eq!(symbol_at(&ed, Point::new(0, 0)), Some(Pos::new(0, 4)));
        ed.fe.editor.delta = Point::new(0, 2);
        assert_eq!(symbol_at(&ed, Point::new(1, 0)), Some(Pos::new(2, 1)));
    }

    #[test]
    fn hover_popup_fits_and_hides_on_input() {
        let shared = SharedPointer::default();
        let popup = Popup {
            pointer: shared.clone(),
            generation: 0,
            at: Point::new(19, 5),
            text: format!("\x1b[31m{}\n{}", "变量".repeat(30), "docs\n".repeat(30)),
        };
        let rows = popup.rows(Point::new(20, 6));
        assert!(!rows.is_empty());
        for (y, x, line, _) in rows {
            assert!((0..6).contains(&y));
            assert!(x >= 0);
            assert!(!line.chars().any(char::is_control));
            assert!(x as usize + line.chars().map(|c| tvision::text::char_width(c).max(1)).sum::<usize>() <= 20);
        }
        let mut backend = HeadlessBackend::new(20, 6);
        backend.push_key(KeyEvent::new(Key::Char('a'), Mods::NONE));
        let mut backend = HoverBackend::new(Box::new(backend), shared);
        assert!(matches!(backend.read(Duration::ZERO), Some(RawEvent::Key(_))));
        assert!(popup.rows(Point::new(20, 6)).is_empty());
    }

    #[test]
    fn hover_backend_tracks_motion_without_changing_events() {
        let pointer = SharedPointer::default();
        let event = RawEvent::Mouse { pos: Point::new(8, 9), buttons: 0, mods: Mods::NONE, wheel: 0 };
        let mut backend = HeadlessBackend::new(20, 20);
        backend.input.push_back(event.clone());
        backend.input.push_back(event.clone());
        backend.input.push_back(RawEvent::Resize(Point::new(40, 20)));
        let mut backend = HoverBackend::new(Box::new(backend), pointer.clone());
        assert_eq!(backend.read(Duration::ZERO), Some(event.clone()));
        let generation = pointer.borrow().generation;
        assert_eq!(pointer.borrow().pos, Some(Point::new(8, 9)));
        assert_eq!(backend.read(Duration::ZERO), Some(event));
        assert_eq!(pointer.borrow().generation, generation);
        backend.read(Duration::ZERO);
        assert!(pointer.borrow().pos.is_none());
        assert_ne!(pointer.borrow().generation, generation);
    }
}
