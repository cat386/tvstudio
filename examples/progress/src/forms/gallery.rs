// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Виды полос» — источник `forms/gallery.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct GalleryData {
    /// Полоса выполнения «progress1».
    pub progress1: i64,
    /// Полоса выполнения «progress2».
    pub progress2: i64,
    /// Полоса выполнения «progress3».
    pub progress3: i64,
    /// Полоса выполнения «progress4».
    pub progress4: i64,
    /// Полоса выполнения «progress5».
    pub progress5: i64,
}

impl Default for GalleryData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
            progress1: 0,
            progress2: 0,
            progress3: 0,
            progress4: 0,
            progress5: 0,
        }
    }
}

impl GalleryData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.progress1),
            Value::Int(self.progress2),
            Value::Int(self.progress3),
            Value::Int(self.progress4),
            Value::Int(self.progress5),
        ])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        if let Some(Value::Int(x)) = it.next() {
            d.progress1 = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.progress2 = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.progress3 = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.progress4 = *x;
        }
        if let Some(Value::Int(x)) = it.next() {
            d.progress5 = *x;
        }
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct GalleryIds {
    pub text1: ViewId,
    pub progress1: ViewId,
    pub text2: ViewId,
    pub progress2: ViewId,
    pub text3: ViewId,
    pub progress3: ViewId,
    pub text4: ViewId,
    pub progress4: ViewId,
    pub text5: ViewId,
    pub progress5: ViewId,
    pub text6: ViewId,
}

/// Построить диалог «Виды полос».
pub fn build() -> (Dialog, GalleryIds) {
    let mut d = Dialog::new(Rect::new(4, 2, 60, 20), "Виды полос");
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(3, 2, 53, 3), "Плавная (по умолчанию):"));
    // Полоса выполнения «progress1»
    let progress1 = d.add(ProgressBar::new(Rect::new(3, 3, 53, 4), 100));
    // Текст «text2»
    let text2 = d.add(StaticText::new(Rect::new(3, 5, 53, 6), "Блоками, как в DOS:"));
    // Полоса выполнения «progress2»
    let progress2 = d.add(ProgressBar::new(Rect::new(3, 6, 53, 7), 100));
    // Текст «text3»
    let text3 = d.add(StaticText::new(Rect::new(3, 8, 53, 9), "Шаблон надписи:"));
    // Полоса выполнения «progress3»
    let progress3 = d.add(ProgressBar::new(Rect::new(3, 9, 53, 10), 100));
    // Текст «text4»
    let text4 = d.add(StaticText::new(Rect::new(3, 11, 53, 12), "Свои цвета:"));
    // Полоса выполнения «progress4»
    let progress4 = d.add(ProgressBar::new(Rect::new(3, 12, 53, 13), 100));
    // Текст «text5»
    let text5 = d.add(StaticText::new(Rect::new(3, 14, 53, 15), "Неизвестная длительность:"));
    // Полоса выполнения «progress5»
    let progress5 = d.add(ProgressBar::new(Rect::new(3, 15, 53, 16), 1));
    // Текст «text6»
    let text6 = d.add(StaticText::new(Rect::new(3, 16, 53, 17), "\u{3}«+» / «-» — изменить значение, Esc — закрыть"));
    (d, GalleryIds { text1, progress1, text2, progress2, text3, progress3, text4, progress4, text5, progress5, text6 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut GalleryData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = GalleryData::from_value(&d.get_data());
    }
    command
}
