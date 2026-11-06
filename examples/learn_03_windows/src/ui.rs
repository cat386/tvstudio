// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «learn_03_windows»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `learn_03_windows.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_NEW_WINDOW: u16 = 100;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~Н~овое окно", cmd::CM_NEW_WINDOW, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~О~кна",
            Menu::new()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE)
                .separator()
                .item_key("~С~ледующее", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .item_key("~Р~аспахнуть", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item_key("~З~акрыть", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt-F3"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F3~ Новое", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_NEW_WINDOW),
        StatusItem::new("~F6~ Следующее", KeyEvent::new(Key::F(6), Mods::NONE), cm::NEXT),
        StatusItem::new("~F5~ Распахнуть", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
        StatusItem::new("~Alt-F3~ Закрыть", KeyEvent::new(Key::F(3), Mods::ALT), cm::CLOSE),
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
