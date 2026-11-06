//! Тесты `tvdemo` на «невидимом» терминале. Запуск: `cargo test --example tvdemo`.

use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

fn key(k: KeyEvent) -> RawEvent {
    RawEvent::Key(k)
}
fn ch(c: char) -> RawEvent {
    RawEvent::Key(KeyEvent::new(Key::Char(c), Mods::NONE))
}
fn plain(k: Key) -> RawEvent {
    RawEvent::Key(KeyEvent::new(k, Mods::NONE))
}

/// Программа на тестовом терминале 80×25 с заданным вводом.
fn run(input: Vec<RawEvent>) -> (Program, TvDemo) {
    let mut b = HeadlessBackend::new(80, 25);
    b.input.extend(input);
    let (mut app, mut demo) = build(Box::new(b));
    app.pump(&mut demo);
    (app, demo)
}

fn screen(app: &Program) -> Vec<String> {
    app.screen_text().lines().map(|s| s.to_string()).collect()
}

/// Открыть пункт меню «≡» по букве.
fn system_menu(c: char) -> Vec<RawEvent> {
    vec![key(kb::alt(' ')), ch(c)]
}

#[test]
fn layout_and_gadgets() {
    let (app, _) = run(vec![]);
    let s = screen(&app);
    assert!(s[0].starts_with("  ≡  File  Windows  Options"), "{:?}", s[0]);
    // Часы ЧЧ:ММ:СС в правом верхнем углу.
    let clock: String = s[0].chars().skip(71).take(8).collect();
    assert_eq!(clock.chars().filter(|c| *c == ':').count(), 2, "{:?}", clock);
    assert!(s[24].starts_with(" F1 Help  Alt-X Exit"), "{:?}", s[24]);
}

#[test]
fn calculator() {
    // 12 * 3 = 36, затем 10 / 4 = 2.5
    let mut input = system_menu('c');
    input.extend("12*3=".chars().map(ch));
    let (app, _) = run(input);
    let w = app.desktop().group.children.last().unwrap();
    let c = w.as_any().downcast_ref::<calc::Calculator>().unwrap();
    assert_eq!(c.display().display(), "36");

    let mut input = system_menu('c');
    input.extend("10/4=".chars().map(ch));
    input.push(ch('±'));
    let (app, _) = run(input);
    let w = app.desktop().group.children.last().unwrap();
    let c = w.as_any().downcast_ref::<calc::Calculator>().unwrap();
    assert_eq!(c.display().display(), "-2.5");
    // Деление на ноль — «Error», «C» сбрасывает.
    let mut d = calc::CalcDisplay::new(Rect::new(0, 0, 18, 1));
    for k in "5/0=".chars() {
        d.calc_key(k);
    }
    assert_eq!(d.display(), "Error");
    d.calc_key('C');
    assert_eq!(d.display(), "0");
}

#[test]
fn calculator_buttons() {
    // Щелчки по кнопкам «7», «+», «8», «=» (калькулятор в (5,3) рабочего стола).
    let click = |x: i32, y: i32| {
        vec![
            RawEvent::Mouse { pos: Point::new(x, y), buttons: 1, mods: Mods::NONE, wheel: 0 },
            RawEvent::Mouse { pos: Point::new(x, y), buttons: 0, mods: Mods::NONE, wheel: 0 },
        ]
    };
    // Кнопка i: x = 5 + (i%4)*5 + 2 + 2, y = 1 + 3 + (i/4)*2 + 4.
    let pos = |i: i32| (5 + (i % 4) * 5 + 4, 1 + 3 + (i / 4) * 2 + 4);
    let mut input = system_menu('c');
    for i in [4, 19, 5, 18] {
        let (x, y) = pos(i);
        input.extend(click(x, y));
    }
    let (app, _) = run(input);
    let w = app.desktop().group.children.last().unwrap();
    let c = w.as_any().downcast_ref::<calc::Calculator>().unwrap();
    assert_eq!(c.display().display(), "15");
}

#[test]
fn puzzle_and_calendar() {
    let (app, _) = run(system_menu('p'));
    let s = screen(&app);
    assert!(s.iter().any(|l| l.contains(" Puzzle ")));
    assert!(s.iter().any(|l| l.contains("Move")));

    // Календарь: «+» — следующий месяц.
    let mut input = system_menu('l');
    input.push(ch('+'));
    let (app, _) = run(input);
    let w = app.desktop().group.children.last().unwrap();
    let cv = w.as_group().unwrap().children.iter().find_map(|c| c.as_any().downcast_ref::<calendar::CalendarView>()).unwrap();
    let now = platform::time::now();
    let expected = if now.month == 12 { 1 } else { now.month + 1 };
    assert_eq!(cv.month, expected);
    assert!(screen(&app).iter().any(|l| l.contains("Su Mo Tu We Th Fr Sa")));
}

#[test]
fn ascii_table() {
    // Ввод «A» переводит курсор на символ 65.
    let mut input = system_menu('t');
    input.push(ch('A'));
    let (app, _) = run(input);
    let s = screen(&app);
    assert!(s.iter().any(|l| l.contains("Char: A Decimal:  65 Hex 41")), "{:?}", s);
    assert!(s.iter().any(|l| l.contains("░▒▓│┤")), "графика CP437");
}

#[test]
fn background_pattern() {
    // Options → Background, ввести «#», Apply, Close.
    let input = vec![key(kb::alt('o')), ch('b'), ch('#'), key(kb::ENTER), key(kb::ESC)];
    let (app, _) = run(input);
    assert_eq!(app.desktop().pattern(), '#');
    assert!(screen(&app)[5].starts_with("####"));
}

#[test]
fn event_viewer_logs() {
    let input = vec![key(kb::alt('0')), ch('x'), ch('y')];
    let (app, demo) = run(input);
    let id = demo.event_viewer.unwrap();
    let v = app.find::<evntview::EventViewer>(id).unwrap();
    let t = v.win.group.children.iter().find_map(|c| c.as_any().downcast_ref::<Terminal>()).unwrap();
    let text: Vec<&String> = t.lines().collect();
    assert!(text.iter().any(|l| l.contains("KeyDown { key: Char('x')")), "{:?}", text);
}

#[test]
fn help_for_puzzle() {
    // F1 в окне пятнашек открывает тему «Puzzle» (Esc закрывает справку).
    let mut input = system_menu('p');
    input.push(key(kb::F1));
    input.push(key(kb::ESC));
    let (_, demo) = run(input);
    assert!(demo.last_help.iter().any(|l| l.contains("Puzzle")), "{:?}", demo.last_help);
    assert!(demo.last_help.iter().any(|l| l.contains("puzzle window")), "{:?}", demo.last_help);
}

#[test]
fn save_and_restore_desktop() {
    let dir = std::env::temp_dir().join(format!("tvdemo-dst-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_current_dir(&dir).unwrap();
    let mut input = system_menu('p');
    input.extend(system_menu('t'));
    input.extend([key(kb::alt('o')), ch('d'), ch('s')]);
    let (_app, _) = run(input);
    let saved = std::fs::read_to_string(dir.join(DESKTOP_FILE)).unwrap();
    assert_eq!(saved.lines().count(), 2, "{}", saved);
    assert!(saved.contains("puzzle") && saved.contains("ascii"));

    let (app, _) = run(vec![key(kb::alt('o')), ch('d'), ch('r')]);
    assert_eq!(app.desktop().group.children.len(), 3, "фон + два окна");
}
