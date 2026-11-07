// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «About» — источник `forms/about.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct AboutData {
}

impl Default for AboutData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
        }
    }
}

impl AboutData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct AboutIds {
    pub text1: ViewId,
    pub button1: ViewId,
}

/// Построить диалог «About».
pub fn build() -> (Dialog, AboutIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 49, 16), "About");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(9, 2, 30, 9), "\u{3}Turbo Vision Demo\n\n\u{3}Rust Version\n\n\u{3}Copyright (c) 1994\n\n\u{3}Borland International"));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(14, 10, 26, 12), " OK", cm::OK, bf::DEFAULT));
    (d, AboutIds { text1, button1 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut AboutData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = AboutData::from_value(&d.get_data());
    }
    command
}
