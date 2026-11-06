// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «json_table»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `json_table.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_OPEN_JSON: u16 = 100;
    /// Показать запись под курсором (F4; Enter и двойной щелчок делают то же).
    pub const CM_SHOW_RECORD: u16 = 101;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~О~ткрыть JSON...", cmd::CM_OPEN_JSON, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~З~апись...", cmd::CM_SHOW_RECORD, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~О~кна",
            Menu::new()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE)
                .separator()
                .item_key("Р~а~спахнуть", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item_key("~С~ледующее", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .item_key("~З~акрыть", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt-F3"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F3~ Открыть", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_OPEN_JSON),
        StatusItem::new("~F4~ Запись", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_SHOW_RECORD),
        StatusItem::new("~Alt-F3~ Закрыть", KeyEvent::new(Key::F(3), Mods::ALT), cm::CLOSE),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
        StatusItem::new("", KeyEvent::new(Key::F(6), Mods::NONE), cm::NEXT),
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
