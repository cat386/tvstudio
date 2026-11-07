// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Выбор города» — источник `forms/city.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct CityData {
    /// Таблица «tid».
    pub tid: i64,
}

impl Default for CityData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            tid: 0,
        }
    }
}

impl CityData {
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
pub struct CityIds {
    pub tid: ViewId,
    pub label1: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Выбор города».
pub fn build() -> (Dialog, CityIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 70, 21), "Выбор города");
    d.win.group.vs.options |= of::CENTERED;
    // Таблица «tid»
    let tid_sb = d.add(ScrollBar::new(Rect::new(56, 3, 57, 14)));
    let tid_model = VecTableModel::new(
        &["Город", "Округ", "Население"],
        vec![
        ],
    );
    let tid = d.add(Table::new(Rect::new(3, 3, 56, 14), Box::new(tid_model), Some(tid_sb), None)
            .with_columns(vec![TableColumn::new(18).title("Город"), TableColumn::new(17).title("Округ"), TableColumn::new(14).title("Население")])
            .header(true)
            .grid(false)
            .editable(false)
            .row_numbers(false));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(17, 15, 29, 17), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(31, 15, 43, 17), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 20, 3), "~Г~орода:", Some(tid)));
    (d, CityIds { tid, label1, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut CityData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = CityData::from_value(&d.get_data());
    }
    command
}
