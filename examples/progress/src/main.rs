//! Пример полосы выполнения (`ProgressBar`).
//!
//! Запуск: `cargo run --example progress`
//!
//! * F2 — «Копирование файлов»: модальный диалог, полоса и имя текущего
//!   файла обновляются из функции фоновой работы (`exec_dialog_idle`);
//!   кнопка «Отмена» прерывает копирование.
//! * F3 — «Поиск простых чисел»: настоящая работа в отдельном потоке.
//!   Поток присылает прогресс через канал (`std::sync::mpsc`), диалог его
//!   показывает; «Отмена» останавливает поток.
//! * F4 — «Виды полос»: немодальное окно со всеми вариантами — плавная,
//!   «блоками», с шаблоном надписи, со своими цветами и бегающий блок.
//!   Клавиши «+» и «-» меняют значение, окно можно оставить открытым.
//! * Alt+X — выход.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


/// Имена «файлов» для имитации копирования.
const FILES: [&str; 12] = [
    "readme.txt",
    "Cargo.toml",
    "src/main.rs",
    "src/view.rs",
    "src/group.rs",
    "docs/руководство.md",
    "фото/отпуск-001.jpg",
    "фото/отпуск-002.jpg",
    "музыка/трек-01.flac",
    "архив/2025.zip",
    "архив/2026.zip",
    "видео/презентация.mp4",
];

/// Размер «файла» в условных блоках (чтобы большие файлы копировались дольше).
fn file_blocks(i: usize) -> i64 {
    [1, 1, 2, 3, 3, 2, 6, 6, 8, 10, 12, 16][i]
}

// ----------------------------------------------------------------------
// F2: имитация копирования в модальном диалоге
// ----------------------------------------------------------------------

/// Состояние копирования между вызовами фоновой функции.
struct CopyJob {
    file: usize,
    block_in_file: i64,
    copied: i64,
}

/// Диалог «Копирование файлов». Возвращает (диалог, id полосы файла,
/// id общей полосы, id надписи с именем файла).
fn copy_dialog(total: i64) -> (Dialog, ViewId, ViewId, ViewId) {
    let mut d = Dialog::centered(52, 12, "Копирование файлов");
    let name = d.add(StaticText::new(Rect::new(3, 2, 49, 3), ""));
    let file_bar = d.add(ProgressBar::new(Rect::new(3, 3, 49, 4), 1));
    d.add(StaticText::new(Rect::new(3, 5, 49, 6), "Всего:"));
    let total_bar = d.add(ProgressBar::new(Rect::new(3, 6, 49, 7), total).with_caption("{value} из {max} блоков — {percent}%"));
    let cancel = d.add(Button::new(Rect::new(20, 8, 32, 10), "Отмена", cm::CANCEL, bf::DEFAULT));
    d.select(cancel);
    (d, file_bar, total_bar, name)
}

fn copy_files(app: &mut Program) {
    let (cmd, copied) = run_copy(app);
    let msg = if cmd == cm::OK {
        format!("\x03Скопировано файлов: {}.", copied)
    } else {
        format!("\x03Копирование прервано.\n\x03Скопировано файлов: {}.", copied)
    };
    message_box(app, &msg, mf::INFORMATION | mf::OK_BUTTON);
}

/// Само копирование: возвращает код закрытия диалога и число скопированных файлов.
fn run_copy(app: &mut Program) -> (u16, usize) {
    let total: i64 = (0..FILES.len()).map(file_blocks).sum();
    let (d, file_bar, total_bar, name) = copy_dialog(total);
    let mut job = CopyJob { file: 0, block_in_file: 0, copied: 0 };

    // Один вызов — один скопированный блок.
    let (cmd, _) = app.exec_dialog_idle(d, |app, _| {
        if job.file >= FILES.len() {
            return Some(cm::OK);
        }
        let blocks = file_blocks(job.file);
        job.block_in_file += 1;
        job.copied += 1;
        if let Some(t) = app.find_mut::<StaticText>(name) {
            t.text = format!("{}  ({}/{})", FILES[job.file], job.file + 1, FILES.len());
        }
        if let Some(b) = app.find_mut::<ProgressBar>(file_bar) {
            b.set_range(0, blocks);
            b.set_value(job.block_in_file);
        }
        if let Some(b) = app.find_mut::<ProgressBar>(total_bar) {
            b.set_value(job.copied);
        }
        if job.block_in_file >= blocks {
            job.file += 1;
            job.block_in_file = 0;
        }
        None
    });

    (cmd, job.file)
}

// ----------------------------------------------------------------------
// F3: работа в отдельном потоке
// ----------------------------------------------------------------------

/// Сообщения от рабочего потока.
enum Work {
    /// Проверено чисел, найдено простых.
    Progress(u64, u64),
    /// Готово: найдено простых.
    Done(u64),
}

/// Верхняя граница поиска.
const PRIME_LIMIT: u64 = 3_000_000;

/// Считает простые числа до `limit`, присылая прогресс каждые 50 000 чисел.
fn count_primes(limit: u64, tx: mpsc::Sender<Work>, stop: Arc<AtomicBool>) {
    let mut found = 0u64;
    for n in 2..=limit {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if is_prime(n) {
            found += 1;
        }
        if n % 50_000 == 0 && tx.send(Work::Progress(n, found)).is_err() {
            return;
        }
    }
    let _ = tx.send(Work::Done(found));
}

