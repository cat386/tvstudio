// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Файл загружен» — источник `forms/summary_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryDialogData {
}

impl Default for SummaryDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
        }
    }
}

impl SummaryDialogData {
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
pub struct SummaryDialogIds {
    pub text1: ViewId,
    pub ok: ViewId,
}

/// Построить диалог «Файл загружен».
pub fn build() -> (Dialog, SummaryDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 70, 17), "Файл загружен");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(3, 2, 57, 10), "{text}"));
    // Кнопка «ok»
    let ok = d.add(Button::new(Rect::new(24, 11, 36, 13), "O~K~", cm::OK, bf::DEFAULT));
    (d, SummaryDialogIds { text1, ok })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut SummaryDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = SummaryDialogData::from_value(&d.get_data());
    }
    command
}
