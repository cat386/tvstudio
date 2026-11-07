//! Пример: загрузка большого файла JSON в отдельном потоке с диалогом
//! «Загрузка файла» и полосой выполнения (`ProgressBar`).
//!
//! Запуск:
//! ```text
//! cargo run --example json_load                              # F2 — тестовый файл, F3 — открыть
//! cargo run --example json_load -- examples/data/books.json  # сразу загрузить файл
//! ```
//!
//! * F2 — создать большой тестовый файл (по умолчанию 100 000 записей) и
//!   загрузить его: на маленьких файлах полосу почти не видно.
//! * F3 — выбрать файл (`FileDialog` с шаблоном `*.json`).
//! * Загрузка идёт в три этапа — чтение файла, разбор JSON, построение
//!   таблицы; у каждого своя часть общей полосы и своя подпись.
//! * «Отмена» (или Esc) останавливает поток посреди любого этапа.
//! * По окончании — сводка: размер, время, число записей и столбцов.
//!   Показ самой таблицы — в примере `json_table`.
//!
//! Как это устроено (готовый образец для любой долгой операции):
//! 1. поток выполняет [`load_with`] и шлёт ход работы через `std::sync::mpsc`;
//!    функция хода возвращает `false`, когда поднят флаг остановки;
//! 2. модальный диалог запускается через `Program::exec_dialog_idle`:
//!    функция простоя забирает сообщения потока, двигает полосу и закрывает
//!    диалог, когда пришёл результат;
//! 3. после диалога флаг остановки поднимается и поток дожидается (`join`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tvision::*;

#[path = "json/mod.rs"]
mod json;
use json::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


// ======================================================================
// Загрузка файла с ходом работы
// ======================================================================

/// Этап загрузки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Чтение файла: (прочитано байт, всего байт).
    Read,
    /// Разбор JSON: (разобрано символов, всего символов).
    Parse,
    /// Построение таблицы: (готово строк, всего строк × 2).
    Table,
}

impl Stage {
    /// Доля этапа в общей полосе: (начало, конец) в тысячных.
    fn span(self) -> (u64, u64) {
        match self {
            Stage::Read => (0, 300),
            Stage::Parse => (300, 850),
            Stage::Table => (850, 1000),
        }
    }

    /// Общий ход загрузки в тысячных (0–1000).
    pub fn permille(self, done: u64, total: u64) -> u64 {
        let (a, b) = self.span();
        let f = if total == 0 { 1.0 } else { (done as f64 / total as f64).min(1.0) };
        a + ((b - a) as f64 * f) as u64
    }
}

/// Результат неудачной загрузки.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// Пользователь нажал «Отмена».
    Cancelled,
    /// Ошибка чтения или разбора — текст для сообщения.
    Failed(String),
}

/// Прочитать и разобрать файл, сообщая о ходе работы:
/// `report(этап, сделано, всего)` → `false` прерывает загрузку.
pub fn load_with(path: &Path, report: &mut dyn FnMut(Stage, u64, u64) -> bool) -> Result<JsonTable, LoadError> {
    use std::io::Read;
    let fail = |e: std::io::Error| LoadError::Failed(format!("Не удалось прочитать файл\n{}\n{}", path.display(), e));

    // 1. Чтение кусками по 256 КБ — чтобы было что показывать на полосе.
    let mut f = std::fs::File::open(path).map_err(fail)?;
    let total = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        if !report(Stage::Read, bytes.len() as u64, total) {
            return Err(LoadError::Cancelled);
        }
        let n = f.read(&mut buf).map_err(fail)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8(bytes).map_err(|_| LoadError::Failed("Файл не в кодировке UTF-8.".into()))?;

    // 2. Разбор JSON.
    let mut hook = |done: usize, all: usize| report(Stage::Parse, done as u64, all as u64);
    let json = match parse_json_with(&text, Some(&mut hook)) {
        Ok(j) => j,
        Err(None) => return Err(LoadError::Cancelled),
        Err(Some(e)) => return Err(LoadError::Failed(format!("Ошибка в JSON:\n{}", e))),
    };
    drop(text);

    // 3. Построение таблицы.
    to_table_with(&json, &mut |done, all| report(Stage::Table, done as u64, all as u64)).ok_or(LoadError::Cancelled)
}

/// Сообщения от потока загрузки.
enum LoadMsg {
    Progress { stage: Stage, done: u64, total: u64 },
    Done(Result<JsonTable, LoadError>),
}

/// «12,3 МБ»
fn megabytes(b: u64) -> String {
    format!("{:.1} МБ", b as f64 / (1024.0 * 1024.0)).replace('.', ",")
}

