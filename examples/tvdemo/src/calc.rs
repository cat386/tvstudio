//! Калькулятор (`calc.cpp`): `TCalcDisplay` и `TCalculator`.
//!
//! В оригинале кнопки посылали широковещательное сообщение `cmCalcButton`,
//! а дисплей читал первую букву надписи нажатой кнопки. Здесь у каждой кнопки
//! своя команда (`CM_CALC_BUTTON + номер`), и окно калькулятора переводит её
//! в «нажатие клавиши» для дисплея.

use tvision::*;

/// Ширина дисплея (`DISPLAYLEN`).
const DISPLAY_LEN: usize = 25;

/// Палитра дисплея (`cpCalcPalette`): цвет строки ввода в диалоге.
const CALC_PALETTE: &[u8] = b"\x13";

/// Команды кнопок: 200..219.
pub const CM_CALC_BUTTON: u16 = 200;

/// Надписи кнопок (`keyChar`): «←» вместо ESC, «±» вместо символа 0xF1.
const KEY_CHAR: [&str; 20] = ["C", "←", "%", "±", "7", "8", "9", "/", "4", "5", "6", "*", "1", "2", "3", "-", "0", ".", "=", "+"];

/// Состояние дисплея (`TCalcState`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CalcState {
    First,
    Valid,
    Error,
}

/// Дисплей калькулятора (`TCalcDisplay`).
pub struct CalcDisplay {
    pub vs: ViewState,
    status: CalcState,
    number: String,
    sign: char,
    operate: char,
    operand: f64,
}

impl CalcDisplay {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.options |= of::SELECTABLE;
        let mut d = CalcDisplay { vs, status: CalcState::First, number: String::new(), sign: ' ', operate: '=', operand: 0.0 };
        d.clear();
        d
    }

    /// Показываемое число со знаком (для тестов).
    #[allow(dead_code)]
    pub fn display(&self) -> String {
        format!("{}{}", if self.sign == '-' { "-" } else { "" }, self.number)
    }

    fn error(&mut self) {
        self.status = CalcState::Error;
        self.number = "Error".into();
        self.sign = ' ';
    }

    fn clear(&mut self) {
        self.status = CalcState::First;
        self.number = "0".into();
        self.sign = ' ';
        self.operate = '=';
        self.operand = 0.0;
    }

    /// Число на дисплее (`getDisplay`).
    fn get_display(&self) -> f64 {
        self.number.parse().unwrap_or(0.0)
    }

    /// Показать результат (`setDisplay`).
    fn set_display(&mut self, r: f64) {
        if !r.is_finite() {
            self.error();
            return;
        }
        self.sign = if r < 0.0 { '-' } else { ' ' };
        // Как вывод double в ostream: без лишних нулей.
        let mut s = format!("{}", r.abs());
        if s.len() > 15 {
            s = format!("{:.10e}", r.abs());
        }
        if s.len() > DISPLAY_LEN {
            self.error();
        } else {
            self.number = s;
        }
    }

    fn check_first(&mut self) {
        if self.status == CalcState::First {
            self.status = CalcState::Valid;
            self.number = "0".into();
            self.sign = ' ';
        }
    }

    /// Обработка «клавиши» калькулятора (`calcKey`).
    pub fn calc_key(&mut self, key: char) {
        let mut key = key.to_ascii_uppercase();
        if self.status == CalcState::Error && key != 'C' {
            key = ' ';
        }
        match key {
            '0'..='9' => {
                self.check_first();
                if self.number.len() < 15 {
                    if self.number == "0" {
                        self.number.clear();
                    }
                    self.number.push(key);
                }
            }
            '.' => {
                self.check_first();
                if !self.number.contains('.') {
                    self.number.push('.');
                }
            }
            '\x08' | '\x1b' | '←' => {
                self.check_first();
                if self.number.chars().count() == 1 {
                    self.number = "0".into();
                } else {
                    self.number.pop();
                }
            }
            '_' | '±' => self.sign = if self.sign == ' ' { '-' } else { ' ' },
            '+' | '-' | '*' | '/' | '=' | '%' | '\r' => {
                if self.status == CalcState::Valid {
                    self.status = CalcState::First;
                    let mut r = self.get_display() * if self.sign == '-' { -1.0 } else { 1.0 };
                    if key == '%' {
                        if self.operate == '+' || self.operate == '-' {
                            r = (self.operand * r) / 100.0;
                        } else {
                            r /= 100.0;
                        }
                    }
                    match self.operate {
                        '+' => self.set_display(self.operand + r),
                        '-' => self.set_display(self.operand - r),
                        '*' => self.set_display(self.operand * r),
                        '/' => {
                            if r == 0.0 {
                                self.error()
                            } else {
                                self.set_display(self.operand / r)
                            }
                        }
                        _ => {}
                    }
                }
                self.operate = key;
                self.operand = self.get_display() * if self.sign == '-' { -1.0 } else { 1.0 };
            }
            'C' => self.clear(),
            _ => {}
        }
    }
}

