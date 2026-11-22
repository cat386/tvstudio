use tvision::platform::HeadlessBackend;
use tvision::*;

#[test]
fn alt_x_quits() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::ALT_X);
    let status = vec![StatusDef::all(vec![StatusItem::new("~Alt-X~ Выход", kb::ALT_X, cm::QUIT)])];
    let mut app = Program::new(Box::new(b), None, Some(status));
    app.pump(&mut program::NoHandler);
    assert!(!app.is_running());
    assert!(app.screen_text().contains("Alt-X Выход"));
}
