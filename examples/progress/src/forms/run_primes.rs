// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Поиск простых чисел» — источник `forms/run_primes.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct RunPrimesData {
    /// Полоса выполнения «bar».
    pub bar: i64,
}

impl Default for RunPrimesData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            bar: 0,
        }
    }
}

impl RunPrimesData {
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
pub struct RunPrimesIds {
    pub info: ViewId,
    pub bar: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Поиск простых чисел».
pub fn build() -> (Dialog, RunPrimesIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 60, 13), "Поиск простых чисел");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «info»
    let info = d.add(StaticText::new(Rect::new(3, 2, 47, 3), "Запуск потока…"));
    // Полоса выполнения «bar»
    let bar = d.add(ProgressBar::new(Rect::new(3, 4, 47, 5), 100));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(19, 6, 31, 8), "Отмена", cm::CANCEL, bf::DEFAULT));
    (d, RunPrimesIds { info, bar, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut RunPrimesData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = RunPrimesData::from_value(&d.get_data());
    }
    command
}
