//! Порт демонстрационной программы `tvdemo` из Turbo Vision.
//!
//! Запуск: `cargo run --example tvdemo [файлы...]`
//!
//! Что внутри (как в оригинале):
//! * меню «≡» (Alt+Пробел): About, Puzzle, Calendar, Ascii Table, Calculator,
//!   Event Viewer, Video mode;
//! * File: Open (просмотр файла), Change Dir, DOS Shell, Exit;
//! * Windows: размер/перемещение, Zoom, Next, Close, Tile, Cascade;
//! * Options: Mouse, Colors, Background, Desktop → Save / Retrieve;
//! * часы в строке меню и счётчик памяти в строке состояния;
//! * контекстная справка по F1 (исходник — `demohelp.txt` из оригинала).

mod ascii;
mod backgrnd;
mod calc;
mod calendar;
mod evntview;
mod fileview;
mod gadgets;
mod mousedlg;
mod puzzle;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// Команды (`tvcmds.h`).

/// Контексты справки (`demohelp.h`, генерирует `tvhc`).
#[allow(dead_code)]
mod hc {
    pub const NOCONTEXT: u16 = 0;
    pub const VIEWER: u16 = 2;
    pub const PUZZLE: u16 = 3;
    pub const CALCULATOR: u16 = 4;
    pub const CALENDAR: u16 = 5;
    pub const ASCII_TABLE: u16 = 6;
    pub const SYSTEM: u16 = 7;
    pub const S_ABOUT: u16 = 8;
    pub const S_PUZZLE: u16 = 9;
    pub const S_CALENDAR: u16 = 10;
    pub const S_ASCII_TABLE: u16 = 11;
    pub const S_CALCULATOR: u16 = 12;
    pub const FILE: u16 = 13;
    pub const F_OPEN: u16 = 14;
    pub const F_CHANGE_DIR: u16 = 15;
    pub const F_DOS_SHELL: u16 = 16;
    pub const F_EXIT: u16 = 17;
    pub const WINDOWS: u16 = 18;
    pub const W_SIZE_MOVE: u16 = 19;
    pub const W_ZOOM: u16 = 20;
    pub const W_TILE: u16 = 21;
    pub const W_CASCADE: u16 = 22;
    pub const W_NEXT: u16 = 23;
    pub const W_PREVIOUS: u16 = 24;
    pub const W_CLOSE: u16 = 25;
    pub const OPTIONS: u16 = 26;
    pub const O_MOUSE: u16 = 27;
    pub const O_COLORS: u16 = 28;
    pub const O_SAVE_DESKTOP: u16 = 29;
    pub const O_RESTORE_DESKTOP: u16 = 30;
    pub const FO_FILE_OPEN_DBOX: u16 = 31;
    pub const FC_CH_DIR_DBOX: u16 = 37;
    pub const OM_MOUSE_DBOX: u16 = 38;
    pub const OC_COLORS_DBOX: u16 = 39;
}

/// Файл сохранённого рабочего стола (`TVDEMO.DST`).
const DESKTOP_FILE: &str = "TVDEMO.DST";

/// Виды окон — для сохранения рабочего стола. В C++ это делали потоки
/// объектов (`TStreamable`); здесь программа сама записывает, какие окна открыты.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Puzzle,
    Calendar,
    Ascii,
    Calc,
    File(PathBuf),
}

struct TvDemo {
    help: Rc<HelpFile>,
    /// Открытые окна, которые умеем сохранять.
    kinds: HashMap<ViewId, Kind>,
    /// Окно событий, если открыто.
    event_viewer: Option<ViewId>,
    /// Номер следующего окна просмотра файла (`winNumber`).
    win_number: i16,
    /// Индекс палитры для «Video mode»: gruvbox → классическая цветная →
    /// ч/б → монохромная.
    video_mode: usize,
    /// Строки последней показанной темы справки (для тестов).
    last_help: Vec<String>,
}

impl TvDemo {
    fn insert(&mut self, app: &mut Program, w: Box<dyn View>, kind: Kind) -> ViewId {
        let id = app.insert_window_boxed(w);
        if id != 0 {
            self.kinds.insert(id, kind);
        }
        id
    }

    /// Окно «About» (`aboutDlgBox`).
    fn about(&mut self, app: &mut Program) {
        let mut d = Dialog::centered(39, 13, "About");
        d.add(StaticText::new(
            Rect::new(9, 2, 30, 9),
            "\x03Turbo Vision Demo\n\n\x03Rust Version\n\n\x03Copyright (c) 1994\n\n\x03Borland International",
        ));
        d.add(Button::new(Rect::new(14, 10, 26, 12), " OK", cm::OK, bf::DEFAULT));
        app.exec_dialog(d);
    }

