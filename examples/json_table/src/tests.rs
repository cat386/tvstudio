use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

/// Тестовая программа 100×30 с заданным вводом.
fn program(input: Vec<RawEvent>) -> Program {
    let mut b = HeadlessBackend::new(100, 30);
    b.input.extend(input);
    Program::new(Box::new(b), Some(menu()), Some(status()))
}

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/data/books.json")
}

#[test]
fn sample_file_in_window() {
    let mut app = program(vec![]);
    let mut h = App { windows: 0 };
    assert!(h.open(&mut app, &sample_path()));
    app.redraw();
    let screen = app.screen_text();
    assert!(screen.contains("books.json → книги"), "заголовок окна:\n{}", screen);
    assert!(screen.contains("название"), "{}", screen);
    assert!(screen.contains("Мастер и Маргарита"), "{}", screen);
    assert!(screen.contains(" 1967│"), "числа выровнены вправо:\n{}", screen);
    // Вложенный объект развёрнут в столбцы (они правее, за краем окна).
    let w = app.desktop().group.children.last().unwrap().as_any().downcast_ref::<JsonWindow>().unwrap();
    assert!(w.data.headers.contains(&"издательство.город".to_string()));
    assert_eq!(w.data.rows.len(), 8);
}

#[test]
fn record_window_on_enter() {
    // Окно вставляем напрямую, а тестовый терминал сам «нажмёт» Esc,
    // когда ввод кончится, — и закроет окно записи.
    let mut app = program(vec![RawEvent::Key(kb::ENTER)]);
    let mut h = App { windows: 0 };
    let data = load(&sample_path()).unwrap();
    let r = app.desktop().group.vs.extent();
    app.insert_window(JsonWindow::new(r, &sample_path(), data, 1));
    // Шагаем, пока окно записи не откроется (оно модальное).
    let mut seen = String::new();
    for _ in 0..10 {
        app.step(&mut h);
        let t = app.screen_text();
        if t.contains("Запись 1") {
            seen = t;
            break;
        }
    }
    assert!(seen.contains("Запись 1"), "{}", app.screen_text());
    assert!(seen.contains("жанры"), "{}", seen);
}

#[test]
fn record_after_sort() {
    // После сортировки по году первая строка — «Евгений Онегин» (1833),
    // и окно записи должно показать именно её, а не первую в файле.
    let mut app = program(vec![]);
    let mut h = App { windows: 0 };
    h.open(&mut app, &sample_path());
    let id = app.desktop().group.children.last().unwrap().vs().id;
    let w = app.find_mut::<JsonWindow>(id).unwrap();
    let year = w.data.headers.iter().position(|h| h == "год").unwrap();
    let tid = w.table;
    let t = w.win.group.find_mut::<Table>(tid).unwrap();
    assert!(t.sort_by(year, true));
    t.set_cursor(0, 0);
    let rec = w.current_record().unwrap();
    assert_eq!(w.data.rows[rec][1], "Евгений Онегин");
}

#[test]
fn bad_file_message() {
    let dir = std::env::temp_dir().join(format!("json-table-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("bad.json");
    std::fs::write(&p, "{\n  \"a\": [1, 2,\n}").unwrap();
    let e = load(&p).unwrap_err();
    assert!(e.contains("строка 3, столбец 1"), "{}", e);
    assert!(load(&dir.join("нет.json")).unwrap_err().contains("Не удалось прочитать"));
}
