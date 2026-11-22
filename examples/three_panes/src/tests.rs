use super::*;
use tvision::platform::HeadlessBackend;

fn key(k: Key) -> KeyEvent {
    KeyEvent::new(k, Mods::NONE)
}

#[test]
fn three_parts_and_filter() {
    let mut b = HeadlessBackend::new(80, 25);
    b.esc_budget = 0;
    let mut app = Program::new(Box::new(b), None, None);
    let size = app.desktop().vs().size;
    let id = app.insert_window(SkladWindow::new(Rect::new(0, 0, size.x, size.y)));
    app.redraw();
    let s = app.screen_text();
    println!("{}", s);
    // Линии-разделители и стыки с рамкой.
    assert!(s.contains('╤') && s.contains('┴') && s.contains('╟') && s.contains('╢'));
    // Под курсором дерева корень «Склад» — в таблице все 17 товаров.
    assert!(s.contains("Склад — Склад (17)"));

    // Фильтр «мо»: Молоко, Монитор, Молоток.
    let w = app.find_mut::<SkladWindow>(id).unwrap();
    let input = w.input;
    w.win.group.find_mut::<InputLine>(input).unwrap().set_text("мо");
    w.apply_filter();
    assert_eq!(w.win.group.find::<Table>(w.table).unwrap().model().row_count(), 4); // Морковь, Молоко, Монитор, Молоток
    app.redraw();
    assert!(app.screen_text().contains("4 из 17"));

    // То же через клавиатуру: Tab, Tab — в строку ввода, сбросить
    // фильтр пустой строкой и Enter.
    let w = app.find_mut::<SkladWindow>(id).unwrap();
    w.win.group.find_mut::<InputLine>(input).unwrap().set_text("");
    app.put_event(Event::KeyDown(key(Key::Tab)));
    app.put_event(Event::KeyDown(key(Key::Tab)));
    app.put_event(Event::KeyDown(key(Key::Enter)));
    app.pump(&mut App);
    app.redraw();
    let w = app.find::<SkladWindow>(id).unwrap();
    assert!(w.filter.is_empty());
    assert_eq!(w.win.group.find::<Table>(w.table).unwrap().model().row_count(), 17);
}
