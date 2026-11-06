// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.
//! Каркас программы «treeview»: команды, меню, строка состояния, действия.
//! Источник — модель проекта `treeview.tvsp` (Проект → Модель приложения).
#![allow(dead_code, unused_imports, unused_variables)]

use tvision::*;

/// Свои команды программы. Номера 100..=255 можно запрещать
/// (`app.disable_command`), остальные разрешены всегда.
pub mod cmd {
    pub const CM_FILES: u16 = 100;
    pub const CM_PLAN: u16 = 101;
    pub const CM_OPEN_JSON: u16 = 102;
    pub const CM_SAMPLE_JSON: u16 = 103;
    pub const CM_COMPONENTS: u16 = 104;
    pub const CM_ADD_CHILD: u16 = 110;
    pub const CM_ADD_SIBLING: u16 = 111;
    pub const CM_RENAME: u16 = 112;
    pub const CM_DELETE: u16 = 113;
    pub const CM_EXPAND_ALL: u16 = 114;
    pub const CM_COLLAPSE_ALL: u16 = 115;
}

/// Строка меню.
pub fn menu_bar() -> Menu {
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item("~Ф~айлы", cmd::CM_FILES)
                .item("~П~лан", cmd::CM_PLAN)
                .item_key("~О~ткрыть JSON...", cmd::CM_OPEN_JSON, KeyEvent::new(Key::F(3), Mods::NONE), "F3")
                .item_key("Образец ~J~SON", cmd::CM_SAMPLE_JSON, KeyEvent::new(Key::F(4), Mods::NONE), "F4")
                .item_key("~К~омпоненты...", cmd::CM_COMPONENTS, KeyEvent::new(Key::F(9), Mods::NONE), "F9")
                .separator()
                .item_key("~В~ыход", cm::QUIT, KeyEvent::new(Key::Char('X'), Mods::ALT), "Alt-X"),
        )
        .submenu(
            "~Д~ерево",
            Menu::new()
                .item_key("Вложенный ~у~зел...", cmd::CM_ADD_CHILD, KeyEvent::new(Key::F(7), Mods::NONE), "F7")
                .item_key("Узел ~р~ядом...", cmd::CM_ADD_SIBLING, KeyEvent::new(Key::Ins, Mods::NONE), "Ins")
                .item_key("~П~ереименовать...", cmd::CM_RENAME, KeyEvent::new(Key::F(2), Mods::NONE), "F2")
                .item_key("~У~далить", cmd::CM_DELETE, KeyEvent::new(Key::Del, Mods::NONE), "Del")
                .separator()
                .item("Р~а~скрыть всё", cmd::CM_EXPAND_ALL)
                .item("~С~вернуть всё", cmd::CM_COLLAPSE_ALL),
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
        StatusItem::new("~F7~ Вложенный", KeyEvent::new(Key::F(7), Mods::NONE), cmd::CM_ADD_CHILD),
        StatusItem::new("~Ins~ Рядом", KeyEvent::new(Key::Ins, Mods::NONE), cmd::CM_ADD_SIBLING),
        StatusItem::new("~F2~ Имя", KeyEvent::new(Key::F(2), Mods::NONE), cmd::CM_RENAME),
        StatusItem::new("~Del~ Удалить", KeyEvent::new(Key::Del, Mods::NONE), cmd::CM_DELETE),
        StatusItem::new("~F3~ JSON", KeyEvent::new(Key::F(3), Mods::NONE), cmd::CM_OPEN_JSON),
        StatusItem::new("~F9~ Компоненты", KeyEvent::new(Key::F(9), Mods::NONE), cmd::CM_COMPONENTS),
        StatusItem::new("~Alt-X~ Выход", KeyEvent::new(Key::Char('X'), Mods::ALT), cm::QUIT),
        StatusItem::new("", KeyEvent::new(Key::F(4), Mods::NONE), cmd::CM_SAMPLE_JSON),
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
