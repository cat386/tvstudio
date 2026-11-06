// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «lfm»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `lfm.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_RENAME: u16 = 100;
    pub const CM_VIEW: u16 = 101;
    pub const CM_EDIT: u16 = 102;
    pub const CM_COPY: u16 = 103;
    pub const CM_MOVE: u16 = 104;
    pub const CM_MKDIR: u16 = 105;
    pub const CM_DELETE: u16 = 106;
    pub const CM_HELP_KEYS: u16 = 107;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F1~ Справка", KeyEvent::new(Key::F(1), Mods::NONE), cmd::CM_HELP_KEYS),
        StatusItem::new("~F2~ Имя", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_RENAME),
        StatusItem::new("~F3~ Просм", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_VIEW),
        StatusItem::new("~F4~ Правка", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_EDIT),
        StatusItem::new("~F5~ Копия", KeyEvent::new(Key::F(5), Mods::NONE), cmd::CM_COPY),
        StatusItem::new("~F6~ Перен", KeyEvent::new(Key::F(6), Mods::NONE), cmd::CM_MOVE),
        StatusItem::new("~F7~ Папка", KeyEvent::new(Key::F(7), Mods::NONE), cmd::CM_MKDIR),
        StatusItem::new("~F8~ Удал", KeyEvent::new(Key::F(8), Mods::NONE), cmd::CM_DELETE),
        StatusItem::new("~F9~ Меню", KeyEvent::new(Key::F(9), Mods::NONE), cm::MENU),
        StatusItem::new("~F10~ Выход", KeyEvent::new(Key::F(10), Mods::NONE), cm::QUIT),
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