/// «123 456» — разряды через пробел.
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Текст этапа под полосой.
fn stage_text(stage: Stage, done: u64, total: u64) -> String {
    match stage {
        Stage::Read => format!("Чтение файла: {} из {}", megabytes(done), megabytes(total)),
        Stage::Parse => format!("Разбор JSON: {}%", if total == 0 { 100 } else { done * 100 / total }),
        Stage::Table => format!("Построение таблицы: {} из {} записей", grouped(done.min(total) / 2), grouped(total / 2)),
    }
}

/// Загрузить файл в отдельном потоке, показывая модальный диалог с полосой
/// выполнения. Кнопка «Отмена» (или Esc) останавливает поток.
pub fn load_with_progress(app: &mut Program, path: &Path) -> Result<JsonTable, LoadError> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};

    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut d = Dialog::centered(58, 11, "Загрузка файла");
    d.add(StaticText::new(Rect::new(3, 2, 55, 3), &name));
    let stage = d.add(StaticText::new(Rect::new(3, 3, 55, 4), "Открытие…"));
    let bar = d.add(ProgressBar::new(Rect::new(3, 5, 55, 6), 1000));
    let cancel = d.add(Button::new(Rect::new(23, 7, 35, 9), "Отмена", cm::CANCEL, bf::DEFAULT));
    d.select(cancel);

    // Поток: загрузка с отчётами не чаще, чем раз в 1/1000 общего хода.
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let (path, stop, tx) = (path.to_path_buf(), stop.clone(), tx.clone());
        std::thread::spawn(move || {
            let mut last = (None, u64::MAX);
            let r = load_with(&path, &mut |st, done, total| {
                let pm = st.permille(done, total);
                // Отправляем при смене этапа или заметном продвижении.
                if last.0 != Some(st) || pm != last.1 {
                    last = (Some(st), pm);
                    let _ = tx.send(LoadMsg::Progress { stage: st, done, total });
                }
                !stop.load(Ordering::Relaxed)
            });
            let _ = tx.send(LoadMsg::Done(r));
        })
    };
    drop(tx);

    let mut result: Option<Result<JsonTable, LoadError>> = None;
    let (cmd, _) = app.exec_dialog_idle(d, |app, _| {
        // Забираем всё, что поток прислал с прошлого раза.
        let mut last = None;
        loop {
            match rx.try_recv() {
                Ok(LoadMsg::Progress { stage, done, total }) => last = Some((stage, done, total)),
                Ok(LoadMsg::Done(r)) => {
                    result = Some(r);
                    return Some(cm::OK);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // Поток завершился, не прислав результат (например, паника).
                Err(mpsc::TryRecvError::Disconnected) => {
                    result = Some(Err(LoadError::Failed("Загрузка прервалась из-за внутренней ошибки.".into())));
                    return Some(cm::OK);
                }
            }
        }
        if let Some((st, done, total)) = last {
            if let Some(b) = app.find_mut::<ProgressBar>(bar) {
                b.set_value(st.permille(done, total) as i64);
            }
            if let Some(t) = app.find_mut::<StaticText>(stage) {
                t.text = stage_text(st, done, total);
            }
        }
        None
    });
    stop.store(true, Ordering::Relaxed);
    let _ = worker.join();
    if cmd != cm::OK {
        return Err(LoadError::Cancelled);
    }
    result.unwrap_or(Err(LoadError::Cancelled))
}

// ======================================================================
// Тестовый файл
// ======================================================================

/// Записать большой файл JSON с `count` записями — чтобы увидеть полосу
/// выполнения на настоящем объёме (маленькие файлы грузятся мгновенно).
pub fn generate_sample(path: &Path, count: usize) -> std::io::Result<()> {
    use std::io::Write;
    const CITIES: [&str; 8] = ["Москва", "Санкт-Петербург", "Казань", "Новосибирск", "Екатеринбург", "Тула", "Самара", "Пермь"];
    const NAMES: [&str; 10] = ["Анна", "Борис", "Вера", "Глеб", "Дарья", "Егор", "Жанна", "Захар", "Ирина", "Кирилл"];
    const GOODS: [&str; 6] = ["чайник", "лампа", "книга", "кружка", "плед", "часы"];
    let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(w, "{{\n  \"описание\": \"Тестовые заказы\",\n  \"заказы\": [")?;
    // Простой генератор псевдослучайных чисел — данные одинаковые при каждом запуске.
    let mut x: u64 = 0x2545F4914F6CDD1D;
    let mut rnd = |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    for i in 0..count {
        let name = NAMES[rnd(10) as usize];
        let city = CITIES[rnd(8) as usize];
        let qty = 1 + rnd(9);
        let price = 100 + rnd(9900);
        let paid = rnd(3) != 0;
        let g1 = GOODS[rnd(6) as usize];
        let g2 = GOODS[rnd(6) as usize];
        let comma = if i + 1 < count { "," } else { "" };
        writeln!(
            w,
            "    {{\"номер\": {}, \"клиент\": \"{} №{}\", \"адрес\": {{\"город\": \"{}\", \"индекс\": \"{:06}\"}}, \"товары\": [\"{}\", \"{}\"], \"количество\": {}, \"сумма\": {}.{:02}, \"оплачен\": {}}}{}",
            i + 1,
            name,
            i + 1,
            city,
            100000 + rnd(900000),
            g1,
            g2,
            qty,
            price * qty,
            rnd(100),
            paid,
            comma
        )?;
    }
    writeln!(w, "  ]\n}}")?;
    w.flush()
}

