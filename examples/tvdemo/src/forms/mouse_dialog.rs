// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Mouse options» — источник `forms/mouse_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct MouseDialogData {
    /// Флажки «checks1».
    pub checks1: u32,
}

impl Default for MouseDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            checks1: 0,
        }
    }
}

impl MouseDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::U32(self.checks1),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::U32(x)) = it.next() {
            d.checks1 = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct MouseDialogIds {
    pub scroll: ViewId,
    pub label1: ViewId,
    pub checks1: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Mouse options».
pub fn build() -> (Dialog, MouseDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 44, 15), "Mouse options");
    d.win.group.vs.options |= of::CENTERED;
    // Полоса прокрутки «scroll»
    let scroll = d.add(ScrollBar::new(Rect::new(3, 4, 30, 5)));
    // Флажки «checks1»
    let checks1 = d.add(Cluster::check_boxes(Rect::new(3, 6, 30, 7), &["~R~everse mouse buttons"]));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(9, 9, 19, 11), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(21, 9, 31, 11), "Cancel", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 21, 3), "~M~ouse double click", Some(scroll)));
    (d, MouseDialogIds { scroll, label1, checks1, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut MouseDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = MouseDialogData::from_value(&d.get_data());
    }
    command
}
