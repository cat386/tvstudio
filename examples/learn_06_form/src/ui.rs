// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «learn_06_form»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `learn_06_form.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_FORM: u16 = 100;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F2~ Анкета", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_FORM),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
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
