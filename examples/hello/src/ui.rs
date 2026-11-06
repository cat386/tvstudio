// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «hello»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `hello.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    /// Своя команда приложения (`GreetThemCmd`).
    pub const GREET_THEM_CMD: u16 = 100;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~H~ello",
            Menu::new()
                .item_key("~G~reeting...", cmd::GREET_THEM_CMD, KeyEvent::new(Key::Char('G'), Mods::ALT), "Alt-G")
                .separator()
                .item_key("E~x~it", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Alt-X~ Exit", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
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
