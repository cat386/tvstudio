// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «colors»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `colors.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_PALETTE: u16 = 100;
    pub const CM_THEME: u16 = 101;
    pub const CM_COLORS: u16 = 102;
    pub const CM_MODE: u16 = 103;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ц~вета",
            Menu::new()
                .item_key("~П~алитра 256", cmd::CM_PALETTE, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~Т~ема: gruvbox/классика/сумерки", cmd::CM_THEME, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~Н~астройка цветов…", cmd::CM_COLORS, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .item_key("~Р~ежим вывода", cmd::CM_MODE, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .separator()
                .item_key("В~ы~ход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt+X"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~Alt+X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("~F2~ Палитра", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_PALETTE),
        StatusItem::new("~F3~ Тема", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_THEME),
        StatusItem::new("~F4~ Цвета", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_COLORS),
        StatusItem::new("~F5~ Режим", KeyEvent::new(Key::F(5), Mods::NONE), cmd::CM_MODE),
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
