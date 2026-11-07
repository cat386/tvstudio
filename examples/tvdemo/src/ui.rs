// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «tvdemo»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `tvdemo.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_ABOUT: u16 = 100;
    pub const CM_PUZZLE: u16 = 101;
    pub const CM_CALENDAR: u16 = 102;
    pub const CM_ASCII: u16 = 103;
    pub const CM_CALC: u16 = 104;
    pub const CM_OPEN: u16 = 105;
    pub const CM_CH_DIR: u16 = 106;
    pub const CM_MOUSE: u16 = 108;
    pub const CM_COLOR: u16 = 109;
    pub const CM_SAVE_DESKTOP: u16 = 110;
    pub const CM_RESTORE_DESKTOP: u16 = 111;
    pub const CM_EVENT_VIEW: u16 = 112;
    pub const CM_CH_BACKGROUND: u16 = 113;
    pub const CM_VIDEO_MODE: u16 = 115;
    /// кнопка формы ch_background
    pub const CM_APPLY_BACKGROUND: u16 = 120;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu_key(
            "~≡~",
            KeyEvent::new(Key::Char(' '), Mods::ALT),
            Menu::new()
                .item("~A~bout...", cmd::CM_ABOUT)
                .help(8)
                .separator()
                .item("~V~ideo mode", cmd::CM_VIDEO_MODE)
                .separator()
                .item("~P~uzzle", cmd::CM_PUZZLE)
                .help(9)
                .item("Ca~l~endar", cmd::CM_CALENDAR)
                .help(10)
                .item("Ascii ~T~able", cmd::CM_ASCII)
                .help(11)
                .item("~C~alculator", cmd::CM_CALC)
                .help(12)
                .item_key("~E~vent Viewer", cmd::CM_EVENT_VIEW, KeyEvent::new(Key::Char('0'), Mods::ALT), "Alt-0"),
        )
        .help(7)
        .submenu(
            "~F~ile",
            Menu::new()
                .item_key("~O~pen...", cmd::CM_OPEN, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .help(14)
                .item("~C~hange Dir...", cmd::CM_CH_DIR)
                .help(15)
                .separator()
                .item("~D~OS Shell", cm::DOS_SHELL)
                .help(16)
                .item_key("E~x~it", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X")
                .help(17),
        )
        .help(13)
        .submenu(
            "~W~indows",
            Menu::new()
                .item_key("~R~esize/move", cm::RESIZE, KeyEvent::new(Key::F(5), Mods::CTRL), "Ctrl-F5")
                .help(19)
                .item_key("~Z~oom", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .help(20)
                .item_key("~N~ext", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .help(23)
                .item_key("~C~lose", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt-F3")
                .help(25)
                .item("~T~ile", cm::TILE)
                .help(21)
                .item("C~a~scade", cm::CASCADE)
                .help(22),
        )
        .help(18)
        .submenu(
            "~O~ptions",
            Menu::new()
                .item("~M~ouse...", cmd::CM_MOUSE)
                .help(27)
                .item("~C~olors...", cmd::CM_COLOR)
                .help(28)
                .item("~B~ackground...", cmd::CM_CH_BACKGROUND)
                .submenu(
                    "~D~esktop",
                    Menu::new()
                        .item("~S~ave desktop", cmd::CM_SAVE_DESKTOP)
                        .help(29)
                        .item("~R~etrieve desktop", cmd::CM_RESTORE_DESKTOP)
                        .help(30),
                ),
        )
        .help(26)
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
    ])]
}

/// Действия команд из модели. Возвращает `true`, если команда обработана.
pub fn handle_command(app: &mut Program, command: u16) -> bool {
    match command {
        _ => false,
    }
}

/// Настройка до создания программы.
pub fn setup() {
}

/// Действия сразу после запуска (стартовая форма).
pub fn startup(app: &mut Program) {
    let _ = app;
}
