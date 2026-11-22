use super::*;
use crate::model::{about_form, Control};

#[test]
fn builds_all_kinds() {
    let mut f = about_form("x");
    f.w = 70;
    f.h = 24;
    let mut y = 1;
    for k in Kind::ALL {
        let mut c = f.new_control(k, 2, y);
        if k == Kind::InputLine {
            c.history = 5;
            c.validator = "range:1..10".into();
        }
        y += c.h;
        f.controls.push(c);
    }
    let mut l = Control::new(Kind::Label, "lbl", 40, 1);
    l.link = "input1".into();
    f.controls.push(l);
    let (d, ids) = build_dialog(&f, &|_| 1000);
    assert_eq!(ids.len(), f.controls.len());
    // Данные: строка ввода, флажки, переключатели, список, поле, комбо.
    let data = d.get_data();
    assert_eq!(data.as_list().unwrap().len(), f.data_controls().len());
    let text = describe_data(&f, &data);
    assert!(text.contains("input1"), "{}", text);
}
