// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Анкета» — источник `forms/anketa.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct AnketaData {
    /// Строка ввода «name».
    pub name: String,
    /// Строка ввода «age».
    pub age: String,
    /// Переключатели «drink».
    pub drink: u32,
    /// Флажки «hobbies».
    pub hobbies: u32,
    /// Список «city».
    pub city: i64,
}

impl Default for AnketaData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            name: "".to_string(),
            age: "".to_string(),
            drink: 0,
            hobbies: 0,
            city: 0,
        }
    }
}

impl AnketaData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()),
            Value::Str(self.age.clone()),
            Value::U32(self.drink),
            Value::U32(self.hobbies),
            Value::Int(self.city),
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
            d.age = x.clone();
        }
        if let Some(Value::U32(x)) = it.next() {
            d.drink = *x;
        }
        if let Some(Value::U32(x)) = it.next() {
            d.hobbies = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.city = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct AnketaIds {
    pub name: ViewId,
    pub name_label: ViewId,
    pub age: ViewId,
    pub age_label: ViewId,
    pub drink: ViewId,
    pub drink_label: ViewId,
    pub hobbies: ViewId,
    pub hobbies_label: ViewId,
    pub city: ViewId,
    pub city_label: ViewId,
    pub ok: ViewId,
    pub cancel: ViewId,
}

/// Построить диалог «Анкета».
pub fn build() -> (Dialog, AnketaIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 66, 22), "Анкета");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «name»
    let name = d.add(InputLine::new(Rect::new(14, 2, 52, 3), 40));
    // Строка ввода «age»
    let age = {
        let mut v = InputLine::new(Rect::new(14, 4, 20, 5), 3);
        v.validator = Some(Box::new(RangeValidator::new(1, 120)));
        d.add(v)
    };
    // Переключатели «drink»
    let drink = d.add(Cluster::radio_buttons(Rect::new(3, 7, 20, 10), &["~Ч~ай", "~К~офе", "~С~ок"]));
    // Флажки «hobbies»
    let hobbies = d.add(Cluster::check_boxes(Rect::new(23, 7, 52, 9), &["~К~ниги", "С~п~орт", "~М~узыка", "И~г~ры"]));
    // Список «city»
    let city_sb = d.add(ScrollBar::new(Rect::new(20, 12, 21, 15)));
    let city = {
        let mut v = ListBox::new(Rect::new(3, 12, 20, 15), 1, Some(city_sb));
        v.set_items(vec!["Москва".to_string(), "Тула".to_string(), "Казань".to_string(), "Тверь".to_string(), "Сочи".to_string(), "Омск".to_string()]);
        d.add(v)
    };
    // Кнопка «ok»
    let ok = d.add(Button::new(Rect::new(30, 12, 42, 14), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «cancel»
    let cancel = d.add(Button::new(Rect::new(30, 15, 42, 17), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «name_label»
    let name_label = d.add(Label::new(Rect::new(2, 2, 13, 3), "~И~мя:", Some(name)));
    // Метка «age_label»
    let age_label = d.add(Label::new(Rect::new(2, 4, 13, 5), "~В~озраст:", Some(age)));
    // Метка «drink_label»
    let drink_label = d.add(Label::new(Rect::new(2, 6, 13, 7), "Напиток", Some(drink)));
    // Метка «hobbies_label»
    let hobbies_label = d.add(Label::new(Rect::new(22, 6, 35, 7), "Увлечения", Some(hobbies)));
    // Метка «city_label»
    let city_label = d.add(Label::new(Rect::new(2, 11, 13, 12), "~Г~ород", Some(city)));
    (d, AnketaIds { name, name_label, age, age_label, drink, drink_label, hobbies, hobbies_label, city, city_label, ok, cancel })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut AnketaData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = AnketaData::from_value(&d.get_data());
    }
    command
}