    fn puzzle(&mut self, app: &mut Program) {
        let mut w = puzzle::puzzle_window();
        w.group.vs.help_ctx = hc::PUZZLE;
        self.insert(app, Box::new(w), Kind::Puzzle);
    }

    fn calendar(&mut self, app: &mut Program) {
        let mut w = calendar::calendar_window();
        w.group.vs.help_ctx = hc::CALENDAR;
        self.insert(app, Box::new(w), Kind::Calendar);
    }

    fn ascii_table(&mut self, app: &mut Program) {
        let mut w = ascii::AsciiChart::new();
        w.win.group.vs.help_ctx = hc::ASCII_TABLE;
        self.insert(app, Box::new(w), Kind::Ascii);
    }

    fn calculator(&mut self, app: &mut Program) {
        let mut c = calc::Calculator::new();
        c.dlg.win.group.vs.help_ctx = hc::CALCULATOR;
        self.insert(app, Box::new(c), Kind::Calc);
    }

    /// Окно событий (`eventViewer`): второй вызов — пауза/продолжение.
    fn event_viewer(&mut self, app: &mut Program) {
        if let Some(id) = self.event_viewer {
            if let Some(v) = app.find_mut::<evntview::EventViewer>(id) {
                v.toggle();
                return;
            }
        }
        let r = app.desktop().group.vs.extent();
        let id = app.insert_window(evntview::EventViewer::new(r, 0x0F00));
        app.event_monitor = Some(id);
        self.event_viewer = Some(id);
    }

    /// Окно просмотра файла (`TFileWindow`).
    fn file_window(&mut self, app: &mut Program, file: &Path) {
        let r = app.desktop().group.vs.extent();
        let mut w = fileview::file_window(r, file, self.win_number);
        self.win_number += 1;
        w.group.vs.help_ctx = hc::VIEWER;
        // Не удалось прочитать файл — сообщение вместо окна (`validView`).
        let err = w.group.children.iter().find_map(|c| c.as_any().downcast_ref::<fileview::FileViewer>()).and_then(|f| f.error.clone());
        match err {
            Some(e) => {
                message_box_rect(app, Rect::new(0, 0, 50, 10), &e, mf::ERROR | mf::OK_BUTTON, true);
            }
            None => {
                self.insert(app, Box::new(w), Kind::File(editor::absolute(file)));
            }
        }
    }

