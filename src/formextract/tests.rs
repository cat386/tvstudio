use super::*;

const SRC: &str = r#"
const CM_GO: u16 = 120;
const ROWS: [&str; 3] = ["~И~мя:", "~Г~ород:", "~П~очта:"];

fn layout_dialog(title: &str) -> Dialog {
    let (w, h) = (44, 12);
    let mut d = Dialog::centered(w, h, title);
    for (i, text) in ROWS.iter().enumerate() {
        let y = 2 + 2 * i as i32;
        let field = d.add(InputLine::new(Rect::new(12, y, w - 2, y + 1), 50).with_validator(FilterValidator::new("0123")));
        d.add(Label::new(Rect::new(2, y, 12, y + 1), text, Some(field)));
    }
    let sb = d.add(ScrollBar::new(Rect::new(30, 8, 31, 10)));
    let mut lb = ListBox::new(Rect::new(3, 8, 30, 10), 1, Some(sb));
    lb.set_items(["a", "b"].iter().map(|s| s.to_string()).collect());
    let list = d.add(lb);
    let mut c = Cluster::check_boxes(Rect::new(32, 8, 42, 9), &["~X~"]);
    c.value = 1;
    d.add(c);
    if title.is_empty() {
        d.add(Button::new(Rect::new(1, 1, 5, 3), "?", cm::OK, bf::NORMAL));
    }
    d.add(MyWidget::new(Rect::new(1, 1, 2, 2)));
    let bw = 12;
    let x0 = (w - (bw + 2 + bw)) / 2;
    d.add(Button::new(Rect::new(x0, h - 3, x0 + bw, h - 1), "O~K~", CM_GO, bf::DEFAULT));
    d
}
"#;

const WINDOW_SRC: &str = r#"
fn open_counters() -> Window {
    let mut w = Window::new(Rect::new(10, 3, 46, 11), "Счётчики", 1);
    w.add(StaticText::new(Rect::new(2, 2, 30, 4), "Всего: 0", None));
    w
}
"#;

#[test]
fn extracts_window_form() {
    let v = extract(WINDOW_SRC, &HashMap::new());
    assert_eq!(v.len(), 1);
    let f = &v[0].form;
    assert_eq!(f.class, crate::model::FormClass::Window);
    assert_eq!((f.x, f.y, f.w, f.h, f.number), (10, 3, 36, 8, 1));
    assert_eq!(f.title, "Счётчики");
    assert!(!f.centered);
    assert_eq!(f.controls.len(), 1);
    assert_eq!(f.controls[0].kind, Kind::StaticText);
}

#[test]
fn extracts_layout_dialog() {
    let v = extract(SRC, &HashMap::new());
    assert_eq!(v.len(), 1);
    let x = &v[0];
    let f = &x.form;
    assert_eq!(f.name, "layout_dialog");
    assert_eq!(f.title, "{title}");
    assert_eq!((f.w, f.h, f.centered), (44, 12, true));
    let kinds: Vec<Kind> = f.controls.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![Kind::InputLine, Kind::Label, Kind::InputLine, Kind::Label, Kind::InputLine, Kind::Label, Kind::ListBox, Kind::CheckBoxes, Kind::Button]
    );
    assert_eq!(f.controls[2].y, 4);
    assert_eq!(f.controls[2].w, 30);
    assert_eq!(f.controls[2].validator, "filter:0123");
    assert_eq!(f.controls[3].link, f.controls[2].name);
    assert_eq!(f.controls[3].text, "~Г~ород:");
    let lb = &f.controls[6];
    assert_eq!((lb.name.as_str(), lb.w, lb.scrollbar), ("list", 28, true));
    assert_eq!(lb.items, vec!["a", "b"]);
    assert_eq!(f.controls[7].value, 1);
    let ok = &f.controls[8];
    assert_eq!((ok.x, ok.y, ok.command.as_str(), ok.default), (9, 9, "CM_GO", true));
    assert_eq!(x.commands, vec![("CM_GO".to_string(), 120)]);
    assert!(x.notes.iter().any(|n| n.contains("условия")), "{:?}", x.notes);
    assert!(x.notes.iter().any(|n| n.contains("MyWidget")), "{:?}", x.notes);
}
