//! Просмотр текстового файла (`fileview.cpp`): `TFileViewer` и `TFileWindow`.

use std::path::{Path, PathBuf};

use tvision::*;

/// Просмотрщик: строки файла с прокруткой (`TFileViewer` на основе `TScroller`).
pub struct FileViewer {
    pub vs: ViewState,
    pub scroller: Scroller,
    pub lines: Vec<String>,
    pub file_name: PathBuf,
    /// Файл прочитан (`isValid`).
    pub is_valid: bool,
    /// Текст ошибки чтения — покажем, когда окно попробуют вставить.
    pub error: Option<String>,
}

impl FileViewer {
    pub fn new(r: Rect, h: Option<ViewId>, v: Option<ViewId>, file: &Path) -> Self {
        let mut vs = ViewState::new(r);
        vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        vs.options |= of::SELECTABLE;
        let mut fv = FileViewer { vs, scroller: Scroller::new(h, v), lines: Vec::new(), file_name: file.to_path_buf(), is_valid: true, error: None };
        fv.read_file(file);
        fv
    }

    /// Чтение файла (`readFile`): строки по `\n`, `\r\n` или `\r`, нулевые байты — пробелы.
    pub fn read_file(&mut self, file: &Path) {
        self.file_name = file.to_path_buf();
        match std::fs::read(file) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).replace('\0', " ").replace("\r\n", "\n").replace('\r', "\n");
                self.lines = text.split('\n').map(|s| s.replace('\t', "        ")).collect();
                if self.lines.last().map(|l| l.is_empty()).unwrap_or(false) {
                    self.lines.pop();
                }
                self.is_valid = true;
            }
            Err(e) => {
                self.error = Some(format!("Failed to open file '{}'.\n{}", file.display(), e));
                self.is_valid = false;
            }
        }
        let w = self.lines.iter().map(|l| text::str_width(l)).max().unwrap_or(0) as i32;
        let size = self.vs.size;
        self.scroller.set_limit(w, self.lines.len() as i32, size);
    }
}

impl View for FileViewer {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let c = dc.color(1);
        let w = self.vs.size.x.max(0) as usize;
        let d = self.scroller.delta;
        for i in 0..self.vs.size.y {
            let mut b = DrawBuffer::new(w);
            b.move_char(0, ' ', c, w);
            if let Some(l) = self.lines.get((d.y + i) as usize) {
                let s: String = l.chars().skip(d.x.max(0) as usize).collect();
                b.move_str_limited(0, &s, c, w);
            }
            dc.write_line(0, i, self.vs.size.x, 1, &b);
        }
    }

    fn palette(&self) -> &'static [u8] {
        color::palettes::SCROLLER
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        tvision::view::default_handle_event(&self.vs, ev, ctx);
        let size = self.vs.size;
        let focused = self.vs.is_focused();
        // Ctrl+PgUp / Ctrl+PgDn — начало и конец файла.
        if let Event::KeyDown(k) = ev {
            if focused && k.mods == Mods::CTRL {
                let x = self.scroller.delta.x;
                match k.key {
                    Key::PgUp => self.scroller.scroll_to(x, 0, size),
                    Key::PgDn => self.scroller.scroll_to(x, i32::MAX / 2, size),
                    _ => {}
                }
            }
        }
        self.scroller.handle(ev, size, focused);
        self.scroller.sync(size, ctx);
    }

    fn change_bounds(&mut self, bounds: Rect) {
        self.vs.set_bounds(bounds);
        let l = self.scroller.limit;
        self.scroller.set_limit(l.x, l.y, bounds.size());
    }

    fn valid(&mut self, _command: u16, _ctx: &mut Ctx) -> bool {
        self.is_valid
    }
}

/// Окно просмотра файла (`TFileWindow`) на весь рабочий стол.
pub fn file_window(desktop: Rect, file: &Path, number: i16) -> Window {
    let mut w = Window::new(desktop, &file.display().to_string(), number);
    w.group.vs.options |= of::TILEABLE;
    let h = w.standard_scroll_bar(false);
    let v = w.standard_scroll_bar(true);
    let r = w.interior();
    w.add(FileViewer::new(r, Some(h), Some(v), file));
    w
}
