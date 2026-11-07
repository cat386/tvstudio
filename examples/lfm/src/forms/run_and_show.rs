// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «PowerCLI» — источник `forms/run_and_show.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct RunAndShowData {
    /// Полоса выполнения «progress1».
    pub progress1: i64,
}

impl Default for RunAndShowData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            progress1: 0,
        }
    }
}

impl RunAndShowData {
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
pub struct RunAndShowIds {
    pub text1: ViewId,
    pub progress1: ViewId,
    pub button1: ViewId,
}

/// Построить диалог «PowerCLI».
pub fn build() -> (Dialog, RunAndShowIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 70, 11), "PowerCLI");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(2, 2, 58, 3), "Выполняется: {}"));
    // Полоса выполнения «progress1»
    let progress1 = d.add(ProgressBar::new(Rect::new(2, 3, 58, 4), 1));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(24, 5, 36, 7), "~С~топ", cm::CANCEL, bf::DEFAULT));
    (d, RunAndShowIds { text1, progress1, button1 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut RunAndShowData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = RunAndShowData::from_value(&d.get_data());
    }
    command
}
