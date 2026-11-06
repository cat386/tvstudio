//! Таблица символов (`ascii.cpp`): `TTable`, `TReport`, `TAsciiChart`.
//!
//! Оригинал показывал 256 символов кодовой страницы CP437. Терминал теперь
//! работает в Unicode, поэтому символы переводятся таблицей CP437 → Unicode.

use tvision::*;

/// Кодовая страница 437 в Unicode (символы 0–31 — «графические», как на экране DOS).
pub const CP437: [char; 256] = [
    ' ', '☺', '☻', '♥', '♦', '♣', '♠', '•', '◘', '○', '◙', '♂', '♀', '♪', '♫', '☼', //
    '►', '◄', '↕', '‼', '¶', '§', '▬', '↨', '↑', '↓', '→', '←', '∟', '↔', '▲', '▼', //
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', //
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', //
    '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', //
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~', '⌂', //
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', ' ', //
];

/// Таблица 32×8 символов (`TTable`). Курсор показывает выбранный символ.
pub struct Table {
    pub vs: ViewState,
    /// Последний выбранный символ (для «отчёта» внизу окна).
    pub focused_char: u8,
}

impl Table {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.options |= of::SELECTABLE | of::FRAMED;
        vs.state |= sf::CURSOR_VIS | sf::CURSOR_INS; // `blockCursor()`
        Table { vs, focused_char: 0 }
    }

    /// Курсор переместился (`charFocused`).
    fn char_focused(&mut self) {
        self.focused_char = (self.vs.cursor.x + 32 * self.vs.cursor.y).clamp(0, 255) as u8;
    }
}

impl View for Table {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let color = dc.color(6);
        let w = self.vs.size.x.max(0) as usize;
        for y in 0..self.vs.size.y {
            let mut b = DrawBuffer::new(w);
            b.move_char(0, ' ', color, w);
            for x in 0..w {
                let code = (32 * y as usize + x).min(255);
                b.move_char(x, CP437[code], color, 1);
            }
            dc.write_line(0, y, self.vs.size.x, 1, &b);
        }
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        tvision::view::default_handle_event(&self.vs, ev, ctx);
        let id = self.vs.id;
        let size = self.vs.size;
        match ev {
            Event::MouseDown(_) | Event::MouseMove(_) if matches!(ev, Event::MouseDown(_)) || ctx.mouse_grabbed_by(id) => {
                let p = self.vs.make_local(ev.mouse().unwrap().pos);
                if self.vs.extent().contains(p) {
                    self.vs.set_cursor(p.x, p.y);
                    self.char_focused();
                }
                if matches!(ev, Event::MouseDown(_)) {
                    ctx.grab_mouse(id);
                }
                ev.clear();
            }
            Event::MouseUp(_) if ctx.mouse_grabbed_by(id) => {
                ctx.release_mouse();
                ev.clear();
            }
            Event::KeyDown(k) if self.vs.is_focused() => {
                let c = self.vs.cursor;
                match k.key {
                    Key::Home => self.vs.set_cursor(0, 0),
                    Key::End => self.vs.set_cursor(size.x - 1, size.y - 1),
                    Key::Up if c.y > 0 => self.vs.set_cursor(c.x, c.y - 1),
                    Key::Down if c.y < size.y - 1 => self.vs.set_cursor(c.x, c.y + 1),
                    Key::Left if c.x > 0 => self.vs.set_cursor(c.x - 1, c.y),
                    Key::Right if c.x < size.x - 1 => self.vs.set_cursor(c.x + 1, c.y),
                    Key::Up | Key::Down | Key::Left | Key::Right => {}
                    Key::Char(ch) => {
                        // Набранный символ: ищем его в таблице CP437.
                        if let Some(code) = CP437.iter().position(|&x| x == ch) {
                            self.vs.set_cursor(code as i32 % 32, code as i32 / 32);
                        }
                    }
                    _ => return,
                }
                self.char_focused();
                ev.clear();
            }
            _ => {}
        }
    }
}

/// Строка с кодом выбранного символа (`TReport`).
pub struct Report {
    pub vs: ViewState,
    pub ascii_char: u8,
}

impl View for Report {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let color = dc.color(6);
        let c = self.ascii_char;
        let shown = if c == 0 { ' ' } else { CP437[c as usize] };
        let s = format!("  Char: {} Decimal: {:3} Hex {:02X}     ", shown, c, c);
        let mut b = DrawBuffer::new(32);
        b.move_char(0, ' ', color, 32);
        b.move_str(0, &s, color);
        dc.write_line(0, 0, 32, 1, &b);
    }
}

/// Окно таблицы символов (`TAsciiChart`), серое.
pub struct AsciiChart {
    pub win: Window,
    table: ViewId,
    report: ViewId,
}

impl AsciiChart {
    pub fn new() -> Self {
        let mut win = Window::new(Rect::new(0, 0, 34, 12), "ASCII Chart", 0);
        win.flags &= !(wf::GROW | wf::ZOOM);
        win.group.vs.grow_mode = 0;
        win.palette = color::palettes::GRAY_WINDOW;
        let mut r = win.group.vs.extent().grown(-1, -1);
        r.a.y = r.b.y - 1;
        let report = win.add(Report { vs: ViewState::new(r), ascii_char: 0 });
        let mut r = win.group.vs.extent().grown(-1, -1);
        r.b.y -= 2;
        let table = win.add(Table::new(r));
        win.group.select_id(table, &mut Ctx::new());
        AsciiChart { win, table, report }
    }
}

impl View for AsciiChart {
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
        // `ofFramed` у таблицы и отчёта: разделительная линия между ними,
        // которую в оригинале рисовала рамка окна.
        let w = self.win.group.vs.size.x;
        let y = self.win.group.vs.size.y - 3;
        let mut fdc = dc.sub(self.win.group.vs.extent(), Frame::palette());
        let active = self.win.group.vs.is_active();
        let (attr, l, m, r) = if active { (fdc.get_color(0x0503), '╟', '─', '╢') } else { (fdc.get_color(0x0101), '├', '─', '┤') };
        let a = color::lo(attr);
        fdc.write_char(0, y, l, a, 1);
        fdc.write_char(1, y, m, a, w - 2);
        fdc.write_char(w - 1, y, r, a, 1);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        self.win.handle_window_event(ev, ctx);
        // Вместо сообщения `cmCharFocused` — передаём код прямо в «отчёт».
        let c = self.win.group.find::<Table>(self.table).map(|t| t.focused_char).unwrap_or(0);
        if let Some(r) = self.win.group.find_mut::<Report>(self.report) {
            r.ascii_char = c;
        }
    }
}
