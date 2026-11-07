// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Поиск файлов» — источник `forms/find.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct FindData {
    /// Строка ввода «pid».
    pub pid: String,
    /// Строка ввода «tid».
    pub tid: String,
}

impl Default for FindData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            pid: "*".to_string(),
            tid: "".to_string(),
        }
    }
}

impl FindData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.pid.clone()),
            Value::Str(self.tid.clone()),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Str(x)) = it.next() {
            d.pid = x.clone();
        }
        if let Some(Value::Str(x)) = it.next() {
            d.tid = x.clone();
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct FindIds {
    pub pid: ViewId,
    pub label1: ViewId,
    pub tid: ViewId,
    pub label2: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Поиск файлов».
pub fn build() -> (Dialog, FindIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 66, 14), "Поиск файлов");
    d.win.group.vs.options |= of::CENTERED;
    // Строка ввода «pid»
    let pid = {
        let mut v = InputLine::new(Rect::new(3, 3, 53, 4), 255);
        v.set_text("*");
        d.add(v)
    };
    // Строка ввода «tid»
    let tid = d.add(InputLine::new(Rect::new(3, 6, 53, 7), 255));
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(14, 8, 26, 10), "~И~скать", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(29, 8, 41, 10), "Отмена", cm::CANCEL, bf::NORMAL));
    // Метка «label1»
    let label1 = d.add(Label::new(Rect::new(2, 2, 40, 3), "~И~мя (шаблоны через запятую):", Some(pid)));
    // Метка «label2»
    let label2 = d.add(Label::new(Rect::new(2, 5, 40, 6), "~С~одержит текст (необязательно):", Some(tid)));
    (d, FindIds { pid, label1, tid, label2, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut FindData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = FindData::from_value(&d.get_data());
    }
    command
}
