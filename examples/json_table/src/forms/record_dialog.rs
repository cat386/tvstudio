// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Запись {}» — источник `forms/record_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordDialogData {
    /// Таблица «tid».
    pub tid: i64,
}

impl Default for RecordDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            tid: 0,
        }
    }
}

impl RecordDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.tid),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Int(x)) = it.next() {
            d.tid = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct RecordDialogIds {
    pub tid: ViewId,
    pub button1: ViewId,
}

/// Построить диалог «Запись {}».
pub fn build() -> (Dialog, RecordDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 70, 19), "Запись {}");
    d.win.group.vs.options |= of::CENTERED;
    // Таблица «tid»
    let tid_sb = d.add(ScrollBar::new(Rect::new(57, 2, 58, 12)));
    let tid_model = VecTableModel::new(
        &["Поле", "Значение"],
        vec![
        ],
    );
    let tid = d.add(Table::new(Rect::new(2, 2, 57, 12), Box::new(tid_model), Some(tid_sb), None)
            .with_columns(vec![TableColumn::new(16).title("Поле"), TableColumn::new(36).title("Значение")])
            .header(true)
            .grid(true)
            .editable(false)
            .row_numbers(false));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(24, 13, 36, 15), "O~K~", cm::OK, bf::DEFAULT));
    (d, RecordDialogIds { tid, button1 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut RecordDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = RecordDialogData::from_value(&d.get_data());
    }
    command
}
