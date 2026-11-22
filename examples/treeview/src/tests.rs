use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

fn key(k: Key) -> RawEvent {
    RawEvent::Key(KeyEvent::new(k, Mods::NONE))
}

fn text(s: &str) -> Vec<RawEvent> {
    s.chars().map(|c| key(Key::Char(c))).collect()
}

fn program(input: Vec<RawEvent>) -> Program {
    let mut b = HeadlessBackend::new(100, 30);
    b.input.extend(input);
    Program::new(Box::new(b), Some(menu()), Some(status()))
}

/// Каталог с файлами для теста.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("treeview-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("каталог/вложенный")).unwrap();
    std::fs::write(d.join("file10.txt"), "10").unwrap();
    std::fs::write(d.join("file2.txt"), "2").unwrap();
    std::fs::write(d.join("каталог/внутри.txt"), "привет").unwrap();
    d
}

fn row_texts(t: &TreeView) -> Vec<String> {
    (0..t.row_count()).map(|i| t.node(t.row_path(i).unwrap()).unwrap().text.clone()).collect()
}

#[test]
fn fs_tree_is_lazy_and_sorted() {
    let d = tmp("fs");
    let mut t = fs_tree(Rect::new(0, 0, 60, 20), &d, None, None);
    // Каталоги первыми, имена — в естественном порядке.
    assert_eq!(row_texts(&t)[1..], ["каталог", "file2.txt", "file10.txt"]);
    let dir = t.find_text("каталог").unwrap();
    assert!(t.node(&dir).unwrap().lazy, "каталог ещё не прочитан");
    t.expand(&dir);
    assert_eq!(t.node(&dir).unwrap().children.len(), 2);
    assert!(t.find_text("внутри.txt").is_some());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn names_compare_naturally() {
    let mut v = vec!["file10.txt", "File2.txt", "file1.txt", "a", "file02b"];
    v.sort_by(|a, b| name_cmp(a, b));
    assert_eq!(v, ["a", "file1.txt", "File2.txt", "file02b", "file10.txt"]);
}

#[test]
fn json_tree() {
    let j = parse_json(r#"{"имя": "Аня", "теги": ["a", "b"], "адрес": {"город": "Тула"}}"#).unwrap();
    let n = json_node("x.json", &j);
    assert_eq!(n.text, "x.json  (3)");
    assert_eq!(n.icon, Some('{'));
    assert_eq!(n.children[0].text, "имя: \"Аня\"");
    assert_eq!(n.children[1].icon, Some('['));
    assert_eq!(n.children[1].children[1].text, "[1]: \"b\"");
    assert_eq!(n.count(), 7);
}

#[test]
fn components_choice() {
    let (mut d, tid) = components_dialog();
    let t = d.win.group.find_mut::<TreeView>(tid).unwrap();
    assert_eq!(t.check_state(&[0]), CheckState::Partial);
    // Отметить «Документацию» целиком и снять «Примеры».
    t.toggle_check(&[2]);
    t.set_checked(&[1], false);
    assert_eq!(chosen(t), ["Библиотека / Ядро", "Библиотека / Виджеты", "Документация / Руководство", "Документация / Справочник"]);
}

#[test]
fn components_dialog_keys() {
    // Пробел снимает «Библиотеку» (она частично отмечена → отмечается целиком),
    // ↓ на «Ядро», пробел снимает его, Esc тестового терминала закрывает диалог.
    let (d, tid) = components_dialog();
    let mut app = program(vec![key(Key::Char(' ')), key(Key::Down), key(Key::Char(' '))]);
    let (_, d) = app.exec_dialog(d);
    let t = d.win.group.find::<TreeView>(tid).unwrap();
    assert_eq!(t.check_state(&[0, 0]), CheckState::Unchecked);
    assert_eq!(t.check_state(&[0, 2]), CheckState::Checked, "Справка отмечена вместе с разделом");
    assert_eq!(t.check_state(&[0]), CheckState::Partial);
}

#[test]
fn plan_editing() {
    // F7 на корне: новый вложенный узел «Итоги»; потом переименовать в «Отчёт».
    let mut input = text("Итоги");
    input.push(key(Key::Enter));
    let mut app = program(input);
    let mut h = App { windows: 0, root: PathBuf::from(".") };
    let r = app.desktop().group.vs.extent();
    h.open_plan(&mut app, r);
    app.put_event(Event::command_event(CM_ADD_CHILD));
    app.step(&mut h);
    let w = App::active(&mut app).unwrap();
    assert_eq!(w.tree().current().unwrap().text, "Итоги");
    assert_eq!(w.tree().cursor_path(), Some(vec![0, 3]));

    // Переименование. Ввод для окна input_box кладём в очередь событий
    // программы после команды; текст поля выделен целиком — набор заменяет его.
    app.put_event(Event::command_event(CM_RENAME));
    for e in text("Отчёт").into_iter().chain([key(Key::Enter)]) {
        if let RawEvent::Key(k) = e {
            app.put_event(Event::KeyDown(k));
        }
    }
    app.step(&mut h);
    let w = App::active(&mut app).unwrap();
    assert_eq!(w.tree().current().unwrap().text, "Отчёт");

    // Удаление листа — без вопроса.
    app.put_event(Event::command_event(CM_DELETE));
    app.step(&mut h);
    let w = App::active(&mut app).unwrap();
    assert!(w.tree().find_text("Отчёт").is_none());
    assert_eq!(w.tree().current().unwrap().text, "После переезда");
}

#[test]
fn files_window_info_follows_cursor() {
    let d = tmp("info");
    let mut app = program(vec![key(Key::End)]);
    let mut h = App { windows: 0, root: d.clone() };
    let r = app.desktop().group.vs.extent();
    h.open_files(&mut app, r);
    app.step(&mut h);
    app.redraw();
    let screen = app.screen_text();
    assert!(screen.contains("file10.txt"), "{}", screen);
    // Полный путь — с разделителем каталогов этой системы (на Windows «\»).
    let sep = std::path::MAIN_SEPARATOR;
    assert!(screen.contains("file10.txt\n") || screen.contains(&format!("{}file10.txt", sep)), "{}", screen);
    assert!(screen.contains("file10.txt — 2 байт"), "сведения о последнем файле:\n{}", screen);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn start_screen() {
    let mut app = program(vec![]);
    let mut h = App { windows: 0, root: tmp("start") };
    open_start_windows(&mut app, &mut h);
    app.redraw();
    let s = app.screen_text();
    assert!(s.contains("План") && s.contains("Файлы"), "{}", s);
    assert!(s.contains("▾ Переезд в новый офис"), "{}", s);
    assert!(s.contains("├▸ Переезд"), "{}", s);
}
