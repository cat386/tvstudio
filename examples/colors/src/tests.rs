use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn theme_and_table() {
    let mut b = HeadlessBackend::new(80, 25);
    b.push_key(kb::F2); // таблица цветов
    b.push_key(kb::f(3)); // классическая
    b.push_key(kb::f(3)); // «Сумерки»
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    assert_eq!(app.palette, color::palettes::GRUVBOX); // тема по умолчанию
    let mut h = App { theme: 0, mode: None };
    app.pump(&mut h);
    app.redraw();
    assert_eq!(h.theme, 2);
    // Рабочий стол (цвет 1 палитры, было 0x71) — теперь xterm 24 на 252.
    let desk: color::Attr = app.screen().get(79, 22).attr;
    assert_eq!(desk, attr256(24, 252));
    // В таблице клетка с номером 196 закрашена цветом 196.
    let s = app.screen_text();
    let (y, line) = s.lines().enumerate().find(|(_, l)| l.contains(" 196")).unwrap();
    let x = line.chars().take_while(|_| true).collect::<Vec<_>>().windows(4).position(|w| w.iter().collect::<String>() == " 196").unwrap();
    assert_eq!(color::bg256(app.screen().get(x + 1, y).attr), 196);
}
