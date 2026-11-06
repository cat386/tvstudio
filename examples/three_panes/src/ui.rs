// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «three_panes»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `three_panes.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~О~кно",
            Menu::new()
                .item_key("Р~а~спахнуть", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item_key("~Р~азмер/перемещение", cm::RESIZE, KeyEvent::new(Key::F(5), Mods::CTRL), "Ctrl-F5")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("~F5~ Распахнуть", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::CTRL), cm::RESIZE),
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
