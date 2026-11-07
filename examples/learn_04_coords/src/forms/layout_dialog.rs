// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Разметка» — источник `forms/layout_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDialogData {
    /// Строка ввода «field».
    pub field: String,
    /// Строка ввода «field2».
    pub field2: String,
    /// Строка ввода «field3».
    pub field3: String,
}

impl Default for LayoutDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            field: "".to_string(),
            field2: "".to_string(),
            field3: "".to_string(),
        }
    }
}

impl LayoutDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.field.clone()),
            Value::Str(self.field2.clone()),
            Value::Str(self.field3.clone()),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.field = x.clone();
        }
        if let Some(Value::Str(x)) = it.next() {
            d.field2 = x.clone();
        }
        if let Some(Value::Str(x)) = it.next() {
            d.field3 = x.clone();
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct LayoutDialogIds {
    pub field: ViewId,
    pub label1: ViewId,
    pub field2: ViewId,
    pub label2: ViewId,
    pub field3: ViewId,
    pub label3: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Разметка».
pub fn build() -> (Dialog, LayoutDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 54, 15), "Разметка");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «field»
    let field = d.add(InputLine::new(Rect::new(12, 2, 42, 3), 50));
    // Строка ввода «field2»
    let field2 = d.add(InputLine::new(Rect::new(12, 4, 42, 5), 50));
    // Строка ввода «field3»
    let field3 = d.add(InputLine::new(Rect::new(12, 6, 42, 7), 50));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(9, 9, 21, 11), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(23, 9, 35, 11), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 12, 3), "~И~мя:", Some(field)));
    // Метка «label2»
    let label2 = d.add(Label::new(Rect::new(2, 4, 12, 5), "~Г~ород:", Some(field2)));
    // Метка «label3»
    let label3 = d.add(Label::new(Rect::new(2, 6, 12, 7), "~П~очта:", Some(field3)));
    (d, LayoutDialogIds { field, label1, field2, label2, field3, label3, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut LayoutDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = LayoutDialogData::from_value(&d.get_data());
    }
    command
}
