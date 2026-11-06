//! Редактор кода: окно с номерами строк, подсветкой синтаксиса,
//! пометками ошибок сборки и переходом к строке.
//!
//! Правка текста целиком делегируется `FileEditor` из tvision-rs
//! (клавиши WordStar, отмена, поиск и замена, буфер обмена, вопрос о
//! сохранении). Своя здесь только отрисовка: тот же текст, но в цвете.

use std::path::{Path, PathBuf};

use tvision::color::{is_bg256, is_fg256, with_fg256, with_fore};
use tvision::widgets::editor::TAB_SIZE;
use tvision::*;

use crate::cmds::*;
use crate::highlight::{highlight_line, Class, Lang, State};
use crate::toolwins::MsgKind;

/// Ширина полосы номеров строк.
pub const GUTTER_W: i32 = 7;

/// Цвет класса лексемы на фоне обычного текста `normal`.
pub fn class_attr(normal: Attr, c: Class) -> Attr {
    // 256-цветная тема (gruvbox) — цвета gruvbox; иначе — 16 цветов BIOS.
    if is_bg256(normal) || is_fg256(normal) {
        let fg = match c {
            Class::Normal | Class::Punct => return normal,
            Class::Keyword => 167,
            Class::Type => 214,
            Class::Number => 175,
            Class::Str => 142,
            Class::Comment => 245,
            Class::Macro => 108,
            Class::Attr => 109,
            Class::Lifetime => 208,
        };
        with_fg256(normal, fg)
    } else {
        let fg = match c {
            Class::Normal => return normal,
            Class::Punct => return normal,
            Class::Keyword => 15,
            Class::Type => 11,
            Class::Number => 13,
            Class::Str => 10,
            Class::Comment => 7,
            Class::Macro => 12,
            Class::Attr => 3,
            Class::Lifetime => 13,
        };
        with_fore(normal, fg)
    }
}

// ======================================================================
// Редактор с подсветкой
// ======================================================================

/// `FileEditor` с подсветкой синтаксиса.
pub struct CodeEditor {
    pub fe: FileEditor,
    pub lang: Lang,
}

impl CodeEditor {
    pub fn new(bounds: Rect, h: Option<ViewId>, v: Option<ViewId>, ind: Option<ViewId>, path: &Path) -> Self {
        let mut fe = FileEditor::new(bounds, h, v, ind, path);
        fe.editor.vs.help_ctx = HC_EDITOR;
        // Отступы в Rust — 4 пробела; табуляции в файле тоже показываются.
        CodeEditor { fe, lang: Lang::from_path(path) }
    }
}

