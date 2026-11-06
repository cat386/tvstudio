// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «learn_08_todo»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `learn_08_todo.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_ADD: u16 = 100;
    pub const CM_DONE: u16 = 101;
    pub const CM_DELETE: u16 = 102;
    pub const CM_NEW_LIST: u16 = 103;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Ins~ Добавить", KeyEvent::new(Key::Ins, Mods::NONE), cmd::CM_ADD),
        StatusItem::new("~F4~ Сделано", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_DONE),
        StatusItem::new("~Del~ Удалить", KeyEvent::new(Key::Del, Mods::NONE), cmd::CM_DELETE),
        StatusItem::new("~F3~ Новый список", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_NEW_LIST),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(6), Mods::NONE), cm::NEXT),
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
    // Русские надписи стандартных диалогов (Да/Нет/Отмена, файлы...).
    locale::set_language(locale::Language::Ru);
}

/// Действия сразу после запуска (стартовая форма).
pub fn startup(app: &mut Program) {
    let _ = app;
}
