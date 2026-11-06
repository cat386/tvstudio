//! Пример: разбор JSON — проверка, форматирование и сжатие текста в редакторе.
//!
//! Запуск:
//! ```text
//! cargo run --example json_parse                              # окно с образцом
//! cargo run --example json_parse -- examples/data/books.json  # открыть файл
//! ```
//!
//! * F9 — проверить JSON: при ошибке курсор встаёт на место ошибки, в
//!   сообщении — строка, столбец и причина; без ошибок — сводка (сколько
//!   объектов, массивов, строк, чисел и какая глубина вложенности).
//! * F8 — отформатировать (отступ 2 пробела), F7 — сжать в одну строку.
//!   Обе правки отменяются, как обычные (Ctrl+U / «Отменить»).
//! * F3 — открыть файл, F2 — сохранить, Ctrl+N — новое окно с образцом.
//!
//! Сам разборщик — в `examples/json/parser.rs` (общий с примерами
//! `json_table` и `json_load`): свой, без внешних библиотек. Здесь показано,
//! как связать его с редактором: взять текст (`Editor::text`), поставить
//! курсор (`Editor::set_select` + `track_cursor`) и заменить текст с
//! сохранением отмены (`set_select` на весь текст + `insert_text`).

use std::path::{Path, PathBuf};

use tvision::*;

#[path = "json/mod.rs"]
mod json;
use json::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


/// Образец для нового окна. Можно испортить (убрать запятую или кавычку) и
/// нажать F9.
const SAMPLE: &str = r#"{"магазин": "Книжный на углу", "открыт": true, "рейтинг": 4.8,
"адрес": {"город": "Тула", "улица": "Советская", "дом": 12},
"книги": [{"название": "Мастер и Маргарита", "год": 1967, "жанры": ["роман", "мистика"]},
{"название": "Пикник на обочине", "год": 1972, "жанры": ["фантастика"]}],
"телефон": null}"#;

// ======================================================================
// Проверка и сводка
// ======================================================================

/// Сводка о разобранном JSON.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub objects: usize,
    pub arrays: usize,
    pub strings: usize,
    pub numbers: usize,
    pub bools: usize,
    pub nulls: usize,
    /// Наибольшая вложенность: у `1` — 0, у `[1]` — 1, у `{"a": [1]}` — 2.
    pub depth: usize,
}

impl Stats {
    /// Посчитать значения всех видов, обходя дерево рекурсивно.
    pub fn of(v: &Json) -> Stats {
        let mut s = Stats::default();
        s.visit(v, 0);
        s
    }

    fn visit(&mut self, v: &Json, level: usize) {
        self.depth = self.depth.max(level);
        match v {
            Json::Null => self.nulls += 1,
            Json::Bool(_) => self.bools += 1,
            Json::Num(_) => self.numbers += 1,
            Json::Str(_) => self.strings += 1,
            Json::Arr(a) => {
                self.arrays += 1;
                for x in a {
                    self.visit(x, level + 1);
                }
            }
            Json::Obj(o) => {
                self.objects += 1;
                for (_, x) in o {
                    self.visit(x, level + 1);
                }
            }
        }
    }

    /// Текст для сообщения.
    pub fn text(&self) -> String {
        format!(
            "\x03Ошибок нет.\n\nобъектов: {}   массивов: {}\nстрок: {}   чисел: {}\ntrue/false: {}   null: {}\nвложенность: {}",
            self.objects, self.arrays, self.strings, self.numbers, self.bools, self.nulls, self.depth
        )
    }
}

/// Ошибка, готовая для показа: позиция в редакторе (с 0) и текст.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub pos: Pos,
    pub message: String,
}

/// Разобрать текст. Позиция ошибки переводится в координаты редактора:
/// разборщик считает строки и столбцы с 1 (в символах), редактор — с 0.
pub fn check(text: &str) -> Result<Json, Problem> {
    parse_json(text).map_err(|e| Problem { pos: Pos::new(e.line - 1, e.col - 1), message: format!("Ошибка в JSON:\n{}", e) })
}