impl View for CalcDisplay {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let color = dc.color(1);
        let w = self.vs.size.x.max(0) as usize;
        let mut b = DrawBuffer::new(w);
        b.move_char(0, ' ', color, w);
        let i = w.saturating_sub(self.number.chars().count() + 2);
        b.move_char(i, self.sign, color, 1);
        b.move_str(i + 1, &self.number, color);
        dc.write_line(0, 0, self.vs.size.x, 1, &b);
    }

    fn palette(&self) -> &'static [u8] {
        CALC_PALETTE
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        tvision::view::default_handle_event(&self.vs, ev, ctx);
        if let Event::KeyDown(k) = ev {
            let c = match k.key {
                Key::Char(c) if !k.mods.ctrl && !k.mods.alt => Some(c),
                Key::Enter => Some('\r'),
                Key::Backspace => Some('\x08'),
                Key::Esc => Some('\x1b'),
                _ => None,
            };
            if let Some(c) = c {
                self.calc_key(c);
                ev.clear();
            }
        }
    }
}

/// Окно калькулятора (`TCalculator`).
pub struct Calculator {
    pub dlg: Dialog,
    display: ViewId,
}

impl Calculator {
    pub fn new() -> Self {
        let mut dlg = Dialog::new(Rect::new(5, 3, 29, 18), "Calculator");
        dlg.win.group.vs.options |= of::FIRST_CLICK;
        for (i, t) in KEY_CHAR.iter().enumerate() {
            let x = (i as i32 % 4) * 5 + 2;
            let y = (i as i32 / 4) * 2 + 4;
            let mut b = Button::new(Rect::new(x, y, x + 5, y + 2), t, CM_CALC_BUTTON + i as u16, bf::NORMAL);
            b.vs.options &= !of::SELECTABLE;
            dlg.add(b);
        }
        let display = dlg.add(CalcDisplay::new(Rect::new(3, 2, 21, 3)));
        dlg.select(display);
        Calculator { dlg, display }
    }

    #[allow(dead_code)]
    pub fn display(&self) -> &CalcDisplay {
        self.dlg.win.group.find::<CalcDisplay>(self.display).unwrap()
    }
}

impl View for Calculator {
    tvision::impl_group_basics!(dlg.win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.dlg.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.dlg.draw(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.dlg.palette()
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.dlg.set_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        // Esc и Enter — клавиши калькулятора, а не закрытие диалога.
        if let Event::KeyDown(k) = ev {
            if matches!(k.key, Key::Esc | Key::Enter) && k.mods == Mods::NONE {
                let id = self.display;
                if let Some(d) = self.dlg.win.group.find_mut::<CalcDisplay>(id) {
                    d.calc_key(if k.key == Key::Esc { '\x1b' } else { '\r' });
                }
                ev.clear();
                return;
            }
        }
        self.dlg.handle_event(ev, ctx);
        if let Some(c) = ev.command() {
            if (CM_CALC_BUTTON..CM_CALC_BUTTON + 20).contains(&c) {
                let key = KEY_CHAR[(c - CM_CALC_BUTTON) as usize].chars().next().unwrap();
                let id = self.display;
                if let Some(d) = self.dlg.win.group.find_mut::<CalcDisplay>(id) {
                    d.calc_key(key);
                }
                ev.clear();
            }
        }
    }
}
