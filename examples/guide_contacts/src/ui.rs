// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «guide_contacts»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `guide_contacts.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_ADD: u16 = 100;
    pub const CM_EDIT: u16 = 101;
    pub const CM_DELETE: u16 = 102;
    pub const CM_SORT: u16 = 103;
    pub const CM_EXIT: u16 = 104;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item("~Н~овый", cm::NEW)
                .item_key("~О~ткрыть...", cm::OPEN, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~С~охранить", cm::SAVE, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item("Сохранить ~к~ак...", cm::SAVE_AS)
                .separator()
                .item_key("~В~ыход", cmd::CM_EXIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~К~онтакт",
            Menu::new()
                .item_key("~Д~обавить...", cmd::CM_ADD, KeyEvent::new(Key::Ins, Mods::NONE), "Ins")
                .item_key("~И~зменить...", cmd::CM_EDIT, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .item_key("~У~далить", cmd::CM_DELETE, KeyEvent::new(Key::Del, Mods::NONE), "Del")
                .separator()
                .item("~С~ортировать по имени", cmd::CM_SORT),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Ins~ Добавить", KeyEvent::new(Key::Ins, Mods::NONE), cmd::CM_ADD),
        StatusItem::new("~F4~ Изменить", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_EDIT),
        StatusItem::new("~Del~ Удалить", KeyEvent::new(Key::Del, Mods::NONE), cmd::CM_DELETE),
        StatusItem::new("~F2~ Сохранить", KeyEvent::new(Key::F(2), Mods::NONE), cm::SAVE),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cmd::CM_EXIT),
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
