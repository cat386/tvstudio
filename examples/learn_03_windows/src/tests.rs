use super::*;
use tvision::platform::HeadlessBackend;

/// Сколько окон на рабочем столе.
fn windows(app: &Program) -> usize {
    app.desktop().group.children.iter().filter(|c| c.as_window().is_some()).count()
}

#[test]
fn open_and_close() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::F3);
    b.push_key(kb::F3);
    b.push_key(kb::ALT_F3); // закрыть верхнее
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    let mut h = App { opened: 0 };
    app.pump(&mut h);
    assert_eq!(windows(&app), 1);
    assert!(app.screen_text().contains("Окно 1"));
}