impl View for CodeEditor {
    fn vs(&self) -> &ViewState {
        &self.fe.editor.vs
    }
    fn vs_mut(&mut self) -> &mut ViewState {
        &mut self.fe.editor.vs
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn draw(&self, dc: &mut DrawCtx) {
        let ed = &self.fe.editor;
        let normal = dc.color(1);
        let selected = dc.color(2);
        let size = ed.vs.size;
        let w = size.x.max(0) as usize;
        let sel = ed.selection();
        let n = ed.line_count();
        let top = ed.delta.y.max(0) as usize;
        let dx = ed.delta.x.max(0) as usize;
        // Состояние разбора на первой видимой строке.
        let mut state = State::Normal;
        if self.lang != Lang::Plain {
            for li in 0..top.min(n) {
                let l: Vec<char> = ed.line(li).chars().collect();
                highlight_line(&l, &mut state, self.lang);
            }
        }
        for row in 0..size.y {
            let mut b = DrawBuffer::new(w);
            b.move_char(0, ' ', normal, w);
            let li = top + row as usize;
            if li < n {
                let line: Vec<char> = ed.line(li).chars().collect();
                let classes = highlight_line(&line, &mut state, self.lang);
                let in_sel = |col: usize| sel.map(|(a, bb)| Pos::new(li, col) >= a && Pos::new(li, col) < bb).unwrap_or(false);
                let mut x = 0usize;
                for (ci, &c) in line.iter().enumerate() {
                    let cw = if c == '\t' { TAB_SIZE - x % TAB_SIZE } else { tvision::text::char_width(c).max(1) };
                    let attr = if in_sel(ci) { selected } else { class_attr(normal, classes.get(ci).copied().unwrap_or(Class::Normal)) };
                    let vis_a = x.max(dx);
                    let vis_b = (x + cw).min(dx + w);
                    if vis_a < vis_b {
                        if c == '\t' || vis_b - vis_a < cw {
                            b.move_char(vis_a - dx, ' ', attr, vis_b - vis_a);
                        } else {
                            let mut tmp = [0u8; 4];
                            b.move_str(x - dx, c.encode_utf8(&mut tmp), attr);
                        }
                    }
                    x += cw;
                    if x >= dx + w {
                        break;
                    }
                }
                if let Some((a, bb)) = sel {
                    let eol = Pos::new(li, line.len());
                    if eol >= a && eol < bb && x >= dx && x - dx < w {
                        b.move_char(x - dx, ' ', selected, 1);
                    }
                }
            }
            dc.write_line(0, row, size.x, 1, &b);
        }
    }

    fn palette(&self) -> &'static [u8] {
        self.fe.editor.palette
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        // Tab в коде — четыре пробела до ближайшей кратной позиции.
        if let Event::KeyDown(k) = ev {
            if k.key == Key::Tab && k.mods == Mods::NONE && self.fe.editor.vs.is_focused() && self.fe.editor.selection().is_none() {
                let col = self.fe.editor.cursor().col;
                let n = 4 - col % 4;
                self.fe.editor.insert_text(&" ".repeat(n), false);
                self.fe.editor.track_cursor(false);
                ev.clear();
                // Обновить полосы и индикатор — обработкой «пустого» события.
                let mut nothing = Event::Broadcast(Message::new(0xFFFE));
                self.fe.handle_event(&mut nothing, ctx);
                return;
            }
        }
        self.fe.handle_event(ev, ctx);
    }

    fn exec_result(&mut self, tag: u16, command: u16, view: Box<dyn View>, ctx: &mut Ctx) {
        self.fe.exec_result(tag, command, view, ctx);
    }

    fn update_commands(&self, ctx: &mut Ctx) {
        self.fe.update_commands(ctx);
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.fe.valid(command, ctx)
    }

    fn change_bounds(&mut self, bounds: Rect) {
        self.fe.change_bounds(bounds);
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.fe.set_state(flag, enable);
    }
}

// ======================================================================
// Номера строк
// ======================================================================

/// Полоса номеров строк слева от текста; помечает строки с ошибками.
pub struct Gutter {
    pub vs: ViewState,
    pub top: usize,
    pub count: usize,
    pub cur: usize,
    /// Пометки: (номер строки с 0, вид).
    pub marks: Vec<(usize, MsgKind)>,
}

impl Gutter {
    pub fn new(bounds: Rect) -> Self {
        let mut vs = ViewState::new(bounds);
        vs.grow_mode = gf::GROW_HI_Y;
        Gutter { vs, top: 0, count: 1, cur: 0, marks: Vec::new() }
    }
}

impl View for Gutter {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let base = dc.color(6);
        let wide = is_bg256(base) || is_fg256(base);
        let dim = if wide { with_fg256(base, 243) } else { with_fore(base, 8) };
        let cur = if wide { with_fg256(base, 214) } else { with_fore(base, 14) };
        let err = if wide { with_fg256(base, 167) } else { with_fore(base, 12) };
        let warn = if wide { with_fg256(base, 214) } else { with_fore(base, 14) };
        let w = self.vs.size.x.max(1) as usize;
        for row in 0..self.vs.size.y {
            let li = self.top + row as usize;
            let mut b = DrawBuffer::new(w);
            b.move_char(0, ' ', dim, w);
            if li < self.count {
                let s = format!("{:>width$}", li + 1, width = w.saturating_sub(2));
                b.move_str(1, &s, if li == self.cur { cur } else { dim });
                if let Some((_, k)) = self.marks.iter().find(|(l, _)| *l == li) {
                    match k {
                        MsgKind::Error => b.move_str(0, "●", err),
                        _ => b.move_str(0, "▲", warn),
                    };
                }
            }
            b.move_char(w - 1, '│', dim, 1);
            dc.write_line(0, row, self.vs.size.x, 1, &b);
        }
    }
}