// ======================================================================
// Сводка после загрузки
// ======================================================================

/// «2,35 с»
fn seconds(d: Duration) -> String {
    format!("{:.2} с", d.as_secs_f64()).replace('.', ",")
}

/// Текст сводки о загруженном файле. Список столбцов обрезается по `width`.
pub fn summary_text(path: &Path, data: &JsonTable, bytes: u64, elapsed: Duration, width: usize) -> String {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let source = if data.source.is_empty() { "весь файл".to_string() } else { format!("поле «{}»", data.source) };
    let mut cols = data.headers.join(", ");
    if cols.chars().count() > width {
        cols = cols.chars().take(width.saturating_sub(1)).collect::<String>() + "…";
    }
    format!(
        "Файл:     {}\nРазмер:   {}\nВремя:    {}\nЗаписи:   {} ({})\nСтолбцов: {}\n\n{}",
        name,
        megabytes(bytes),
        seconds(elapsed),
        grouped(data.rows.len() as u64),
        source,
        data.headers.len(),
        cols
    )
}

/// Диалог «Файл загружен».
pub fn summary_dialog(text: &str) -> Dialog {
    let mut d = Dialog::centered(60, 14, "Файл загружен");
    d.add(StaticText::new(Rect::new(3, 2, 57, 10), text));
    let ok = d.add(Button::new(Rect::new(24, 11, 36, 13), "O~K~", cm::OK, bf::DEFAULT));
    d.select(ok);
    d
}

// ======================================================================
// Приложение
// ======================================================================

struct App;

impl App {
    /// Загрузить файл с диалогом хода работы и показать сводку (или ошибку).
    /// Возвращает `true`, если файл загружен.
    fn open(&mut self, app: &mut Program, path: &Path) -> bool {
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let t0 = Instant::now();
        match load_with_progress(app, path) {
            Ok(data) => {
                let text = summary_text(path, &data, bytes, t0.elapsed(), 54);
                app.exec_dialog(summary_dialog(&text));
                true
            }
            // Отмену пользователь сделал сам — сообщать нечего.
            Err(LoadError::Cancelled) => false,
            Err(LoadError::Failed(e)) => {
                message_box_rect(app, Rect::new(0, 0, 60, 11), &e, mf::ERROR | mf::OK_BUTTON, true);
                false
            }
        }
    }

    /// Создать тестовый файл заданного размера во временном каталоге и загрузить.
    fn generate(&mut self, app: &mut Program) {
        let n = match input_box(app, "Тестовый файл", "~З~аписей:", "100000", 8) {
            Some(t) => match t.trim().replace(' ', "").parse::<usize>() {
                Ok(n) if n > 0 => n.min(2_000_000),
                _ => {
                    message_box(app, "\x03Нужно целое число больше нуля.", mf::ERROR | mf::OK_BUTTON);
                    return;
                }
            },
            None => return,
        };
        let path = std::env::temp_dir().join(format!("tvision-заказы-{}.json", n));
        if let Err(e) = generate_sample(&path, n) {
            message_box(app, &format!("Не удалось создать файл:\n{}", e), mf::ERROR | mf::OK_BUTTON);
            return;
        }
        self.open(app, &path);
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
            Some(CM_GENERATE) => self.generate(app),
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
    let mut h = App;
    let files: Vec<String> = std::env::args().skip(1).collect();
    for f in &files {
        h.open(&mut app, Path::new(f));
    }
    if files.is_empty() {
        // Сразу предложить создать тестовый файл.
        app.put_event(Event::command_event(CM_GENERATE));
    }
    app.run(&mut h);
    Ok(())
}

// ======================================================================
// Тесты: cargo test --example json_load
// ======================================================================

#[cfg(test)]
mod tests {
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
}
