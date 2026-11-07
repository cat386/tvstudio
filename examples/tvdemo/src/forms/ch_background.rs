// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «» — источник `forms/ch_background.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct ChBackgroundData {
    /// Строка ввода «input».
    pub input: String,
}

impl Default for ChBackgroundData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            input: "".to_string(),
        }
    }
}

impl ChBackgroundData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.input.clone()),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.input = x.clone();
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct ChBackgroundIds {
    pub input: ViewId,
    pub text1: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «».
pub fn build() -> (Dialog, ChBackgroundIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 39, 12), "");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «input»
    let input = d.add(InputLine::new(Rect::new(4, 5, 7, 6), 1));
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(2, 2, 27, 3), "Enter background pattern:"));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(16, 4, 26, 6), "~A~pply", cmd::CM_APPLY_BACKGROUND, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(16, 6, 26, 8), "~C~lose", cm::CANCEL, bf::NORMAL));
    (d, ChBackgroundIds { input, text1, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut ChBackgroundData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = ChBackgroundData::from_value(&d.get_data());
    }
    command
}