// ======================================================================
// Окно редактора кода
// ======================================================================

/// Окно редактора кода.
pub struct CodeWindow {
    pub win: Window,
    pub editor_id: ViewId,
    pub gutter_id: ViewId,
    /// Каталог проекта — заголовок окна показывается относительно него.
    pub base: Option<PathBuf>,
}

impl CodeWindow {
    pub fn new(bounds: Rect, path: &Path, number: i16, base: Option<PathBuf>) -> Self {
        let mut win = Window::new(bounds, "", number);
        win.group.vs.options |= of::TILEABLE;
        let size = win.group.vs.size;
        let hsb = win.add(ScrollBar::new(Rect::new(18, size.y - 1, size.x - 2, size.y)));
        let vsb = win.standard_scroll_bar(true);
        let ind = win.add(Indicator::new(Rect::new(2, size.y - 1, 16, size.y)));
        let gutter_id = win.add(Gutter::new(Rect::new(1, 1, 1 + GUTTER_W, size.y - 1)));
        let ed = CodeEditor::new(Rect::new(1 + GUTTER_W, 1, size.x - 1, size.y - 1), Some(hsb), Some(vsb), Some(ind), path);
        let editor_id = win.add(ed);
        let mut w = CodeWindow { win, editor_id, gutter_id, base };
        w.sync();
        w
    }

    pub fn editor(&self) -> &CodeEditor {
        self.win.group.find::<CodeEditor>(self.editor_id).unwrap()
    }

    pub fn editor_mut(&mut self) -> &mut CodeEditor {
        let id = self.editor_id;
        self.win.group.find_mut::<CodeEditor>(id).unwrap()
    }

    /// Путь файла (пустой — безымянный).
    pub fn path(&self) -> PathBuf {
        self.editor().fe.file_name.clone()
    }

    /// Перейти к строке и столбцу (нумерация с 1).
    pub fn goto(&mut self, line: usize, col: usize) {
        let e = &mut self.editor_mut().fe.editor;
        let p = Pos::new(line.saturating_sub(1), col.saturating_sub(1));
        e.set_select(p, p);
        e.track_cursor(true);
        self.sync();
    }

    /// Пометки ошибок и предупреждений.
    pub fn set_marks(&mut self, marks: Vec<(usize, MsgKind)>) {
        let id = self.gutter_id;
        if let Some(g) = self.win.group.find_mut::<Gutter>(id) {
            g.marks = marks;
        }
    }

    /// Обновить полосу номеров и заголовок.
    fn sync(&mut self) {
        let (top, count, cur, title) = {
            let ce = self.editor();
            let e = &ce.fe.editor;
            let path = &ce.fe.file_name;
            let title = if path.as_os_str().is_empty() {
                "Без имени".to_string()
            } else {
                match &self.base {
                    Some(b) => path.strip_prefix(b).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string()),
                    None => path.display().to_string(),
                }
            };
            (e.delta.y.max(0) as usize, e.line_count(), e.cursor().line, title)
        };
        let gid = self.gutter_id;
        if let Some(g) = self.win.group.find_mut::<Gutter>(gid) {
            g.top = top;
            g.count = count;
            g.cur = cur;
        }
        if self.win.title != title {
            self.win.title = title;
        }
    }
}

impl View for CodeWindow {
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        self.win.handle_window_event(ev, ctx);
        self.sync();
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.win.group.valid(command, ctx)
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        (Point::new(24, 6), owner_size)
    }
}
