use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

fn key(k: Key) -> RawEvent {
    RawEvent::Key(KeyEvent::new(k, Mods::NONE))
}

fn text(s: &str) -> Vec<RawEvent> {
    s.chars().map(|c| key(Key::Char(c))).collect()
}

/// Программа на «невидимом» терминале 100×30 с окном образца.
fn program(input: Vec<RawEvent>) -> Program {
    let mut b = HeadlessBackend::new(100, 30);
    b.input.extend(input);
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    App::open_window(&mut app, sample_contacts(), None);
    app
}

#[test]
fn value_roundtrip() {
    let c = Contact::new("Аня", "123", "a@b", 2, true);
    assert_eq!(Contact::from_value(&c.to_value()), c);
    assert_eq!(Contact::from_line(&c.to_line()), Some(c));
}

#[test]
fn window_shows_table() {
    let mut app = program(vec![]);
    app.redraw();
    let s = app.screen_text();
    assert!(s.contains("Контакты — без имени"), "{}", s);
    assert!(s.contains("Анна Петрова") && s.contains("Работа"), "{}", s);
}

#[test]
fn dialog_validates_name() {
    // Пустое имя: OK не закрывает диалог, а показывает сообщение
    // (его закроет Esc тестового терминала), потом Esc закрывает диалог.
    let mut app = program(vec![key(Key::Enter)]);
    let (cmd, _) = app.exec_dialog(contact_dialog("Новый контакт", &Contact::default()));
    assert_eq!(cmd, cm::CANCEL, "без имени диалог OK не принимает");
}

#[test]
fn add_contact_through_dialog() {
    // Ins → диалог; имя, Tab, телефон (буквы фильтр не пропустит), Enter — OK.
    let mut input = vec![key(Key::Ins)];
    input.extend(text("Дарья"));
    input.push(key(Key::Tab));
    input.extend(text("+7 (999) abc 12"));
    input.push(key(Key::Enter));
    let mut app = program(input);
    app.pump(&mut App);
    let w = App::window(&mut app).unwrap();
    assert_eq!(w.contacts.len(), 5);
    let c = w.contacts.last().unwrap();
    assert_eq!((c.name.as_str(), c.phone.as_str()), ("Дарья", "+7 (999)  12"));
    assert!(w.modified);
    assert_eq!(w.current(), Some(4), "курсор на новом контакте");
}

#[test]
fn delete_asks_and_commands_follow_state() {
    // Del → вопрос, Enter — «Да» (кнопка по умолчанию).
    let mut app = program(vec![key(Key::Del), key(Key::Enter)]);
    app.pump(&mut App);
    assert_eq!(App::window(&mut app).unwrap().contacts.len(), 3);
    // Без строк «Изменить» и «Удалить» запрещены.
    App::window(&mut app).unwrap().contacts.clear();
    App.idle(&mut app);
    assert!(!app.command_enabled(CM_DELETE) && !app.command_enabled(CM_EDIT));
    assert!(app.command_enabled(CM_ADD));
}

#[test]
fn save_and_load_file() {
    let path = std::env::temp_dir().join(format!("guide-contacts-{}.tsv", std::process::id()));
    save_contacts(&path, &sample_contacts()).unwrap();
    assert_eq!(load_contacts(&path).unwrap(), sample_contacts());
    let _ = std::fs::remove_file(&path);
}
