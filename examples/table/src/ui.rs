// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «table»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `table.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_BIG: u16 = 100;
    pub const CM_GOODS: u16 = 101;
    pub const CM_CITY: u16 = 102;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Т~аблицы",
            Menu::new()
                .item_key("~Б~ольшая таблица", cmd::CM_BIG, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~С~клад", cmd::CM_GOODS, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~В~ыбор города…", cmd::CM_CITY, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .separator()
                .item_key("В~ы~ход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt+X"),
        )
        .submenu(
            "~О~кна",
            Menu::new()
                .item_key("~С~ледующее", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .item_key("~Р~аспахнуть", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item_key("~З~акрыть", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt+F3")
                .separator()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Alt+X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("~F2~ Большая", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_BIG),
        StatusItem::new("~F3~ Склад", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_GOODS),
        StatusItem::new("~F4~ Город", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_CITY),
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
