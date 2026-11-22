use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

fn key(k: Key, m: Mods) -> RawEvent {
    RawEvent::Key(KeyEvent::new(k, m))
}

/// Все три окна открываются, рисуются без ошибок палитры, правка работает.
#[test]
fn smoke() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::F2); // большая таблица
    b.input.push_back(key(Key::End, Mods::CTRL));
    b.push_key(kb::f(3)); // склад
    b.input.push_back(key(Key::Right, Mods::NONE)); // «Наименование»
    b.input.push_back(key(Key::Right, Mods::NONE)); // «Кол-во»
    b.input.push_back(key(Key::Char('5'), Mods::NONE));
    b.input.push_back(key(Key::Enter, Mods::NONE));
    let menu = Menu::new().item("x", CM_BIG);
    let status = vec![StatusDef::all(vec![
        StatusItem::new("~F2~", kb::F2, CM_BIG),
        StatusItem::new("~F3~", kb::f(3), CM_GOODS),
        StatusItem::new("~F4~", kb::f(4), CM_CITY),
    ])];
    let mut app = Program::new(Box::new(b), Some(menu), Some(status));
    let mut h = App { windows: 0 };
    app.pump(&mut h);
    app.redraw();
    let s = app.screen_text();
    assert!(s.contains("Склад"), "{}", s);
    assert!(s.contains("Наименование"));
    assert!(s.contains("6.00"), "сумма пересчитана после правки количества: {}", s);
    assert!(s.contains("1000000"), "номер последней строки большой таблицы:\n{}", s);
    for y in 0..25 {
        for x in 0..80 {
            assert_ne!(app.screen().get(x, y).attr, color::ERROR_ATTR, "({}, {})", x, y);
        }
    }
}
