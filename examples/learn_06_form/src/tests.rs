use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn data_roundtrip() {
    let p = Profile { name: "Боря".into(), age: "41".into(), drink: 2, hobbies: 0b1010, city: 4 };
    let d = form_dialog(&p);
    assert_eq!(Profile::from_value(&d.get_data()), p);
    assert!(p.describe().contains("Спорт, Игры"));
}

#[test]
fn age_is_checked() {
    // Возраст 200: OK не закрывает диалог (сообщение об ошибке и сам
    // диалог закроет Esc тестового терминала).
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::TAB); // в поле «Возраст»
    for c in "200".chars() {
        b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    b.push_key(kb::ENTER);
    let mut app = Program::new(Box::new(b), None, None);
    let (cmd, _) = app.exec_dialog(form_dialog(&Profile::default()));
    assert_eq!(cmd, cm::CANCEL);
}
