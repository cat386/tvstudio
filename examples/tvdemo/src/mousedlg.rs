//! Диалог настроек мыши (`mousedlg.cpp`): `TClickTester` и `TMouseDialog`.
//! Полоса прокрутки задаёт интервал двойного щелчка, двойной щелчок по
//! надписи проверяет его, флажок меняет местами кнопки мыши.

use std::time::Duration;

use tvision::*;

/// Палитра проверочной надписи (`cpMousePalette`).
const MOUSE_PALETTE: &[u8] = b"\x07\x08";

/// Надпись, которая меняет цвет по двойному щелчку (`TClickTester`).
pub struct ClickTester {
    pub text: StaticText,
    pub clicked: bool,
}

impl View for ClickTester {
    tvision::impl_view_basics_field!(text);

    fn draw(&self, dc: &mut DrawCtx) {
        let c = if self.clicked { dc.color(2) } else { dc.color(1) };
        let w = self.text.vs.size.x.max(0) as usize;
        let mut b = DrawBuffer::new(w);
        b.move_char(0, ' ', c, w);
        b.move_str(0, &self.text.text, c);
        dc.write_line(0, 0, self.text.vs.size.x, 1, &b);
    }

    fn palette(&self) -> &'static [u8] {
        MOUSE_PALETTE
    }

    fn handle_event(&mut self, ev: &mut Event, _ctx: &mut Ctx) {
        if let Event::MouseDown(m) = ev {
            if m.double {
                self.clicked = !self.clicked;
            }
            ev.clear();
        }
    }
}

/// Диалог «Mouse options» (`TMouseDialog`). Шкала: 1..20 десятков миллисекунд,
/// как `TEventQueue::doubleDelay` (в «тиках» по ~55 мс в оригинале).
pub struct MouseDialog {
    pub dlg: Dialog,
    scroll: ViewId,
    /// Текущий интервал двойного щелчка — программа применит его после OK.
    pub delay: Duration,
    old_delay: Duration,
}

/// Шаг шкалы: одно деление — 50 мс.
const STEP_MS: u64 = 50;

impl MouseDialog {
    pub fn new(delay: Duration, reverse: bool) -> Self {
        let mut dlg = Dialog::centered(34, 12, "Mouse options");
        let mut sb = ScrollBar::new(Rect::new(3, 4, 30, 5));
        let v = (delay.as_millis() as u64 / STEP_MS).clamp(1, 20) as i32;
        sb.set_params(v, 1, 20, 20, 1);
        sb.vs.options |= of::SELECTABLE;
        let scroll = dlg.add(sb);
        dlg.add(Label::new(Rect::new(2, 2, 21, 3), "~M~ouse double click", Some(scroll)));
        dlg.add(ClickTester { text: StaticText::new(Rect::new(3, 3, 30, 4), "Fast       Medium      Slow"), clicked: false });
        let mut cb = Cluster::check_boxes(Rect::new(3, 6, 30, 7), &["~R~everse mouse buttons"]);
        cb.value = reverse as u32;
        dlg.add(cb);
        dlg.add(Button::new(Rect::new(9, 9, 19, 11), "O~K~", cm::OK, bf::DEFAULT));
        dlg.add(Button::new(Rect::new(21, 9, 31, 11), "Cancel", cm::CANCEL, bf::NORMAL));
        dlg.select(scroll);
        MouseDialog { dlg, scroll, delay, old_delay: delay }
    }

    /// Флажок «Reverse mouse buttons».
    pub fn reverse(&self) -> bool {
        match self.dlg.get_data() {
            Value::List(v) => v.iter().any(|x| x.as_u32() == Some(1)),
            _ => false,
        }
    }
}

impl View for MouseDialog {
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
        // Стрелки влево/вправо двигают выбранную полосу прокрутки.
        if let Event::KeyDown(k) = ev {
            let focused = self.dlg.win.group.current_view().map(|v| v.vs().id) == Some(self.scroll);
            if focused && matches!(k.key, Key::Left | Key::Right) && k.mods == Mods::NONE {
                let d = if k.key == Key::Left { -1 } else { 1 };
                if let Some(sb) = self.dlg.win.group.find_mut::<ScrollBar>(self.scroll) {
                    let v = sb.value + d;
                    sb.set_params(v, 1, 20, 20, 1);
                }
                ev.clear();
            }
        }
        self.dlg.handle_event(ev, ctx);
        if ev.command() == Some(cm::CANCEL) {
            self.delay = self.old_delay;
        }
        if let Some(sb) = self.dlg.win.group.find::<ScrollBar>(self.scroll) {
            self.delay = Duration::from_millis(sb.value as u64 * STEP_MS);
        }
    }
}
