use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn add_done_delete() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::key(Key::Ins));
    for c in "Купить хлеб".chars() {
        b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    b.push_key(kb::ENTER);
    b.push_key(kb::f(4)); // курсор на новом деле — отметить
    let mut app = Program::new(Box::new(b), None, Some(status()));
    let mut h = App { lists: 0 };
    h.new_list(&mut app);
    app.pump(&mut h);
    let w = App::active(&mut app).unwrap();
    assert_eq!(w.tasks[1], Task { text: "Купить хлеб".into(), done: true });
    assert!(app.screen_text().contains("[x] Купить хлеб"));

    app.put_event(Event::command_event(CM_DELETE));
    app.pump(&mut h);
    assert_eq!(App::active(&mut app).unwrap().tasks.len(), 1);
}
