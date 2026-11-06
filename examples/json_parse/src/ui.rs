// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «json_parse»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `json_parse.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    /// Проверить JSON в активном окне.
    pub const CM_CHECK: u16 = 100;
    /// Отформатировать с отступами.
    pub const CM_FORMAT: u16 = 101;
    /// Сжать в одну строку.
    pub const CM_COMPACT: u16 = 102;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~Н~овый (образец)", cm::NEW, KeyEvent::new(Key::Char('N'), Mods::CTRL), "Ctrl-N")
                .item_key("~О~ткрыть...", cm::OPEN, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("~С~охранить", cm::SAVE, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item("Сохранить ~к~ак...", cm::SAVE_AS)
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~J~SON",
            Menu::new()
                .item_key("~П~роверить", cmd::CM_CHECK, KeyEvent::new(Key::F(9), Mods::NONE), "F9")
                .item_key("~Ф~орматировать", cmd::CM_FORMAT, KeyEvent::new(Key::F(8), Mods::NONE), "F8")
                .item_key("~С~жать", cmd::CM_COMPACT, KeyEvent::new(Key::F(7), Mods::NONE), "F7"),
        )
        .submenu(
            "~П~равка",
            Menu::new()
                .item_key("~О~тменить", cm::UNDO, KeyEvent::new(Key::Char('U'), Mods::CTRL), "Ctrl-U")
                .item("~П~овторить", cm::REDO)
                .separator()
                .item_key("~В~ырезать", cm::CUT, KeyEvent::new(Key::Del, Mods::SHIFT), "Shift-Del")
                .item_key("~К~опировать", cm::COPY, KeyEvent::new(Key::Ins, Mods::CTRL), "Ctrl-Ins")
                .item_key("Вст~а~вить", cm::PASTE, KeyEvent::new(Key::Ins, Mods::SHIFT), "Shift-Ins"),
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
        StatusItem::new("~F9~ Проверить", KeyEvent::new(Key::F(9), Mods::NONE), cmd::CM_CHECK),
        StatusItem::new("~F8~ Форматировать", KeyEvent::new(Key::F(8), Mods::NONE), cmd::CM_FORMAT),
        StatusItem::new("~F7~ Сжать", KeyEvent::new(Key::F(7), Mods::NONE), cmd::CM_COMPACT),
        StatusItem::new("~F3~ Открыть", KeyEvent::new(Key::F(3), Mods::NONE), cm::OPEN),
        StatusItem::new("~F2~ Сохранить", KeyEvent::new(Key::F(2), Mods::NONE), cm::SAVE),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
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
}

/// Действия сразу после запуска (стартовая форма).
pub fn startup(app: &mut Program) {
    let _ = app;
}
