// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «learn_02_menu»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `learn_02_menu.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_HELLO: u16 = 100;
    pub const CM_ABOUT: u16 = 101;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~П~ривет", cmd::CM_HELLO, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~С~правка",
            Menu::new()
                .item("~О~ программе", cmd::CM_ABOUT),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F2~ Привет", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_HELLO),
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
    // Русские надписи стандартных диалогов (Да/Нет/Отмена, файлы...).
    locale::set_language(locale::Language::Ru);
}

/// Действия сразу после запуска (стартовая форма).
pub fn startup(app: &mut Program) {
    let _ = app;
}