fn is_prime(n: u64) -> bool {
    if n < 4 {
        return n >= 2;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

fn find_primes(app: &mut Program, limit: u64) {
    let msg = match run_primes(app, limit) {
        Some(found) => format!("\x03До {} найдено\n\x03{} простых чисел.", limit, found),
        None => "\x03Поиск остановлен.".to_string(),
    };
    message_box(app, &msg, mf::INFORMATION | mf::OK_BUTTON);
}

/// Поиск в потоке с диалогом прогресса. `None` — поиск отменён.
fn run_primes(app: &mut Program, limit: u64) -> Option<u64> {
    let mut d = Dialog::centered(50, 10, "Поиск простых чисел");
    let info = d.add(StaticText::new(Rect::new(3, 2, 47, 3), "Запуск потока…"));
    // Пока поток не прислал первое сообщение — бегающий блок.
    let bar = d.add(ProgressBar::new(Rect::new(3, 4, 47, 5), limit as i64).indeterminate());
    let cancel = d.add(Button::new(Rect::new(19, 6, 31, 8), "Отмена", cm::CANCEL, bf::DEFAULT));
    d.select(cancel);

    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let stop = stop.clone();
        thread::spawn(move || count_primes(limit, tx, stop))
    };

    let mut result = None;
    let (cmd, _) = app.exec_dialog_idle(d, |app, _| {
        // Забираем всё, что поток успел прислать.
        let mut last = None;
        for msg in rx.try_iter() {
            match msg {
                Work::Progress(n, found) => last = Some((n, found)),
                Work::Done(found) => {
                    result = Some(found);
                    return Some(cm::OK);
                }
            }
        }
        if let Some((n, found)) = last {
            if let Some(b) = app.find_mut::<ProgressBar>(bar) {
                b.indeterminate = false;
                b.set_value(n as i64);
            }
            if let Some(t) = app.find_mut::<StaticText>(info) {
                t.text = format!("Проверено: {}, найдено простых: {}", n, found);
            }
        }
        None
    });
    stop.store(true, Ordering::Relaxed);
    let _ = worker.join();
    if cmd == cm::OK {
        result
    } else {
        None
    }
}

// ----------------------------------------------------------------------
// F4: окно со всеми видами полос
// ----------------------------------------------------------------------

/// Немодальное окно-диалог: «+»/«-» меняют значение всех полос.
struct Gallery {
    dlg: Dialog,
    bars: Vec<ViewId>,
}

impl Gallery {
    fn new() -> Self {
        let mut d = Dialog::new(Rect::new(4, 2, 60, 20), "Виды полос");
        let mut bars = Vec::new();
        let mut row = |d: &mut Dialog, y: i32, label: &str, bar: ProgressBar| {
            d.add(StaticText::new(Rect::new(3, y, 53, y + 1), label));
            bars.push(d.add(bar));
        };
        let r = |y: i32| Rect::new(3, y + 1, 53, y + 2);
        row(&mut d, 2, "Плавная (по умолчанию):", ProgressBar::new(r(2), 100));
        row(&mut d, 5, "Блоками, как в DOS:", ProgressBar::new(r(5), 100).with_style(ProgressStyle::Blocks));
        row(&mut d, 8, "Шаблон надписи:", ProgressBar::new(r(8), 100).with_caption("Загружено {value} МБ из {max} МБ"));
        // Свои цвета: белый на красном поверх тёмно-серого.
        row(&mut d, 11, "Свои цвета:", ProgressBar::new(r(11), 100).with_colors(color::attr(7, 8), color::attr(15, 4)));
        // Бегающий блок: значение не важно, анимация идёт сама.
        d.add(StaticText::new(Rect::new(3, 14, 53, 15), "Неизвестная длительность:"));
        d.add(ProgressBar::new(r(14), 0).indeterminate());
        d.add(StaticText::new(Rect::new(3, 16, 53, 17), "\x03«+» / «-» — изменить значение, Esc — закрыть"));
        let mut g = Gallery { dlg: d, bars };
        g.change(37);
        g
    }

    fn change(&mut self, delta: i64) {
        for &id in &self.bars {
            if let Some(b) = self.dlg.win.group.find_mut::<ProgressBar>(id) {
                b.step(delta);
            }
        }
    }

    #[allow(dead_code)]
    fn value(&self) -> i64 {
        self.dlg.win.group.find::<ProgressBar>(self.bars[0]).map(|b| b.value()).unwrap_or(0)
    }
}

impl View for Gallery {
    tvision::impl_group_basics!(dlg.win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.dlg.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.dlg.draw(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.dlg.palette()
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.dlg.set_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        if let Event::KeyDown(k) = ev {
            match k.key {
                Key::Char('+') | Key::Right => {
                    self.change(5);
                    ev.clear();
                    return;
                }
                Key::Char('-') | Key::Left => {
                    self.change(-5);
                    ev.clear();
                    return;
                }
                // Немодальное окно: Esc закрывает его.
                Key::Esc => {
                    ctx.close(self.dlg.id());
                    ev.clear();
                    return;
                }
                _ => {}
            }
        }
        self.dlg.handle_event(ev, ctx);
    }
}

// ----------------------------------------------------------------------
// Приложение
// ----------------------------------------------------------------------

struct App;

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_COPY) => copy_files(app),
            Some(CM_PRIMES) => find_primes(app, PRIME_LIMIT),
            Some(CM_GALLERY) => {
                app.insert_window(Gallery::new());
            }
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
    app.insert_window(Gallery::new());
    app.run(&mut App);
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
