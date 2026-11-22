use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

/// Тестовая программа 100×30 с заданным вводом.
/// Тестовый терминал сам не нажимает Esc (`esc_budget = 0`): иначе он
/// отменил бы загрузку, пока поток ещё читает файл.
fn program(input: Vec<RawEvent>) -> Program {
    let mut b = HeadlessBackend::new(100, 30);
    b.input.extend(input);
    b.esc_budget = 0;
    Program::new(Box::new(b), Some(menu()), Some(status()))
}

/// Временный каталог для файлов теста.
fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("json-load-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn bad_file_shows_error() {
    let p = tmp("bad").join("bad.json");
    std::fs::write(&p, "{\n  \"a\": [1, 2,\n}").unwrap();
    let mut app = program(vec![]);
    match load_with_progress(&mut app, &p) {
        Err(LoadError::Failed(m)) => assert!(m.contains("строка 3, столбец 1"), "{}", m),
        other => panic!("ожидалась ошибка разбора: {:?}", other.map(|t| t.rows.len())),
    }
}

#[test]
fn stages_in_order() {
    // Этапы идут по порядку, общий ход растёт и доходит до 1000.
    let p = tmp("stages").join("orders.json");
    generate_sample(&p, 3000).unwrap();
    let mut seen: Vec<(Stage, u64)> = Vec::new();
    let t = load_with(&p, &mut |st, done, total| {
        seen.push((st, st.permille(done, total)));
        true
    })
    .unwrap();
    assert_eq!(t.rows.len(), 3000);
    assert!(t.headers.contains(&"адрес.город".to_string()));
    let stages: Vec<Stage> = seen.iter().map(|x| x.0).collect();
    assert_eq!(stages.first(), Some(&Stage::Read));
    assert_eq!(stages.last(), Some(&Stage::Table));
    assert!(stages.contains(&Stage::Parse));
    assert!(seen.windows(2).all(|w| w[0].1 <= w[1].1), "ход не убывает");
    assert_eq!(seen.last().unwrap().1, 1000);
}

#[test]
fn load_can_be_cancelled() {
    // Esc в диалоге загрузки = «Отмена»: поток останавливается.
    let p = tmp("cancel").join("big.json");
    generate_sample(&p, 50_000).unwrap();
    let mut app = program(vec![RawEvent::Key(kb::ESC)]);
    let mut h = App;
    let t0 = std::time::Instant::now();
    assert!(!h.open(&mut app, &p), "загрузка отменена");
    assert!(t0.elapsed().as_secs() < 5, "поток остановился быстро");
}

#[test]
fn cancel_inside_parse() {
    // Прерывание посреди разбора JSON (а не только между этапами).
    let p = tmp("cancel2").join("big.json");
    generate_sample(&p, 20_000).unwrap();
    let r = load_with(&p, &mut |st, done, total| !(st == Stage::Parse && done * 2 > total));
    assert_eq!(r.unwrap_err(), LoadError::Cancelled);
}

#[test]
fn progress_dialog_shows_stage() {
    // Кадр диалога загрузки: заголовок, имя файла, полоса и «Отмена».
    let p = tmp("frame").join("orders.json");
    generate_sample(&p, 200).unwrap();
    let mut app = program(vec![]);
    load_with_progress(&mut app, &p).unwrap();
    // Последний нарисованный кадр — ещё с диалогом (сводку тест не показывает).
    let frame = app.screen_text();
    assert!(frame.contains("Загрузка файла"), "{}", frame);
    assert!(frame.contains("orders.json"), "{}", frame);
    assert!(frame.contains("Отмена"), "{}", frame);
}

#[test]
fn number_formatting() {
    assert_eq!(grouped(1234567), "1 234 567");
    assert_eq!(grouped(999), "999");
    assert_eq!(megabytes(1536 * 1024), "1,5 МБ");
    assert_eq!(stage_text(Stage::Table, 4000, 10000), "Построение таблицы: 2 000 из 5 000 записей");
}

#[test]
fn summary() {
    let p = tmp("summary").join("orders.json");
    generate_sample(&p, 1500).unwrap();
    let t = load_with(&p, &mut |_, _, _| true).unwrap();
    let s = summary_text(&p, &t, 2 * 1024 * 1024, Duration::from_millis(1234), 30);
    assert!(s.contains("orders.json"), "{}", s);
    assert!(s.contains("2,0 МБ"), "{}", s);
    assert!(s.contains("1,23 с"), "{}", s);
    assert!(s.contains("1 500 (поле «заказы»)"), "{}", s);
    assert!(s.lines().last().unwrap().ends_with('…'), "список столбцов обрезан:\n{}", s);
}
