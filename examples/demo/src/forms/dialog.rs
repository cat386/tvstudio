// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Настройки» — источник `forms/dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogData {
    /// Строка ввода «name».
    pub name: String,
    /// Флажки «opts».
    pub opts: u32,
    /// Переключатели «color».
    pub color: u32,
    /// Список «list».
    pub list: i64,
}

impl Default for DialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            name: "".to_string(),
            opts: 0,
            color: 0,
            list: 0,
        }
    }
}

impl DialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()),
            Value::U32(self.opts),
            Value::U32(self.color),
            Value::Int(self.list),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.name = x.clone();
        }
        if let Some(Value::U32(x)) = it.next() {
            d.opts = *x;
        }
        if let Some(Value::U32(x)) = it.next() {
            d.color = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.list = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct DialogIds {
    pub name: ViewId,
    pub label1: ViewId,
    pub opts: ViewId,
    pub label2: ViewId,
    pub color: ViewId,
    pub label3: ViewId,
    pub list: ViewId,
    pub label4: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Настройки».
pub fn build() -> (Dialog, DialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 66, 21), "Настройки");
    d.win.group.vs.options |= of::CENTERED;
    d.win.group.vs.help_ctx = 20;
    // Строка ввода «name»
    let name = d.add(InputLine::new(Rect::new(14, 2, 52, 3), 64));
    // Флажки «opts»
    let opts = d.add(Cluster::check_boxes(Rect::new(3, 5, 26, 8), &["~Ж~ирный", "~К~урсив", "~П~одчёркнутый"]));
    // Переключатели «color»
    let color = d.add(Cluster::radio_buttons(Rect::new(29, 5, 52, 8), &["~С~иний", "~З~елёный", "К~р~асный"]));
    // Список «list»
    let list_sb = d.add(ScrollBar::new(Rect::new(52, 10, 53, 14)));
    let list = {
        let mut v = ListBox::new(Rect::new(3, 10, 52, 14), 2, Some(list_sb));
        v.set_items(vec!["Rust".to_string(), "C++".to_string(), "Pascal".to_string(), "C".to_string(), "Go".to_string(), "Zig".to_string(), "Ada".to_string(), "Modula-2".to_string(), "Oberon".to_string(), "Fortran".to_string()]);
        d.add(v)
    };
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(14, 15, 26, 17), "~O~K", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(30, 15, 42, 17), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 13, 3), "~И~мя:", Some(name)));
    // Метка «label2»
    let label2 = d.add(Label::new(Rect::new(2, 4, 12, 5), "Стиль", Some(opts)));
    // Метка «label3»
    let label3 = d.add(Label::new(Rect::new(28, 4, 38, 5), "Цвет", Some(color)));
    // Метка «label4»
    let label4 = d.add(Label::new(Rect::new(2, 9, 12, 10), "~Я~зык", Some(list)));
    (d, DialogIds { name, label1, opts, label2, color, label3, list, label4, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut DialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = DialogData::from_value(&d.get_data());
    }
    command
}
