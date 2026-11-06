//! Пример: открыть файл JSON и показать данные в таблице.
//!
//! Запуск:
//! ```text
//! cargo run --example json_table                              # F3 — открыть файл
//! cargo run --example json_table -- examples/data/books.json  # сразу открыть файл
//! ```
//!
//! * F3 — диалог «Открыть JSON» (`FileDialog` с шаблоном `*.json`).
//! * Каждый файл открывается в своём окне с таблицей: номера строк,
//!   чередование цветов, сортировка щелчком по заголовку, ширина столбцов
//!   подобрана по данным, числа выровнены вправо.
//! * Enter, F4 или двойной щелчок по строке — окно «Запись» со всеми полями
//!   записи, вложенные объекты и массивы показаны целиком.
//! * Ошибка в файле — сообщение с номером строки и столбца.
//!
//! Файл читается сразу, без потока — для обычных файлов это мгновенно.
//! Загрузка больших файлов в потоке с полосой выполнения — в примере
//! `json_load`, свой разбор JSON — в `json_parse`. Как разные виды JSON
//! превращаются в таблицу — см. `examples/json/table.rs`.

use std::path::{Path, PathBuf};

use tvision::*;

#[path = "json/mod.rs"]
mod json;
use json::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


// ======================================================================
// Загрузка
// ======================================================================

/// Прочитать файл, разобрать JSON и построить таблицу. Ошибка — готовый
/// текст для сообщения.
pub fn load(path: &Path) -> Result<JsonTable, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Не удалось прочитать файл\n{}\n{}", path.display(), e))?;
    let text = String::from_utf8(bytes).map_err(|_| "Файл не в кодировке UTF-8.".to_string())?;
    let json = parse_json(&text).map_err(|e| format!("Ошибка в JSON:\n{}", e))?;
    Ok(to_table(&json))
}

// ======================================================================
// Окна
// ======================================================================

/// Окно с таблицей данных одного файла.
pub struct JsonWindow {
    pub win: Window,
    pub table: ViewId,
    pub data: JsonTable,
}

impl JsonWindow {
    pub fn new(bounds: Rect, path: &Path, data: JsonTable, number: i16) -> Self {
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let title = if data.source.is_empty() { name } else { format!("{} → {}", name, data.source) };
        let mut win = Window::new(bounds, &title, number);
        win.group.vs.options |= of::TILEABLE;
        let v = win.standard_scroll_bar(true);
        let h = win.standard_scroll_bar(false);
        let headers: Vec<&str> = data.headers.iter().map(|s| s.as_str()).collect();
        let model = VecTableModel::new(&headers, data.rows.clone());
        // Столбцы: числа — вправо, true/false — по центру.
        let cols = data
            .kinds
            .iter()
            .map(|k| {
                TableColumn::new(10).limits(3, 40).align(match k {
                    ColKind::Number => Align::Right,
                    ColKind::Bool => Align::Center,
                    ColKind::Text => Align::Left,
                })
            })
            .collect();
        let mut t = Table::new(win.interior(), Box::new(model), Some(v), Some(h))
            .with_columns(cols)
            .row_numbers(true)
            .striped(true)
            .sortable(true)
            .selection(SelectionMode::Row);
        // Ширина по первым 200 строкам.
        t.auto_width(200);
        let table = win.add(t);
        JsonWindow { win, table, data }
    }

    /// Номер записи под курсором (с учётом сортировки — по содержимому строки).
    fn current_record(&self) -> Option<usize> {
        let t = self.win.group.find::<Table>(self.table)?;
        let (row, _) = t.cursor();
        let m = t.model();
        if row >= m.row_count() {
            return None;
        }
        // После сортировки порядок строк в модели меняется — ищем исходную
        // запись по совпадению всех ячеек.
        let cells: Vec<String> = (0..m.col_count()).map(|c| m.cell(row, c)).collect();
        self.data.rows.iter().position(|r| *r == cells)
    }

    /// Окно «Запись» со всеми полями.
    fn show_record(&self, ctx: &mut Ctx) {
        let i = match self.current_record() {
            Some(i) => i,
            None => return,
        };
        let d = record_dialog(&self.data.records[i], i, ctx.screen_size);
        ctx.exec(Box::new(d), None, 0);
    }
}

