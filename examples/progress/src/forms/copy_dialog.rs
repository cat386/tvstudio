// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Копирование файлов» — источник `forms/copy_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyDialogData {
    /// Полоса выполнения «file_bar».
    pub file_bar: i64,
    /// Полоса выполнения «total_bar».
    pub total_bar: i64,
}

impl Default for CopyDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            file_bar: 0,
            total_bar: 0,
        }
    }
}

impl CopyDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.file_bar),
            Value::Int(self.total_bar),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Int(x)) = it.next() {
            d.file_bar = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.total_bar = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct CopyDialogIds {
    pub name: ViewId,
    pub file_bar: ViewId,
    pub text1: ViewId,
    pub total_bar: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Копирование файлов».
pub fn build() -> (Dialog, CopyDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 62, 15), "Копирование файлов");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «name»
    let name = d.add(StaticText::new(Rect::new(3, 2, 49, 3), ""));
    // Полоса выполнения «file_bar»
    let file_bar = d.add(ProgressBar::new(Rect::new(3, 3, 49, 4), 1));
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(3, 5, 49, 6), "Всего:"));
    // Полоса выполнения «total_bar»
    let total_bar = d.add(ProgressBar::new(Rect::new(3, 6, 49, 7), 100));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(20, 8, 32, 10), "Отмена", cm::CANCEL, bf::DEFAULT));
    (d, CopyDialogIds { name, file_bar, text1, total_bar, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut CopyDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = CopyDialogData::from_value(&d.get_data());
    }
    command
}
