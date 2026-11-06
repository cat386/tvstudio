// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Hello, World!» — источник `forms/greeting.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct GreetingData {
}

impl Default for GreetingData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
        }
    }
}

impl GreetingData {
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
pub struct GreetingIds {
    pub question: ViewId,
    pub terrific: ViewId,
    pub ok: ViewId,
    pub lousy: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Hello, World!».
pub fn build() -> (Dialog, GreetingIds) {
    let mut d = Dialog::new(Rect::new(25, 5, 55, 16), "Hello, World!");
    // Текст «question»
    let question = d.add(StaticText::new(Rect::new(3, 5, 15, 6), "How are you?"));
    // Кнопка «terrific»
    let terrific = d.add(Button::new(Rect::new(16, 2, 28, 4), "Terrific", cm::CANCEL, bf::NORMAL));
    // Кнопка «ok»
    let ok = d.add(Button::new(Rect::new(16, 4, 28, 6), "Ok", cm::CANCEL, bf::NORMAL));
    // Кнопка «lousy»
    let lousy = d.add(Button::new(Rect::new(16, 6, 28, 8), "Lousy", cm::CANCEL, bf::NORMAL));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(16, 8, 28, 10), "Cancel", cm::CANCEL, bf::NORMAL));
    (d, GreetingIds { question, terrific, ok, lousy, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut GreetingData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = GreetingData::from_value(&d.get_data());
    }
    command
}
