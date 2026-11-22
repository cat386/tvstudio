use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn remembers_name() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::F2);
    for c in "Аня".chars() {
        b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    b.push_key(kb::ENTER); // OK; окно «Привет» закроет Esc тестового терминала
    let mut app = Program::new(Box::new(b), None, Some(status()));
    let mut h = App { name: String::new() };
    app.pump(&mut h);
    assert_eq!(h.name, "Аня");
}
