//! Строка меню и контекстные подсказки строки состояния TV Studio.

use tvision::*;

use crate::cmds::*;
use crate::designer::Align;
use crate::model::Kind;
use crate::ui_commands::MenuCommands;

use super::git_glue;

fn k(key: Key, mods: Mods) -> KeyEvent {
    KeyEvent::new(key, mods)
}

/// Строка меню среды.
pub fn menu_bar() -> Menu {
    let mut add = Menu::new();
    for (i, kd) in Kind::ALL.iter().enumerate() {
        add = add.item(kd.title(), CM_INS_BASE + i as u16);
    }
    let mut align = Menu::new();
    for (i, a) in Align::ALL.iter().enumerate() {
        align = align.item(a.title(), CM_ALIGN_BASE + i as u16);
    }
    Menu::new()
        .submenu(
            "~Ф~айл",
            Menu::new()
                .item("~Н~овый проект...", CM_NEW_PROJECT)
                .item("~О~ткрыть проект...", CM_OPEN_PROJECT)
                .item("~К~лонировать репозиторий...", CM_GIT_CLONE)
                .item("Н~е~давние проекты...", CM_RECENT)
                .item("~И~мпорт программы...", CM_IMPORT)
                .item("~З~акрыть проект", CM_CLOSE_PROJECT)
                .separator()
                .item("Новый ~ф~айл", cm::NEW)
                .command(cm::OPEN)
                .command(cm::SAVE)
                .item("Сохранить к~а~к...", cm::SAVE_AS)
                .command(cm::SAVE_ALL)
                .separator()
                .item("~Т~ерминал", cm::DOS_SHELL)
                .item("~П~араметры среды...", CM_OPTIONS)
                .item_key("В~ы~ход", cm::QUIT, kb::ALT_X, "Alt+X"),
        )
        .submenu(
            "~П~равка",
            Menu::new()
                .command(cm::UNDO)
                .command(cm::REDO)
                .separator()
                .command(cm::CUT)
                .command(cm::COPY)
                .command(cm::PASTE)
                .command(cm::CLEAR)
                .command(cm::SELECT_ALL),
        )
        .submenu(
            "~К~од",
            Menu::new()
                .command(cm::FIND)
                .command(cm::REPLACE)
                .command(cm::SEARCH_AGAIN)
                .separator()
                .command(CM_GOTO)
                .item("Найти в ~ф~айлах...", CM_FIND_FILES)
                .separator()
                .command(CM_COMPLETE)
                .command(CM_GOTO_DEF)
                .command(CM_FIND_REFS)
                .command(CM_NAV_BACK)
                .command(CM_CODE_ACTIONS)
                .submenu("Серверы ~я~зыка", Menu::new().item("~Д~иагностика", CM_DIAGNOSTICS).item("~П~ерезапустить серверы", CM_LSP_RESTART))
                .separator()
                .command(CM_NEXT_ERR)
                .command(CM_PREV_ERR),
        )
        .submenu(
            "Фо~р~ма",
            Menu::new()
                .item("~Н~овая форма...", CM_NEW_FORM)
                .item("~О~ткрыть форму...", CM_OPEN_FORM)
                .item("~У~далить форму и файл...", CM_REMOVE_FORM)
                .separator()
                .submenu("~Д~обавить элемент", add)
                .command(CM_DS_PROPS)
                .submenu("~В~ыровнять", align)
                .item("Раньше в порядке ~T~ab", CM_DS_BACKWARD)
                .item("Позже в порядке T~a~b", CM_DS_FORWARD)
                .item("В начало порядка", CM_DS_BACK)
                .item("В конец порядка", CM_DS_FRONT)
                .command(CM_DS_DUP)
                .separator()
                .item("Се~т~ка", CM_GRID)
                .item("Номера Ta~b~", CM_TABNUM)
                .separator()
                .command(CM_FORM_TEST)
                .command(CM_FORM_CODE),
        )
        .submenu(
            "Про~е~кт",
            Menu::new()
                .item("Строка ~м~еню...", CM_EDIT_MENU)
                .item("Строка ~с~остояния...", CM_EDIT_STATUS)
                .item("~К~оманды...", CM_EDIT_COMMANDS)
                .item("~П~араметры программы...", CM_APP_OPTIONS)
                .separator()
                .item("~Ф~ормы из кода", CM_EXTRACT_FORMS)
                .item("~Г~енерировать код", CM_GENERATE)
                .item("~Д~ерево проекта", CM_SHOW_PROJECT)
                .separator()
                .submenu("~G~it", git_glue::menu()),
        )
        .submenu(
            "~С~борка",
            Menu::new()
                .command(CM_CHECK)
                .command(CM_BUILD)
                .command(CM_RUN)
                .command(CM_DEBUG)
                .item("Параметры с~б~орки...", CM_BUILD_OPTS)
                .item("~Т~есты", CM_TEST)
                .item("~О~чистить сборку", CM_CLEAN)
                .separator()
                .command(CM_SHOW_PROBLEMS)
                .separator()
                .item("Остано~в~ить", CM_STOP),
        )
        .submenu(
            "~О~кно",
            Menu::new()
                .item("Компо~н~енты", CM_SHOW_PALETTE)
                .command(CM_SHOW_INSPECTOR)
                .item("П~р~оект", CM_SHOW_PROJECT)
                .item("~С~ообщения", CM_SHOW_MESSAGES)
                .command(CM_SHOW_PROBLEMS)
                .item("Раскладка по ~у~молчанию", CM_LAYOUT)
                .separator()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE)
                .item_key("Сле~д~ующее", cm::NEXT, kb::F6, "F6")
                .item_key("Пр~е~дыдущее", cm::PREV, kb::SHIFT_F6, "Shift+F6")
                .item_key("Размер/переме~щ~ение", cm::RESIZE, kb::CTRL_F5, "Ctrl+F5")
                .item_key("Р~а~звернуть", cm::ZOOM, kb::F5, "F5")
                .command(cm::CLOSE)
                .item("Закрыть ~в~се окна", cm::CLOSE_ALL),
        )
        .submenu("Спр~а~вка", Menu::new().item_key("~С~одержание", cm::HELP, kb::F1, "F1").item("~К~лавиши", CM_KEYS).item("~О~ TV Studio", CM_ABOUT))
}

