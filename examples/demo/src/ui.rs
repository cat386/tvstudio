// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «demo»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `demo.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_DIALOG: u16 = 100;
    pub const CM_LIST: u16 = 101;
    pub const CM_ABOUT: u16 = 102;
    pub const CM_ASK_NAME: u16 = 103;
    pub const CM_OUTLINE: u16 = 104;
    pub const CM_COLORS: u16 = 105;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item_key("~Н~овое окно", cm::NEW, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~С~писок", cmd::CM_LIST, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~Д~иалоги",
            Menu::new()
                .item_key("~Н~астройки...", cmd::CM_DIALOG, KeyEvent::new(Key::Char('D'), Mods::ALT), "Alt-D")
                .item("~В~вод строки...", cmd::CM_ASK_NAME)
                .item("~Ц~вета...", cmd::CM_COLORS)
                .submenu(
                    "~Е~щё",
                    Menu::new()
                        .item("~О~ программе...", cmd::CM_ABOUT),
                ),
        )
        .submenu(
            "~О~кна",
            Menu::new()
                .item("~Д~ерево каталогов", cmd::CM_OUTLINE)
                .separator()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE)
                .separator()
                .item_key("~Р~азмер/перемещение", cm::RESIZE, KeyEvent::new(Key::F(5), Mods::CTRL), "Ctrl-F5")
                .item_key("Р~а~спахнуть", cm::ZOOM, KeyEvent::new(Key::F(5), Mods::NONE), "F5")
                .item_key("~С~ледующее", cm::NEXT, KeyEvent::new(Key::F(6), Mods::NONE), "F6")
                .item_key("~З~акрыть", cm::CLOSE, KeyEvent::new(Key::F(3), Mods::ALT), "Alt-F3"),
        )
}

/// Строка состояния.
pub fn status_line() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![
        StatusItem::new("~F1~ Справка", KeyEvent::new(Key::F(1), Mods::NONE), cm::HELP),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("~F2~ Окно", KeyEvent::new(Key::F(2), Mods::NONE), cm::NEW),
        StatusItem::new("~F3~ Список", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_LIST),
        StatusItem::new("~Alt-D~ Диалог", KeyEvent::new(Key::Char('D'), Mods::ALT), cmd::CM_DIALOG),
        StatusItem::new("~F10~ Меню", KeyEvent::new(Key::F(10), Mods::NONE), cm::MENU),
        StatusItem::new("", KeyEvent::new(Key::F(3), Mods::ALT), cm::CLOSE),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::NONE), cm::ZOOM),
        StatusItem::new("", KeyEvent::new(Key::F(6), Mods::NONE), cm::NEXT),
        StatusItem::new("", KeyEvent::new(Key::F(5), Mods::CTRL), cm::RESIZE),
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
