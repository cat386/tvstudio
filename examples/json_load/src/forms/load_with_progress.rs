// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Загрузка файла» — источник `forms/load_with_progress.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadWithProgressData {
    /// Полоса выполнения «bar».
    pub bar: i64,
}

impl Default for LoadWithProgressData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            bar: 0,
        }
    }
}

impl LoadWithProgressData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.bar),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Int(x)) = it.next() {
            d.bar = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct LoadWithProgressIds {
    pub text1: ViewId,
    pub stage: ViewId,
    pub bar: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Загрузка файла».
pub fn build() -> (Dialog, LoadWithProgressIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 68, 14), "Загрузка файла");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(3, 2, 55, 3), "{name}"));
    // Текст «stage»
    let stage = d.add(StaticText::new(Rect::new(3, 3, 55, 4), "Открытие…"));
    // Полоса выполнения «bar»
    let bar = d.add(ProgressBar::new(Rect::new(3, 5, 55, 6), 1000));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(23, 7, 35, 9), "Отмена", cm::CANCEL, bf::DEFAULT));
    (d, LoadWithProgressIds { text1, stage, bar, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut LoadWithProgressData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = LoadWithProgressData::from_value(&d.get_data());
    }
    command
}
