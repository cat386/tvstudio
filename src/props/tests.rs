use super::*;
use crate::model::about_form;

#[test]
fn edit_props() {
    let mut f = about_form("x");
    set(&mut f, Some(1), Prop::Name, "close_btn").unwrap();
    assert_eq!(f.controls[1].name, "close_btn");
    assert!(set(&mut f, Some(1), Prop::Name, "info").is_err());
    assert!(set(&mut f, Some(1), Prop::Default, "может быть").is_err());
    set(&mut f, Some(0), Prop::Text, "a\\nb").unwrap();
    assert_eq!(f.controls[0].text, "a\nb");
    assert_eq!(get(&f, Some(0), Prop::Text), "a\\nb");
    set(&mut f, None, Prop::FormPalette, "синий").unwrap();
    assert_eq!(f.palette, Pal::Blue);
    assert!(set(&mut f, Some(1), Prop::Validator, "range:1..x").is_err());
}

#[test]
fn tree_and_table_props() {
    let mut f = about_form("x");
    f.controls.push(crate::model::Control::new(Kind::TreeView, "tree", 2, 2));
    let i = f.controls.len() - 1;
    assert_eq!(get(&f, Some(i), Prop::Items), "Документы | >Письма | >Отчёты | Музыка | Фото | >2026");
    set(&mut f, Some(i), Prop::Items, "А | >Б | >>В").unwrap();
    assert_eq!(f.controls[i].items, vec!["А", "  Б", "    В"]);
    let t = crate::model::parse_tree(&f.controls[i].items);
    assert_eq!(t[0].children[0].children[0].text, "В");
    set(&mut f, Some(i), Prop::Checks, "да").unwrap();
    assert!(f.controls[i].checks);
    f.controls.push(crate::model::Control::new(Kind::Table, "table", 2, 2));
    let c = f.controls.last().unwrap();
    assert_eq!(c.table_columns(), vec![("Имя".to_string(), 16), ("Телефон".to_string(), 14)]);
    assert_eq!(c.table_rows()[1], vec!["Борис", "234-56-78"]);
}

#[test]
fn line_props() {
    let mut f = about_form("x");
    f.controls.push(crate::model::Control::new(Kind::Line, "line", 2, 2));
    let i = f.controls.len() - 1;
    assert_eq!((f.controls[i].w, f.controls[i].h), (20, 1));
    set(&mut f, Some(i), Prop::Vertical, "да").unwrap();
    assert!(f.controls[i].vertical);
    assert_eq!((f.controls[i].w, f.controls[i].h), (1, 20));
    // Повторная установка того же значения размеры не трогает.
    set(&mut f, Some(i), Prop::Vertical, "да").unwrap();
    assert_eq!((f.controls[i].w, f.controls[i].h), (1, 20));
    set(&mut f, Some(i), Prop::Double, "да").unwrap();
    set(&mut f, Some(i), Prop::ToFrame, "да").unwrap();
    assert!(f.controls[i].double && f.controls[i].to_frame);
    assert_eq!(get(&f, Some(i), Prop::ToFrame), "да");
    set(&mut f, Some(i), Prop::Vertical, "нет").unwrap();
    assert_eq!((f.controls[i].w, f.controls[i].h), (20, 1));
}

#[test]
fn escapes() {
    let s = "\x03По центру\nи \\ слеш";
    assert_eq!(unescape(&escape(s)), s);
}
