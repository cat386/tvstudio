// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Форма «Calculator» — источник `forms/calculator.tvf` (меняйте её в дизайнере).
#![allow(dead_code, unused_imports, unused_mut, unused_variables)]

use tvision::*;

use crate::ui::cmd;

/// Данные формы — то, чем диалог обменивается с программой
/// (`set_data`/`get_data`). Порядок полей = порядок элементов.
#[derive(Clone, Debug, PartialEq)]
pub struct CalculatorData {
}

impl Default for CalculatorData {
    /// Начальные значения — как на форме в дизайнере.
    fn default() -> Self {
        Self {
        }
    }
}

impl CalculatorData {
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
pub struct CalculatorIds {
    pub button1: ViewId,
    pub button2: ViewId,
    pub button3: ViewId,
    pub button4: ViewId,
    pub button5: ViewId,
    pub button6: ViewId,
    pub button7: ViewId,
    pub button8: ViewId,
    pub button9: ViewId,
    pub button10: ViewId,
    pub button11: ViewId,
    pub button12: ViewId,
    pub button13: ViewId,
    pub button14: ViewId,
    pub button15: ViewId,
    pub button16: ViewId,
    pub button17: ViewId,
    pub button18: ViewId,
    pub button19: ViewId,
    pub button20: ViewId,
}

/// Построить диалог «Calculator».
pub fn build() -> (Dialog, CalculatorIds) {
    let mut d = Dialog::new(Rect::new(5, 3, 29, 18), "Calculator");
    // Кнопка «button1»
    let button1 = d.add(Button::new(Rect::new(2, 4, 7, 6), "C", 200, bf::NORMAL));
    // Кнопка «button2»
    let button2 = d.add(Button::new(Rect::new(7, 4, 12, 6), "←", 201, bf::NORMAL));
    // Кнопка «button3»
    let button3 = d.add(Button::new(Rect::new(12, 4, 17, 6), "%", 202, bf::NORMAL));
    // Кнопка «button4»
    let button4 = d.add(Button::new(Rect::new(17, 4, 22, 6), "±", 203, bf::NORMAL));
    // Кнопка «button5»
    let button5 = d.add(Button::new(Rect::new(2, 6, 7, 8), "7", 204, bf::NORMAL));
    // Кнопка «button6»
    let button6 = d.add(Button::new(Rect::new(7, 6, 12, 8), "8", 205, bf::NORMAL));
    // Кнопка «button7»
    let button7 = d.add(Button::new(Rect::new(12, 6, 17, 8), "9", 206, bf::NORMAL));
    // Кнопка «button8»
    let button8 = d.add(Button::new(Rect::new(17, 6, 22, 8), "/", 207, bf::NORMAL));
    // Кнопка «button9»
    let button9 = d.add(Button::new(Rect::new(2, 8, 7, 10), "4", 208, bf::NORMAL));
    // Кнопка «button10»
    let button10 = d.add(Button::new(Rect::new(7, 8, 12, 10), "5", 209, bf::NORMAL));
    // Кнопка «button11»
    let button11 = d.add(Button::new(Rect::new(12, 8, 17, 10), "6", 210, bf::NORMAL));
    // Кнопка «button12»
    let button12 = d.add(Button::new(Rect::new(17, 8, 22, 10), "*", 211, bf::NORMAL));
    // Кнопка «button13»
    let button13 = d.add(Button::new(Rect::new(2, 10, 7, 12), "1", 212, bf::NORMAL));
    // Кнопка «button14»
    let button14 = d.add(Button::new(Rect::new(7, 10, 12, 12), "2", 213, bf::NORMAL));
    // Кнопка «button15»
    let button15 = d.add(Button::new(Rect::new(12, 10, 17, 12), "3", 214, bf::NORMAL));
    // Кнопка «button16»
    let button16 = d.add(Button::new(Rect::new(17, 10, 22, 12), "-", 215, bf::NORMAL));
    // Кнопка «button17»
    let button17 = d.add(Button::new(Rect::new(2, 12, 7, 14), "0", 216, bf::NORMAL));
    // Кнопка «button18»
    let button18 = d.add(Button::new(Rect::new(7, 12, 12, 14), ".", 217, bf::NORMAL));
    // Кнопка «button19»
    let button19 = d.add(Button::new(Rect::new(12, 12, 17, 14), "=", 218, bf::NORMAL));
    // Кнопка «button20»
    let button20 = d.add(Button::new(Rect::new(17, 12, 22, 14), "+", 219, bf::NORMAL));
    (d, CalculatorIds { button1, button2, button3, button4, button5, button6, button7, button8, button9, button10, button11, button12, button13, button14, button15, button16, button17, button18, button19, button20 })
}

/// Показать форму модально. `data` заносятся в диалог, а если диалог
/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.
pub fn run(app: &mut Program, data: &mut CalculatorData) -> u16 {
    let (mut d, _ids) = build();
    d.set_data(&data.to_value());
    let (command, d) = app.exec_dialog(d);
    if command != cm::CANCEL {
        *data = CalculatorData::from_value(&d.get_data());
    }
    command
}