    /// «Открыть файл» (`openFile`).
    fn open_file(&mut self, app: &mut Program, spec: &str) {
        let mut d = FileDialog::new(spec, "Open a File", "~N~ame", fd::OPEN_BUTTON, 100);
        d.dlg.win.group.vs.help_ctx = hc::FO_FILE_OPEN_DBOX;
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            let name = d.file_name();
            self.file_window(app, &name);
        }
    }

    fn change_dir(&mut self, app: &mut Program) {
        let mut d = ChDirDialog::new(cd::NORMAL, 0);
        d.dlg.win.group.vs.help_ctx = hc::FC_CH_DIR_DBOX;
        app.exec_dialog(d);
    }

    /// Настройки мыши (`mouse`).
    fn mouse(&mut self, app: &mut Program) {
        let mut d = mousedlg::MouseDialog::new(app.double_delay, app.reverse_buttons);
        d.dlg.win.group.vs.help_ctx = hc::OM_MOUSE_DBOX;
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            app.double_delay = d.delay;
            app.reverse_buttons = d.reverse();
        }
    }

    /// Цвета (`colors`) — группы как в оригинальном tvdemo.
    fn colors(&mut self, app: &mut Program) {
        let window = |base: u8, last: &'static str| -> Vec<(&'static str, u8)> {
            vec![
                ("Frame passive", base),
                ("Frame active", base + 1),
                ("Frame icons", base + 2),
                ("Scroll bar page", base + 3),
                ("Scroll bar icons", base + 4),
                (last, base + 5),
            ]
        };
        let mut puzzle = window(8, "Normal text");
        puzzle.push(("Highlighted text", 14));
        let mut calendar = window(16, "Normal text");
        calendar.push(("Current day", 22));
        let mut groups = vec![
            ColorGroup::new("Desktop", &[("Color", 1)]),
            ColorGroup::new(
                "Menus",
                &[("Normal", 2), ("Disabled", 3), ("Shortcut", 4), ("Selected", 5), ("Selected disabled", 6), ("Shortcut selected", 7)],
            ),
        ];
        // «Dialogs/Calc» — как стандартная группа диалогов.
        let dialogs = standard_color_groups().into_iter().find(|g| g.name == "Dialogs").unwrap();
        groups.push(ColorGroup { name: "Dialogs/Calc".into(), ..dialogs });
        groups.push(ColorGroup::new("Viewer", &window(8, "Text")));
        groups.push(ColorGroup::new("Puzzle", &puzzle));
        groups.push(ColorGroup::new("Calendar", &calendar));
        groups.push(ColorGroup::new("Ascii table", &window(24, "Text")));
        let mut d = ColorDialog::new(&app.palette, groups);
        d.dlg.win.group.vs.help_ctx = hc::OC_COLORS_DBOX;
        let (cmd, d) = app.exec_dialog(d);
        if cmd == cm::OK {
            app.palette = d.palette.clone();
        }
    }

    fn ch_background(&mut self, app: &mut Program) {
        let d = backgrnd::ChBackground::new(app.desktop().pattern());
        app.exec_dialog(d);
    }

    /// «Video mode»: в оригинале переключал 25/43 строки. В терминале число
    /// строк задаёт сам терминал, поэтому здесь переключаются палитры:
    /// gruvbox → классическая цветная → чёрно-белая → монохромная.
    fn video_mode(&mut self, app: &mut Program) {
        use color::palettes::{APP_BLACK_WHITE, APP_COLOR, APP_MONOCHROME, GRUVBOX};
        const MODES: [&[Attr]; 4] = [GRUVBOX, APP_COLOR, APP_BLACK_WHITE, APP_MONOCHROME];
        // Текущий режим узнаём по палитре: в 16-цветном терминале программа
        // начинает с классической палитры, а не с gruvbox.
        let cur = MODES.iter().position(|m| app.palette == *m).unwrap_or(self.video_mode);
        self.video_mode = (cur + 1) % MODES.len();
        app.palette = MODES[self.video_mode].to_vec();
    }

    /// Сохранить рабочий стол (`saveDesktop`): виды окон и их границы,
    /// снизу вверх, в текстовом файле.
    fn save_desktop(&mut self, app: &mut Program) {
        let mut s = String::new();
        for c in &app.desktop().group.children {
            let id = c.vs().id;
            if let Some(k) = self.kinds.get(&id) {
                let b = c.vs().bounds();
                let kind = match k {
                    Kind::Puzzle => "puzzle".to_string(),
                    Kind::Calendar => "calendar".to_string(),
                    Kind::Ascii => "ascii".to_string(),
                    Kind::Calc => "calc".to_string(),
                    Kind::File(p) => format!("file {}", p.display()),
                };
                s.push_str(&format!("{} {} {} {} {}\n", b.a.x, b.a.y, b.b.x, b.b.y, kind));
            }
        }
        if std::fs::write(DESKTOP_FILE, s).is_err() {
            message_box(app, &format!("Could not create {}.", DESKTOP_FILE), mf::OK_BUTTON | mf::ERROR);
        }
    }

    /// Восстановить рабочий стол (`retrieveDesktop`): закрыть окна и открыть
    /// сохранённые.
    fn retrieve_desktop(&mut self, app: &mut Program) {
        let text = match std::fs::read_to_string(DESKTOP_FILE) {
            Ok(t) => t,
            Err(_) => {
                message_box(app, "Could not find desktop file", mf::OK_BUTTON | mf::ERROR);
                return;
            }
        };
        // Закрываем все окна (`deskTop->forEach(closeView)`).
        let ids: Vec<ViewId> = app.desktop().group.children.iter().skip(1).map(|c| c.vs().id).collect();
        for id in ids {
            app.desktop_mut().group.remove(id);
        }
        self.kinds.clear();
        self.event_viewer = None;
        app.event_monitor = None;
        for line in text.lines() {
            let mut parts = line.splitn(5, ' ');
            let nums: Vec<i32> = (0..4).filter_map(|_| parts.next()?.parse().ok()).collect();
            let kind = parts.next().unwrap_or("");
            if nums.len() != 4 {
                message_box(app, "Error reading desktop file", mf::OK_BUTTON | mf::ERROR);
                return;
            }
            let r = Rect::new(nums[0], nums[1], nums[2], nums[3]);
            let before = self.kinds.len();
            match kind.split_once(' ').map(|(a, b)| (a, Some(b))).unwrap_or((kind, None)) {
                ("puzzle", _) => self.puzzle(app),
                ("calendar", _) => self.calendar(app),
                ("ascii", _) => self.ascii_table(app),
                ("calc", _) => self.calculator(app),
                ("file", Some(p)) => self.file_window(app, Path::new(p)),
                _ => {}
            }
            // Возвращаем сохранённые границы.
            if self.kinds.len() > before {
                if let Some(c) = app.desktop_mut().group.children.last_mut() {
                    c.change_bounds(r);
                }
            }
        }
    }

    /// Справка по контексту (F1).
    fn help(&mut self, app: &mut Program) {
        let ctx = app.help_ctx();
        let (_, w) = app.exec_dialog(HelpWindow::new(self.help.clone(), ctx));
        self.last_help = w.viewer().text_lines();
    }
}

