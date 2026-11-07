// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Компоненты» — источник `forms/components_dialog.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentsDialogData {
}

impl Default for ComponentsDialogData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
        }
    }
}

impl ComponentsDialogData {
    /// Данные → значение для `Dialog::set_data`.
    pub fn to_value(&self) -> Value {
        Value::List(vec![])
    }

    /// Значение из `Dialog::get_data` → данные.
    pub fn from_value(v: &Value) -> Self {
        let mut d = Self::default();
        let mut it = v.as_list().unwrap_or(&[]).iter();
        d
    }
}

/// id элементов построенного диалога (для доступа из своего кода:
/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).
#[derive(Clone, Copy, Debug)]
pub struct ComponentsDialogIds {
    pub text1: ViewId,
    pub tid: ViewId,
    pub button1: ViewId,
    pub button2: ViewId,
}

/// Построить диалог «Компоненты».
pub fn build() -> (Dialog, ComponentsDialogIds) {
    let mut d = Dialog::new(Rect::new(10, 3, 62, 21), "Компоненты");
    d.win.group.vs.options |= of::CENTERED;
    // Текст «text1»
    let text1 = d.add(StaticText::new(Rect::new(3, 2, 49, 3), "Пробел — отметить, Enter на листе — OK"));
    // Дерево «tid»
    let tid_sb = d.add(ScrollBar::new(Rect::new(47, 4, 48, 13)));
    let tid = {
        let mut v = TreeView::new(
            Rect::new(3, 4, 47, 13),
            vec![],
            Some(tid_sb),
            None,
        )
        .lines(true)
        .checkboxes(true);
        v.vs.grow_mode = 0;
        d.add(v)
    };
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(12, 14, 24, 16), "O~K~", cm::OK, bf::DEFAULT));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(27, 14, 39, 16), "Отмена", cm::CANCEL, bf::NORMAL));
    (d, ComponentsDialogIds { text1, tid, button1, button2 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut ComponentsDialogData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = ComponentsDialogData::from_value(&d.get_data());
    }
    command
}
