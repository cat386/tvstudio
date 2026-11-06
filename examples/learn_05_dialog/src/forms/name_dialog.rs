// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Знакомство» — источник `forms/name_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct NameDialogData {
    /// Строка ввода «name».
    pub name: String,
}

impl Default for NameDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            name: "".to_string(),
        }
    }
}

impl NameDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.name = x.clone();
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct NameDialogIds {
    pub name: ViewId,
    pub name_label: ViewId,
    pub ok: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Знакомство».
pub fn build() -> (Dialog, NameDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 50, 12), "Знакомство");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «name»
    let name = d.add(InputLine::new(Rect::new(10, 2, 37, 3), 30));
    // Кнопка «ok»
    let ok = d.add(Button::new(Rect::new(6, 5, 18, 7), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(21, 5, 33, 7), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «name_label»
    let name_label = d.add(Label::new(Rect::new(2, 2, 9, 3), "~И~мя:", Some(name)));
    (d, NameDialogIds { name, name_label, ok, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut NameDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = NameDialogData::from_value(&d.get_data());
    }
    command
}