impl View for JsonWindow {
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        // Enter — показать запись (таблица без правки Enter не использует).
        if let Event::KeyDown(k) = ev {
            if k.key == Key::Enter && k.mods == Mods::NONE && self.win.group.vs.is_focused() {
                self.show_record(ctx);
                ev.clear();
                return;
            }
        }
        // Команда из строки состояния / меню (F4).
        if ev.command() == Some(CM_SHOW_RECORD) {
            self.show_record(ctx);
            ev.clear();
            return;
        }
        // Двойной щелчок по строке: сначала таблица ставит курсор, потом — окно записи.
        let double = matches!(ev, Event::MouseDown(m) if m.double);
        self.win.handle_window_event(ev, ctx);
        if double {
            self.show_record(ctx);
        }
    }
}

/// Диалог «Запись N»: таблица «поле — значение», вложенное — JSON с отступами
/// (каждая строка JSON — отдельная строка таблицы).
pub fn record_dialog(rec: &Json, index: usize, screen: Point) -> Dialog {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut fields = Vec::new();
    match rec {
        Json::Obj(o) => flatten("", o, 3, &mut fields),
        other => fields.push(("значение".to_string(), other.clone())),
    }
    for (k, v) in fields {
        // Массив простых значений — одной строкой через запятую.
        let simple_arr = matches!(&v, Json::Arr(a) if a.iter().all(|x| !matches!(x, Json::Arr(_) | Json::Obj(_))));
        match v {
            Json::Arr(_) | Json::Obj(_) if !simple_arr => {
                for (i, line) in v.pretty().lines().enumerate() {
                    rows.push(vec![if i == 0 { k.clone() } else { String::new() }, line.to_string()]);
                }
            }
            other => rows.push(vec![k, other.cell_text()]),
        }
    }
    let w = (screen.x - 4).clamp(40, 76);
    // Строки таблицы + заголовок таблицы (2) + рамка и кнопка (7).
    let h = (rows.len() as i32 + 9).clamp(10, screen.y - 3);
    let mut d = Dialog::centered(w, h, &format!("Запись {}", index + 1));
    let sb = d.add(ScrollBar::new(Rect::new(w - 3, 2, w - 2, h - 4)));
    let model = VecTableModel::new(&["Поле", "Значение"], rows);
    let mut t = Table::new(Rect::new(2, 2, w - 3, h - 4), Box::new(model), Some(sb), None)
        .with_columns(vec![TableColumn::new(16).limits(4, 30), TableColumn::new(w - 24).limits(10, 200)])
        .grid(true)
        .selection(SelectionMode::Row);
    t.auto_width(500);
    // «Значение» занимает всё оставшееся место (граница между столбцами — 1 клетка).
    let table_w = w - 5;
    t.columns[1].width = (table_w - t.columns[0].width - 2).max(t.columns[1].width);
    let tid = d.add(t);
    d.add(Button::new(Rect::new(w / 2 - 6, h - 3, w / 2 + 6, h - 1), "O~K~", cm::OK, bf::DEFAULT));
    d.select(tid);
    d
}

// ======================================================================
// Приложение
// ======================================================================

struct App {
    windows: i16,
}

impl App {
    /// Открыть файл в новом окне (или показать ошибку).
    fn open(&mut self, app: &mut Program, path: &Path) -> bool {
        match load(path) {
            Ok(data) => {
                self.windows += 1;
                let r = app.desktop().group.vs.extent();
                let n = if self.windows < 10 { self.windows } else { 0 };
                app.insert_window(JsonWindow::new(r, path, data, n));
                true
            }
            Err(e) => {
                message_box_rect(app, Rect::new(0, 0, 60, 11), &e, mf::ERROR | mf::OK_BUTTON, true);
                false
            }
        }
    }

    /// Диалог выбора файла.
    fn open_dialog(&mut self, app: &mut Program) {
        let d = FileDialog::new("*.json", "Открыть JSON", "~И~мя", fd::OPEN_BUTTON, 100);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            let p: PathBuf = d.file_name();
            self.open(app, &p);
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_OPEN_JSON) => self.open_dialog(app),
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
        h.open(&mut app, Path::new(f));
    }
    if files.is_empty() {
        // Сразу предложить выбрать файл.
        app.put_event(Event::command_event(CM_OPEN_JSON));
    }
    app.run(&mut h);
    Ok(())
}

// ======================================================================
// Тесты: cargo test --example json_table
// ======================================================================

#[cfg(test)]
mod tests {
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
}
