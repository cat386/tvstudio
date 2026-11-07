// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Сортировка» — источник `forms/sort.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct SortData {
    /// Переключатели «rid».
    pub rid: u32,
    /// Флажки «cid».
    pub cid: u32,
}

impl Default for SortData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            rid: 0,
            cid: 0,
        }
    }
}

impl SortData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::U32(self.rid),
            Value::U32(self.cid),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::U32(x)) = it.next() {
            d.rid = *x;
        }
        if let Some(Value::U32(x)) = it.next() {
            d.cid = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct SortIds {
    pub rid: ViewId,
    pub label1: ViewId,
    pub cid: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Сортировка».
pub fn build() -> (Dialog, SortIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 54, 16), "Сортировка");
    d.win.group.vs.options |= of::CENTERED;
    // Переключатели «rid»
    let rid = d.add(Cluster::radio_buttons(Rect::new(3, 3, 22, 8), &["~И~мя", "~Р~асширение", "Ра~з~мер", "~В~ремя", "~Б~ез сортировки"]));
    // Флажки «cid»
    let cid = d.add(Cluster::check_boxes(Rect::new(23, 3, 42, 6), &["~О~братный", "~К~аталоги с файлами", "Р~е~гистр"]));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(9, 10, 21, 12), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(23, 10, 35, 12), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 20, 3), "~П~орядок", Some(rid)));
    (d, SortIds { rid, label1, cid, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut SortData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = SortData::from_value(&d.get_data());
    }
    command
}
