// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Поиск» — источник `forms/run_search.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSearchData {
    /// Полоса выполнения «progress1».
    pub progress1: i64,
}

impl Default for RunSearchData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            progress1: 0,
        }
    }
}

impl RunSearchData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.progress1),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Int(x)) = it.next() {
            d.progress1 = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct RunSearchIds {
    pub where_: ViewId,
    pub count: ViewId,
    pub progress1: ViewId,
    pub button1: ViewId,
}

/// Построить диалог «Поиск».
pub fn build() -> (Dialog, RunSearchIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 72, 12), "Поиск");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «where_»
    let where_ = d.add(StaticText::new(Rect::new(2, 2, 60, 3), ""));
    // Текст «count»
    let count = d.add(StaticText::new(Rect::new(2, 3, 60, 4), ""));
    // Полоса выполнения «progress1»
    let progress1 = d.add(ProgressBar::new(Rect::new(2, 4, 60, 5), 1));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(25, 6, 37, 8), "~С~топ", cm::CANCEL, bf::DEFAULT));
    (d, RunSearchIds { where_, count, progress1, button1 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut RunSearchData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = RunSearchData::from_value(&d.get_data());
    }
    command
}
