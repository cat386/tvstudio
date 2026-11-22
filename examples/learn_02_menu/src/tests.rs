use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn f2_greets() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::F2); // окно сообщения закроет Esc тестового терминала
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    let mut h = App { greetings: 0 };
    app.pump(&mut h);
    assert_eq!(h.greetings, 1);
    // Второй раз — командой, как будто выбрали пункт меню.
    app.put_event(Event::command_event(CM_HELLO));
    app.pump(&mut h);
    assert_eq!(h.greetings, 2);
    assert!(app.screen_text().contains("Файл"));
}
