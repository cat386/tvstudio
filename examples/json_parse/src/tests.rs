use super::*;
use tvision::platform::HeadlessBackend;

/// Программа с одним окном-образцом, текст которого заменён на `text`.
fn program_with(text: &str) -> (Program, App) {
    let b = HeadlessBackend::new(100, 30);
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    let mut h = App { windows: 0 };
    h.open_window(&mut app, Path::new(""));
    App::active_editor(&mut app).unwrap().set_text(text);
    (app, h)
}

fn run_command(app: &mut Program, h: &mut App, cmd: u16) {
    app.put_event(Event::command_event(cmd));
    app.step(h);
}

#[test]
fn sample_is_valid() {
    let j = check(SAMPLE).unwrap();
    let s = Stats::of(&j);
    assert_eq!((s.objects, s.arrays, s.depth), (4, 3, 4), "{:?}", s);
    assert_eq!((s.bools, s.nulls), (1, 1));
    assert!(s.text().contains("объектов: 4"));
}

#[test]
fn error_position_is_zero_based() {
    // Пропущена запятая во второй строке перед "b".
    let p = check("{\"a\": 1\n \"b\": 2}").unwrap_err();
    assert_eq!(p.pos, Pos::new(1, 1));
    assert!(p.message.contains("строка 2, столбец 2"), "{}", p.message);
}

#[test]
fn check_moves_cursor_to_error() {
    // Сообщение об ошибке закроет Esc тестового терминала.
    let (mut app, mut h) = program_with("[1, 2,\n  3,\n  ]");
    run_command(&mut app, &mut h, CM_CHECK);
    let ed = App::active_editor(&mut app).unwrap();
    assert_eq!(ed.cursor(), Pos::new(2, 2));
}

#[test]
fn format_and_compact_with_undo() {
    let (mut app, mut h) = program_with(r#"{"a":[1,2],"b":{"c":null}}"#);
    run_command(&mut app, &mut h, CM_FORMAT);
    let text = App::active_editor(&mut app).unwrap().text();
    assert_eq!(text, "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": null\n  }\n}");

    run_command(&mut app, &mut h, CM_COMPACT);
    assert_eq!(App::active_editor(&mut app).unwrap().text(), r#"{"a": [1, 2], "b": {"c": null}}"#);

    // Отмена возвращает отформатированный текст.
    App::active_editor(&mut app).unwrap().undo();
    assert!(App::active_editor(&mut app).unwrap().text().starts_with("{\n  \"a\""));
}

#[test]
fn broken_text_is_not_reformatted() {
    let (mut app, mut h) = program_with("[1, 2");
    run_command(&mut app, &mut h, CM_FORMAT);
    assert_eq!(App::active_editor(&mut app).unwrap().text(), "[1, 2");
}