// ======================================================================
// Приложение
// ======================================================================

struct App {
    windows: i16,
}

impl App {
    /// Новое окно редактора: с файлом или (пустое имя) с образцом.
    fn open_window(&mut self, app: &mut Program, file: &Path) {
        self.windows += 1;
        let r = app.desktop().group.vs.extent();
        let mut w = EditWindow::new(r, file, if self.windows < 10 { self.windows } else { 0 });
        if file.as_os_str().is_empty() {
            w.editor_mut().editor.set_text(SAMPLE);
        }
        app.insert_window(w);
    }

    /// Редактор активного окна, если активно окно редактора.
    fn active_editor(app: &mut Program) -> Option<&mut Editor> {
        let w = app.desktop_mut().group.current_view_mut()?.as_any_mut().downcast_mut::<EditWindow>()?;
        Some(&mut w.editor_mut().editor)
    }

    /// Поставить курсор на место ошибки и показать сообщение.
    fn show_problem(app: &mut Program, p: Problem) {
        if let Some(ed) = Self::active_editor(app) {
            ed.set_select(p.pos, p.pos);
            ed.track_cursor(true);
        }
        message_box_rect(app, Rect::new(0, 0, 60, 10), &p.message, mf::ERROR | mf::OK_BUTTON, true);
    }

    /// F9: проверить.
    fn check_active(&mut self, app: &mut Program) {
        let text = match Self::active_editor(app) {
            Some(ed) => ed.text(),
            None => return,
        };
        match check(&text) {
            Ok(j) => {
                message_box_rect(app, Rect::new(0, 0, 46, 12), &Stats::of(&j).text(), mf::INFORMATION | mf::OK_BUTTON, true);
            }
            Err(p) => Self::show_problem(app, p),
        }
    }

    /// F8 / F7: заменить текст отформатированным или сжатым JSON.
    fn reformat_active(&mut self, app: &mut Program, pretty: bool) {
        let text = match Self::active_editor(app) {
            Some(ed) => ed.text(),
            None => return,
        };
        match check(&text) {
            Ok(j) => {
                let out = if pretty { j.pretty() } else { j.compact() };
                if let Some(ed) = Self::active_editor(app) {
                    // Выделить весь текст и вставить поверх: так правка
                    // попадает в историю отмены (в отличие от `set_text`).
                    let last = ed.line_count() - 1;
                    let end = Pos::new(last, ed.line(last).chars().count());
                    ed.set_select(Pos::new(0, 0), end);
                    ed.insert_text(&out, false);
                    ed.set_select(Pos::new(0, 0), Pos::new(0, 0));
                    ed.track_cursor(false);
                }
            }
            Err(p) => Self::show_problem(app, p),
        }
    }

    /// F3: открыть файл.
    fn open_dialog(&mut self, app: &mut Program) {
        let d = FileDialog::new("*.json", "Открыть JSON", "~И~мя", fd::OPEN_BUTTON, 100);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            let p: PathBuf = d.file_name();
            self.open_window(app, &p);
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_CHECK) => self.check_active(app),
            Some(CM_FORMAT) => self.reformat_active(app, true),
            Some(CM_COMPACT) => self.reformat_active(app, false),
            Some(cm::OPEN) => self.open_dialog(app),
            Some(cm::NEW) => self.open_window(app, Path::new("")),
            _ => return,
        }
        ev.clear();
    }
}

fn menu() -> Menu {
    ui::menu_bar()
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    let mut app = Program::with_terminal(Some(menu()), Some(status()))?;
    let mut h = App { windows: 0 };
    let files: Vec<String> = std::env::args().skip(1).collect();
    for f in &files {
        h.open_window(&mut app, Path::new(f));
    }
    if files.is_empty() {
        h.open_window(&mut app, Path::new(""));
    }
    app.run(&mut h);
    Ok(())
}

// ======================================================================
// Тесты: cargo test --example json_parse
// ======================================================================

#[cfg(test)]
mod tests {
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
}
