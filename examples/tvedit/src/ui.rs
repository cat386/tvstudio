// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «tvedit»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `tvedit.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_CHANGE_DRCT: u16 = 102;
    pub const CM_SHOW_CLIP: u16 = 105;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~F~ile",
            Menu::new()
                .item_key("~O~pen", cm::OPEN, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~N~ew", cm::NEW, KeyEvent::new(Key::Char('N'), Mods::CTRL), "Ctrl-N")
                .item_key("~S~ave", cm::SAVE, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item("S~a~ve as...", cm::SAVE_AS)
                .separator()
                .item("~C~hange dir...", cmd::CM_CHANGE_DRCT)
                .item("~D~OS shell", cm::DOS_SHELL)
                .item_key("E~x~it", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~E~dit",
            Menu::new()
                .item_key("~U~ndo", cm::UNDO, KeyEvent::new(Key::Char('U'), Mods::CTRL), "Ctrl-U")
                .item("~R~edo", cm::REDO)
                .separator()
                .item_key("Cu~t~", cm::CUT, KeyEvent::new(Key::Del, Mods::SHIFT), "Shift-Del")
                .item_key("~C~opy", cm::COPY, KeyEvent::new(Key::Ins, Mods::CTRL), "Ctrl-Ins")
                .item_key("~P~aste", cm::PASTE, KeyEvent::new(Key::Ins, Mods::SHIFT), "Shift-Ins")
                .item("~S~how clipboard", cmd::CM_SHOW_CLIP)
                .separator()
                .item_key("~C~lear", cm::CLEAR, KeyEvent::new(Key::Del, Mods::CTRL), "Ctrl-Del"),
        )
        .submenu(
            "~S~earch",
            Menu::new()
                .item("~F~ind...", cm::FIND)
                .item("~R~eplace...", cm::REPLACE)
                .item("~S~earch again", cm::SEARCH_AGAIN),
        )
        .submenu(
            "~W~indows",
            Menu::new()
                .item_key("~S~ize/move", cm::RESIZE, KeyEvent::new(Key::F(5), Mods::CTRL), "Ctrl-F5")
                .item_key("~Z~oom", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item("~T~ile", cm::TILE)
                .item("C~a~scade", cm::CASCADE)
                .item_key("~N~ext", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .item_key("~P~revious", cm::PREV, KeyEvent::new(Key::F(6), Mods::SHIFT), "Shift-F6")
                .item_key("~C~lose", cm::CLOSE, KeyEvent::new(Key::Char('W'), Mods::CTRL), "Ctrl-W"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("~F2~ Save", KeyEvent::new(Key::F(2), Mods::NONE), cm::SAVE),
        StatusItem::new("~F3~ Open", KeyEvent::new(Key::F(3), Mods::NONE), cm::OPEN),
        StatusItem::new("~Ctrl-W~ Close", KeyEvent::new(Key::F(3), Mods::ALT), cm::CLOSE),
        StatusItem::new("~F5~ Zoom", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
        StatusItem::new("~F6~ Next", KeyEvent::new(Key::F(6), Mods::NONE), cm::NEXT),
        StatusItem::new("~F10~ Menu", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
        StatusItem::new("", KeyEvent::new(Key::F(6), Mods::SHIFT), cm::PREV),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::CTRL), cm::RESIZE),
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
