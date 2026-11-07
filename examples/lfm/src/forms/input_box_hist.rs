// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «{title}» — источник `forms/input_box_hist.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct InputBoxHistData {
    /// Строка ввода «iid».
    pub iid: String,
}

impl Default for InputBoxHistData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            iid: "{initial}".to_string(),
        }
    }
}

impl InputBoxHistData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.iid.clone()),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.iid = x.clone();
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct InputBoxHistIds {
    pub iid: ViewId,
    pub label1: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «{title}».
pub fn build() -> (Dialog, InputBoxHistIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 72, 11), "{title}");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «iid»
    let iid = {
        let mut v = InputLine::new(Rect::new(3, 3, 56, 4), 1024);
        v.set_text("{initial}");
        d.add(v)
    };
    d.add(History::new(Rect::new(56, 3, 59, 4), iid, 1));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(17, 5, 29, 7), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(31, 5, 43, 7), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 40, 3), "{label}", Some(iid)));
    (d, InputBoxHistIds { iid, label1, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut InputBoxHistData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = InputBoxHistData::from_value(&d.get_data());
    }
    command
}