fn hint_item(text: &str) -> StatusItem {
    StatusItem { text: text.into(), key: None, command: 0 }
}

/// Строка состояния: свои подсказки у дизайнера и редактора кода.
pub fn status_line() -> Vec<StatusDef> {
    let hidden = || {
        vec![
            StatusItem::new("", kb::F10, cm::MENU),
            StatusItem::new("", crate::ui_commands::spec(cm::CLOSE).key, cm::CLOSE),
            StatusItem::new("", kb::F5, cm::ZOOM),
            StatusItem::new("", kb::F6, cm::NEXT),
            StatusItem::new("", kb::SHIFT_F6, cm::PREV),
            StatusItem::new("", kb::CTRL_F5, cm::RESIZE),
            StatusItem::new("", kb::F1, cm::HELP),
        ]
    };
    let with = |mut v: Vec<StatusItem>| {
        v.extend(hidden());
        v
    };
    vec![
        StatusDef::range(
            HC_DESIGNER,
            HC_DESIGNER,
            with(vec![
                StatusItem::new("~Enter~ Свойства", kb::ENTER, CM_DS_PROPS),
                crate::ui_commands::status(cm::SAVE),
                crate::ui_commands::status(CM_FORM_TEST),
                crate::ui_commands::status(CM_FORM_CODE),
            ]),
        ),
        StatusDef::range(
            HC_EDITOR,
            HC_EDITOR,
            with(vec![
                crate::ui_commands::status(cm::SAVE),
                crate::ui_commands::status(CM_BUILD),
                crate::ui_commands::status(CM_GOTO_DEF),
                StatusItem::new("", crate::ui_commands::spec(CM_FIND_REFS).key, CM_FIND_REFS),
                StatusItem::new("", k(Key::Left, Mods::ALT), CM_NAV_BACK),
                StatusItem::new("", kb::ctrl_f(9), CM_RUN),
                StatusItem::new("", kb::alt('G'), CM_GOTO),
                StatusItem::new("~F8~ Ошибка", kb::f(8), CM_NEXT_ERR),
            ]),
        ),
        StatusDef::range(
            HC_INSPECTOR,
            HC_INSPECTOR,
            with(vec![
                hint_item("~Enter~ Изменить"),
                hint_item("~Пробел~ да/нет"),
                hint_item("~Esc~ Отмена"),
                StatusItem::new("~F10~ Меню", kb::F10, cm::MENU),
            ]),
        ),
        StatusDef::range(
            HC_PROBLEMS,
            HC_PROBLEMS,
            with(vec![
                hint_item("~Enter~ К месту"),
                hint_item("~Tab~ Фильтр"),
                StatusItem::new("~F9~ Собрать", kb::f(9), CM_BUILD),
                StatusItem::new("~Alt+F9~ Проверить", kb::alt_f(9), CM_CHECK),
                StatusItem::new("~Alt+F3~ Закрыть", kb::ALT_F3, cm::CLOSE),
            ]),
        ),
        StatusDef::range(
            HC_APP_MODEL,
            HC_BUILD,
            vec![
                hint_item("~Enter~ Подтвердить"),
                hint_item("~Esc~ Отмена"),
                hint_item("~Tab~ Поле"),
                StatusItem::new("~F1~ Справка", kb::F1, cm::HELP),
            ],
        ),
        StatusDef::range(
            HC_DIALOG,
            HC_PROPERTIES,
            vec![
                hint_item("~Enter~ Подтвердить"),
                hint_item("~Esc~ Отмена"),
                hint_item("~Tab~ Поле"),
                StatusItem::new("~F1~ Справка", kb::F1, cm::HELP),
            ],
        ),
        StatusDef::all(with(vec![
            StatusItem::new("~F1~ Справка", kb::F1, cm::HELP),
            StatusItem::new("~F9~ Собрать", kb::f(9), CM_BUILD),
            StatusItem::new("~Ctrl+F9~ Запустить", kb::ctrl_f(9), CM_RUN),
            StatusItem::new("~F10~ Меню", kb::F10, cm::MENU),
            StatusItem::new("~Alt+X~ Выход", kb::ALT_X, cm::QUIT),
        ])),
    ]
}