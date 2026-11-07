// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «{title}» — источник `forms/run_job.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct RunJobData {
    /// Полоса выполнения «bar».
    pub bar: i64,
}

impl Default for RunJobData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            bar: 0,
        }
    }
}

impl RunJobData {
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
pub struct RunJobIds {
    pub name: ViewId,
    pub bar: ViewId,
    pub counts: ViewId,
    pub button1: ViewId,
}

/// Построить диалог «{title}».
pub fn build() -> (Dialog, RunJobIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 72, 13), "{title}");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «name»
    let name = d.add(StaticText::new(Rect::new(2, 2, 60, 3), "Подсчёт…"));
    // Полоса выполнения «bar»
    let bar = d.add(ProgressBar::new(Rect::new(2, 3, 60, 4), 100));
    // Текст «counts»
    let counts = d.add(StaticText::new(Rect::new(2, 5, 60, 6), ""));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(25, 7, 37, 9), "~С~топ", cm::CANCEL, bf::DEFAULT));
    (d, RunJobIds { name, bar, counts, button1 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut RunJobData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = RunJobData::from_value(&d.get_data());
    }
    command
}
