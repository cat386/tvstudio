use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn clone_dialog_keeps_url_while_browsing() {
    let base = std::env::temp_dir();
    let url = "https://example.test/team/project.git";
    let mut backend = HeadlessBackend::new(100, 30);
    for c in url.chars() {
        backend.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    backend.push_key(kb::key(Key::Tab));
    backend.push_key(kb::key(Key::Tab));
    backend.push_key(kb::key(Key::Char(' '))); // кнопка «…»
    backend.push_key(kb::ENTER); // Выбрать этот каталог
    backend.push_key(kb::ENTER); // Клонировать
    let mut app = Program::new(Box::new(backend), None, None);
    assert_eq!(clone_dialog(&mut app, &base), Some(CloneRequest { url: url.into(), parent: base, branch: CloneBranch::Direct(None) }));
}
