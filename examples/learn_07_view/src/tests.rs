use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn counts() {
    let mut b = HeadlessBackend::new(80, 25);
    for k in [Key::Char(' '), Key::Char(' '), Key::Char(' '), Key::Char('-'), Key::Tab, Key::Enter] {
        b.push_key(KeyEvent::new(k, Mods::NONE));
    }
    let mut app = Program::new(Box::new(b), None, None);
    open_window(&mut app);
    app.pump(&mut program::NoHandler);
    let s = app.screen_text();
    assert!(s.contains("Счёт: 2") && s.contains("Счёт: 1"), "{}", s);
}
