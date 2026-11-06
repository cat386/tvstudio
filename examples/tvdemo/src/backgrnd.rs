//! Смена узора фона рабочего стола (`backgrnd.cpp`: `TChBackground`).
//! Кнопка «Apply» меняет узор и оставляет диалог открытым.

use tvision::*;

pub struct ChBackground {
    pub dlg: Dialog,
    input: ViewId,
    /// Выбранный узор (программа применяет его к фону).
    pub pattern: Option<char>,
    /// Сколько раз нажали «Apply» (программа сверяет, что применять).
    pub applied: u32,
}

/// Команда «Apply» (в оригинале это OK, который не закрывает диалог).
pub const CM_APPLY_BACKGROUND: u16 = 120;

impl ChBackground {
    pub fn new(current: char) -> Self {
        let mut dlg = Dialog::centered(29, 9, "");
        let input = dlg.add(InputLine::new(Rect::new(4, 5, 7, 6), 1));
        dlg.add(StaticText::new(Rect::new(2, 2, 27, 3), "Enter background pattern:"));
        dlg.add(Button::new(Rect::new(16, 4, 26, 6), "~A~pply", CM_APPLY_BACKGROUND, bf::DEFAULT));
        dlg.add(Button::new(Rect::new(16, 6, 26, 8), "~C~lose", cm::CANCEL, bf::NORMAL));
        if let Some(il) = dlg.win.group.find_mut::<InputLine>(input) {
            il.set_text(&current.to_string());
        }
        dlg.select(input);
        ChBackground { dlg, input, pattern: None, applied: 0 }
    }
}

impl View for ChBackground {
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
        self.dlg.handle_event(ev, ctx);
        if ev.command() == Some(CM_APPLY_BACKGROUND) {
            let t = self.dlg.win.group.find::<InputLine>(self.input).map(|i| i.text()).unwrap_or_default();
            if let Some(c) = t.chars().next() {
                self.pattern = Some(c);
                self.applied += 1;
                // Применяем сразу, не закрывая диалог (как `TChBackground::valid`).
                ctx.with_program(move |app| app.desktop_mut().set_pattern(c));
            }
            ev.clear();
        }
    }
}