impl AppHandler for TvDemo {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        let c = match ev.command() {
            Some(c) => c,
            None => return,
        };
        match c {
            CM_ABOUT => self.about(app),
            CM_PUZZLE => self.puzzle(app),
            CM_CALENDAR => self.calendar(app),
            CM_ASCII => self.ascii_table(app),
            CM_CALC => self.calculator(app),
            CM_EVENT_VIEW => self.event_viewer(app),
            CM_OPEN => self.open_file(app, "*"),
            CM_CH_DIR => self.change_dir(app),
            CM_MOUSE => self.mouse(app),
            CM_COLOR => self.colors(app),
            CM_CH_BACKGROUND => self.ch_background(app),
            CM_SAVE_DESKTOP => self.save_desktop(app),
            CM_RESTORE_DESKTOP => self.retrieve_desktop(app),
            CM_VIDEO_MODE => self.video_mode(app),
            cm::DOS_SHELL => app.dos_shell("Type EXIT to return..."),
            cm::HELP => self.help(app),
            _ => return,
        }
        ev.clear();
    }

    /// `TVDemo::idle`: «Мозаика» и «Каскад» доступны, только если есть окна для них.
    fn idle(&mut self, app: &mut Program) {
        let any = app.desktop().group.children.iter().any(|c| c.vs().options & of::TILEABLE != 0);
        app.ctx.set_command(cm::TILE, any);
        app.ctx.set_command(cm::CASCADE, any);
    }
}

fn menu() -> Menu {
    ui::menu_bar()
}

fn status() -> Vec<StatusDef> {
    vec![
        StatusDef::range(
            0,
            50,
            vec![
                StatusItem::new("~F1~ Help", kb::F1, cm::HELP),
                StatusItem::new("~Alt-X~ Exit", kb::ALT_X, cm::QUIT),
                StatusItem::new("", KeyEvent::new(Key::Del, Mods::SHIFT), cm::CUT),
                StatusItem::new("", KeyEvent::new(Key::Ins, Mods::CTRL), cm::COPY),
                StatusItem::new("", KeyEvent::new(Key::Ins, Mods::SHIFT), cm::PASTE),
                StatusItem::new("", kb::ALT_F3, cm::CLOSE),
                StatusItem::new("", kb::F10, cm::MENU),
                StatusItem::new("", kb::F5, cm::ZOOM),
                StatusItem::new("", kb::CTRL_F5, cm::RESIZE),
                StatusItem::new("", kb::F6, cm::NEXT),
            ],
        ),
        StatusDef::range(50, 0xFFFF, vec![StatusItem::new("Howdy", kb::F1, cm::HELP)]),
    ]
}

/// Создание программы: меню, строка состояния, часы и счётчик памяти
/// (конструктор `TVDemo`).
fn build(backend: Box<dyn platform::Backend>) -> (Program, TvDemo) {
    let mut app = Program::new(backend, Some(menu()), Some(status()));
    let size = app.ctx.screen_size;
    app.group.add(gadgets::ClockView::new(Rect::new(size.x - 9, 0, size.x, 1)));
    app.group.add(gadgets::HeapView::new(Rect::new(size.x - 13, size.y - 1, size.x, size.y)));
    let help = Rc::new(HelpFile::parse(include_str!("demohelp.txt")).expect("справка tvdemo"));
    let demo = TvDemo { help, kinds: HashMap::new(), event_viewer: None, win_number: 1, video_mode: 0, last_help: Vec::new() };
    (app, demo)
}

fn main() -> std::io::Result<()> {
    let (mut app, mut demo) = build(platform::default_backend()?);
    // Файлы из командной строки (шаблоны — через диалог открытия).
    for arg in std::env::args().skip(1) {
        let mut spec = arg.clone();
        if spec.ends_with('/') || spec.ends_with('\\') {
            spec.push('*');
        }
        if filedlg::is_wild(&spec) {
            demo.open_file(&mut app, &spec);
        } else {
            demo.file_window(&mut app, Path::new(&spec));
        }
    }
    app.run(&mut demo);
    Ok(())
}

#[cfg(test)]
mod tests;
