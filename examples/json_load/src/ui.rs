// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «json_load»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `json_load.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_OPEN_JSON: u16 = 100;
    /// Создать большой тестовый файл и загрузить его.
    pub const CM_GENERATE: u16 = 102;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~С~оздать тестовый файл...", cmd::CM_GENERATE, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~О~ткрыть JSON...", cmd::CM_OPEN_JSON, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F2~ Тестовый файл", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_GENERATE),
        StatusItem::new("~F3~ Открыть", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_OPEN_JSON),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
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
