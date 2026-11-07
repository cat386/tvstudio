// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «{title}» — источник `forms/contact_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactDialogData {
    /// Строка ввода «name».
    pub name: String,
    /// Строка ввода «phone».
    pub phone: String,
    /// Строка ввода «email».
    pub email: String,
    /// Переключатели «group».
    pub group: u32,
    /// Флажки «checks1».
    pub checks1: u32,
}

impl Default for ContactDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            name: "".to_string(),
            phone: "".to_string(),
            email: "".to_string(),
            group: 0,
            checks1: 0,
        }
    }
}

impl ContactDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()),
            Value::Str(self.phone.clone()),
            Value::Str(self.email.clone()),
            Value::U32(self.group),
            Value::U32(self.checks1),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.name = x.clone();
        }
        if let Some(Value::Str(x)) = it.next() {
            d.phone = x.clone();
        }
        if let Some(Value::Str(x)) = it.next() {
            d.email = x.clone();
        }
        if let Some(Value::U32(x)) = it.next() {
            d.group = *x;
        }
        if let Some(Value::U32(x)) = it.next() {
            d.checks1 = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct ContactDialogIds {
    pub name: ViewId,
    pub label1: ViewId,
    pub phone: ViewId,
    pub label2: ViewId,
    pub email: ViewId,
    pub label3: ViewId,
    pub group: ViewId,
    pub label4: ViewId,
    pub checks1: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «{title}».
pub fn build() -> (Dialog, ContactDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 62, 20), "{title}");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «name»
    let name = d.add(InputLine::new(Rect::new(13, 2, 49, 3), 60));
    // Строка ввода «phone»
    let phone = {
        let mut v = InputLine::new(Rect::new(13, 4, 49, 5), 30);
        v.validator = Some(Box::new(FilterValidator::new("0123456789+-() ")));
        d.add(v)
    };
    // Строка ввода «email»
    let email = d.add(InputLine::new(Rect::new(13, 6, 49, 7), 60));
    // Переключатели «group»
    let group = d.add(Cluster::radio_buttons(Rect::new(3, 9, 22, 12), &["~С~емья", "~Р~абота", "~Д~рузья"]));
    // Флажки «checks1»
    let checks1 = d.add(Cluster::check_boxes(Rect::new(26, 9, 49, 10), &["Из~б~ранный"]));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(12, 13, 24, 15), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(27, 13, 39, 15), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 12, 3), "~И~мя:", Some(name)));
    // Метка «label2»
    let label2 = d.add(Label::new(Rect::new(2, 4, 12, 5), "~Т~елефон:", Some(phone)));
    // Метка «label3»
    let label3 = d.add(Label::new(Rect::new(2, 6, 12, 7), "~П~очта:", Some(email)));
    // Метка «label4»
    let label4 = d.add(Label::new(Rect::new(2, 8, 12, 9), "~Г~руппа", Some(group)));
    (d, ContactDialogIds { name, label1, phone, label2, email, label3, group, label4, checks1, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut ContactDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = ContactDialogData::from_value(&d.get_data());
    }
    command
}
