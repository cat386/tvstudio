// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «progress»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `progress.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_COPY: u16 = 100;
    pub const CM_PRIMES: u16 = 101;
    pub const CM_GALLERY: u16 = 102;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~П~римеры",
            Menu::new()
                .item_key("~К~опирование файлов", cmd::CM_COPY, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~П~ростые числа (поток)", cmd::CM_PRIMES, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~В~иды полос", cmd::CM_GALLERY, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~О~кна",
            Menu::new()
                .item_key("~З~акрыть", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt-F3")
                .item_key("~С~ледующее", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F2~ Копирование", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_COPY),
        StatusItem::new("~F3~ Поток", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_PRIMES),
        StatusItem::new("~F4~ Виды полос", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_GALLERY),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
        StatusItem::new("", KeyEvent::new(Key::F(3), Mods::ALT), cm::CLOSE),
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
