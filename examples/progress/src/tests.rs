use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

/// Программа на тестовом терминале. Esc сам не нажимается
/// (`esc_budget = 0`), иначе тестовый терминал отменил бы диалог.
fn program(input: Vec<RawEvent>) -> Program {
    let mut b = HeadlessBackend::new(80, 25);
    b.input.extend(input);
    b.esc_budget = 0;
    Program::new(Box::new(b), Some(menu()), Some(status()))
}

fn key(k: KeyEvent) -> RawEvent {
    RawEvent::Key(k)
}

#[test]
fn copy_runs_to_the_end() {
    // Копирование идёт само и закрывает диалог, когда все файлы скопированы.
    let mut app = program(vec![]);
    let (cmd, copied) = run_copy(&mut app);
    assert_eq!((cmd, copied), (cm::OK, FILES.len()));
}

#[test]
fn copy_can_be_cancelled() {
    // Esc в диалоге = «Отмена»: копирование прерывается в самом начале.
    let mut app = program(vec![key(kb::ESC)]);
    let (cmd, copied) = run_copy(&mut app);
    assert_eq!(cmd, cm::CANCEL);
    assert!(copied < FILES.len());
}

#[test]
fn primes_in_thread() {
    // До 100 000 — 9592 простых.
    let mut app = program(vec![]);
    assert_eq!(run_primes(&mut app, 100_000), Some(9592));
}

#[test]
fn gallery_keys() {
    let plus = key(KeyEvent::new(Key::Char('+'), Mods::NONE));
    let mut app = program(vec![key(kb::f(4)), plus.clone(), plus]);
    app.pump(&mut App);
    let g = app.desktop().group.children.last().unwrap().as_any().downcast_ref::<Gallery>().unwrap();
    assert_eq!(g.value(), 47);
    let s = app.screen_text();
    assert!(s.contains("47%"), "{}", s);
    assert!(s.contains("Загружено 47 МБ из 100 МБ"), "{}", s);
    assert!(s.contains("█"), "полоса «блоками»");
    // Плавная полоса: окно в (4,2) на рабочем столе (строка 1 экрана),
    // полоса — в строке 3 окна, столбцы 3..53. 47% от 50 клеток = 23,5.
    let scr = app.screen();
    let (y, x0) = (1 + 2 + 3, 4 + 3);
    // Цвета зависят от темы (по умолчанию — 256-цветная), поэтому
    // сравниваем клетки между собой.
    let filled = scr.get(x0, y).attr;
    let empty = scr.get(x0 + 40, y).attr;
    assert_ne!(filled, empty, "заполненная и пустая части разного цвета");
    assert_eq!(scr.get(x0 + 22, y).attr, filled);
    // Надпись «47%» стоит по центру поверх полосы.
    assert_eq!(scr.get(x0 + 23, y).ch, '4');
    assert_eq!(scr.get(x0 + 24, y).attr, empty);
}

#[test]
fn primes_correct() {
    let (tx, rx) = mpsc::channel();
    count_primes(1000, tx, Arc::new(AtomicBool::new(false)));
    let last = rx.try_iter().last();
    assert!(matches!(last, Some(Work::Done(168))));
}
