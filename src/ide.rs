//! Среда TV Studio: меню, строка состояния и обработчик команд, который
//! связывает окна между собой (дизайнеры, инспектор, палитру, дерево
//! проекта, редакторы кода, сообщения) и управляет проектом и сборкой.
//!
//! Окна не знают друг о друге: они посылают команды, а обработчик
//! [`Ide`] (`AppHandler`) находит нужные окна на рабочем столе. Всё,
//! что должно происходить «само» (инспектор следит за выделением,
//! вывод cargo появляется в сообщениях), делается в `idle`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tvision::*;

use crate::appdesign::{LeKind, ListEditor};
use crate::builder::{build_dialog, describe_data};
use crate::cmds::*;
use crate::codeedit::CodeWindow;
use crate::codegen;
use crate::designer::{Align, DesignWindow, DocRef, FormDoc, Reorder};
use crate::dialogs::{self, input_box, message_box, message_box_rect, Config};
use crate::inspector::InspectorWindow;
use crate::lsp::LspClient;
use crate::model::{Action, Form, Kind, Project, FORM_EXT, PROJECT_EXT};
use crate::runner::{After, Job, JobMsg};
use crate::toolwins::{parse_cargo_line, MessagesWindow, MsgEntry, MsgKind, Problem, ProblemsWindow, ProjectWindow};

mod lsp_glue;
pub use lsp_glue::{LspState, Srv};

/// Текст справки (формат tvhc), встроен в программу.
pub const HELP_SOURCE: &str = include_str!("help.txt");

// ======================================================================
// Меню и строка состояния
// ======================================================================

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
                .item("Н~е~давние проекты...", CM_RECENT)
                .item("~И~мпорт программы...", CM_IMPORT)
                .item("~З~акрыть проект", CM_CLOSE_PROJECT)
                .separator()
                .item("Новый ~ф~айл", cm::NEW)
                .item_key("Открыть фа~й~л...", cm::OPEN, kb::f(3), "F3")
                .item_key("~С~охранить", cm::SAVE, kb::f(2), "F2")
                .item("Сохранить ~к~ак...", cm::SAVE_AS)
                .item_key("Сохранить ~в~сё", cm::SAVE_ALL, kb::shift_f(2), "Shift+F2")
                .separator()
                .item("~О~болочка ОС", cm::DOS_SHELL)
                .item_key("В~ы~ход", cm::QUIT, kb::ALT_X, "Alt+X"),
        )
        .submenu(
            "~П~равка",
            Menu::new()
                .item_key("~О~тменить", cm::UNDO, kb::ctrl('Z'), "Ctrl+Z")
                .item("~П~овторить", cm::REDO)
                .separator()
                .item_key("~В~ырезать", cm::CUT, k(Key::Del, Mods::SHIFT), "Shift+Del")
                .item_key("~К~опировать", cm::COPY, k(Key::Ins, Mods::CTRL), "Ctrl+Ins")
                .item_key("Вст~а~вить", cm::PASTE, k(Key::Ins, Mods::SHIFT), "Shift+Ins")
                .item_key("~У~далить", cm::CLEAR, k(Key::Del, Mods::CTRL), "Ctrl+Del")
                .item_key("Выделить ~в~сё", cm::SELECT_ALL, kb::ctrl('A'), "Ctrl+A"),
        )
        .submenu(
            "По~и~ск",
            Menu::new()
                .item("~Н~айти...", cm::FIND)
                .item("~З~аменить...", cm::REPLACE)
                .item_key("~И~скать дальше", cm::SEARCH_AGAIN, kb::ctrl('L'), "Ctrl+L")
                .separator()
                .item_key("~П~ерейти к строке...", CM_GOTO, kb::alt('G'), "Alt+G")
                .item("Найти в ~ф~айлах...", CM_FIND_FILES)
                .separator()
                .item_key("~С~ледующая ошибка", CM_NEXT_ERR, kb::f(8), "F8")
                .item_key("П~р~едыдущая ошибка", CM_PREV_ERR, kb::shift_f(8), "Shift+F8")
                .separator()
                .item_key("~Д~ополнить", CM_COMPLETE, k(Key::Char(' '), Mods::CTRL), "Ctrl+Space")
                .item_key("К ~о~пределению", CM_GOTO_DEF, kb::f(12), "F12")
                .item_key("Найти сс~ы~лки", CM_FIND_REFS, kb::shift_f(12), "Shift+F12")
                .item_key("Н~а~зад", CM_NAV_BACK, k(Key::Left, Mods::ALT), "Alt+←")
                .item_key("~Б~ыстрые исправления", CM_CODE_ACTIONS, k(Key::Enter, Mods::ALT), "Alt+Enter")
                .item("Д~и~агностика LSP", CM_DIAGNOSTICS)
                .item("Перезапустить сервер ~L~SP", CM_LSP_RESTART),
        )
        .submenu(
            "Фо~р~ма",
            Menu::new()
                .item("~Н~овая форма...", CM_NEW_FORM)
                .item("~О~ткрыть форму...", CM_OPEN_FORM)
                .item("~У~далить форму из проекта", CM_REMOVE_FORM)
                .separator()
                .submenu("~Д~обавить элемент", add)
                .item_key("~С~войства элемента...", CM_DS_PROPS, kb::ctrl_f(2), "Ctrl+F2")
                .submenu("~В~ыровнять", align)
                .item("Раньше в порядке ~T~ab", CM_DS_BACKWARD)
                .item("Позже в порядке T~a~b", CM_DS_FORWARD)
                .item("В начало порядка", CM_DS_BACK)
                .item("В конец порядка", CM_DS_FRONT)
                .item("Д~у~блировать", CM_DS_DUP)
                .separator()
                .item("Се~т~ка", CM_GRID)
                .item("~Н~омера Tab", CM_TABNUM)
                .separator()
                .item_key("~П~роверить форму", CM_FORM_TEST, kb::shift_f(9), "Shift+F9")
                .item_key("~К~од формы", CM_FORM_CODE, kb::f(12), "F12"),
        )
        .submenu(
            "Пр~о~ект",
            Menu::new()
                .item("~С~трока меню...", CM_EDIT_MENU)
                .item("Строка ~с~остояния...", CM_EDIT_STATUS)
                .item("~К~оманды...", CM_EDIT_COMMANDS)
                .item("~П~араметры программы...", CM_APP_OPTIONS)
                .separator()
                .item("~Ф~ормы из кода", CM_EXTRACT_FORMS)
                .item("~Г~енерировать код", CM_GENERATE)
                .item("~Д~ерево проекта", CM_SHOW_PROJECT),
        )
        .submenu(
            "~С~борка",
            Menu::new()
                .item_key("~П~роверить (cargo check)", CM_CHECK, kb::alt_f(9), "Alt+F9")
                .item_key("~С~обрать (cargo build)", CM_BUILD, kb::f(9), "F9")
                .item_key("~З~апустить (cargo run)", CM_RUN, kb::ctrl_f(9), "Ctrl+F9")
                .item("Параметры с~б~орки...", CM_BUILD_OPTS)
                .item("~Т~есты (cargo test)", CM_TEST)
                .item("~О~чистить (cargo clean)", CM_CLEAN)
                .separator()
                .item_key("О~ш~ибки и предупреждения", CM_SHOW_PROBLEMS, kb::ctrl_f(8), "Ctrl+F8")
                .separator()
                .item("О~с~тановить", CM_STOP),
        )
        .submenu(
            "~О~кно",
            Menu::new()
                .item("Компо~н~енты", CM_SHOW_PALETTE)
                .item_key("~И~нспектор", CM_SHOW_INSPECTOR, kb::f(11), "F11")
                .item("П~р~оект", CM_SHOW_PROJECT)
                .item("~С~ообщения", CM_SHOW_MESSAGES)
                .item_key("О~ш~ибки и предупреждения", CM_SHOW_PROBLEMS, kb::ctrl_f(8), "Ctrl+F8")
                .item("Р~а~складка по умолчанию", CM_LAYOUT)
                .separator()
                .item("~М~озаика", cm::TILE)
                .item("~К~аскад", cm::CASCADE)
                .item_key("Сле~д~ующее", cm::NEXT, kb::F6, "F6")
                .item_key("Пр~е~дыдущее", cm::PREV, kb::SHIFT_F6, "Shift+F6")
                .item_key("Ра~з~мер/перемещение", cm::RESIZE, kb::CTRL_F5, "Ctrl+F5")
                .item_key("Распа~х~нуть", cm::ZOOM, kb::F5, "F5")
                .item_key("~З~акрыть", cm::CLOSE, kb::ALT_F3, "Alt+F3")
                .item("Закрыть ~в~се", cm::CLOSE_ALL),
        )
        .submenu("~Н~астройки", Menu::new().item("~П~араметры среды...", CM_OPTIONS))
        .submenu("Спр~а~вка", Menu::new().item_key("~С~одержание", cm::HELP, kb::F1, "F1").item("~К~лавиши", CM_KEYS).item("~О~ TV Studio", CM_ABOUT))
}

/// Пункт-надпись без клавиши (подсказка; щелчок по нему ничего не делает).
fn hint_item(text: &str) -> StatusItem {
    StatusItem { text: text.into(), key: None, command: 0 }
}

/// Строка состояния: свои подсказки у дизайнера и редактора кода.
pub fn status_line() -> Vec<StatusDef> {
    let hidden = || {
        vec![
            StatusItem::new("", kb::F10, cm::MENU),
            StatusItem::new("", kb::ALT_F3, cm::CLOSE),
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
                hint_item("~Ins~ Добавить"),
                hint_item("~Del~ Удалить"),
                StatusItem::new("~Shift+F9~ Проверить", kb::shift_f(9), CM_FORM_TEST),
                StatusItem::new("~F12~ Код", kb::f(12), CM_FORM_CODE),
                StatusItem::new("~F2~ Сохр.", kb::F2, cm::SAVE),
                StatusItem::new("~F11~ Инспектор", kb::f(11), CM_SHOW_INSPECTOR),
            ]),
        ),
        StatusDef::range(
            HC_EDITOR,
            HC_EDITOR,
            with(vec![
                StatusItem::new("~F2~ Сохр.", kb::F2, cm::SAVE),
                StatusItem::new("~F9~ Сборка", kb::f(9), CM_BUILD),
                StatusItem::new("~F12~ Опред.", kb::f(12), CM_GOTO_DEF),
                StatusItem::new("~Shift+F12~ Ссылки", kb::shift_f(12), CM_FIND_REFS),
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
        StatusDef::all(with(vec![
            StatusItem::new("~F1~ Справка", kb::F1, cm::HELP),
            StatusItem::new("~F3~ Открыть", kb::f(3), cm::OPEN),
            StatusItem::new("~F9~ Собрать", kb::f(9), CM_BUILD),
            StatusItem::new("~Ctrl+F9~ Запуск", kb::ctrl_f(9), CM_RUN),
            StatusItem::new("~F10~ Меню", kb::F10, cm::MENU),
            StatusItem::new("~Alt+X~ Выход", kb::ALT_X, cm::QUIT),
        ])),
    ]
}

// ======================================================================
// Среда
// ======================================================================

/// Обработчик среды.
pub struct Ide {
    pub project: Option<Project>,
    pub cfg: Config,
    pub job: Option<Job>,
    pub help: Rc<HelpFile>,
    /// Тестовый режим: не сохранять настройки, не запускать программу в терминале.
    pub headless: bool,
    /// Окно «Инспектор»: компоненты и свойства.
    pub inspector: Option<ViewId>,
    pub project_win: Option<ViewId>,
    pub messages: Option<ViewId>,
    /// Окно «Ошибки и предупреждения».
    pub problems: Option<ViewId>,
    /// Замечания с местом из вывода последней сборки.
    build_diags: Vec<MsgEntry>,
    /// Пора пересобрать список окна ошибок.
    problems_dirty: bool,
    /// Пометки ошибок последней сборки по файлам.
    marks: HashMap<PathBuf, Vec<(usize, MsgKind)>>,
    win_no: i16,
    tree_dirty: bool,
    /// Последняя ошибка (в тестовом режиме ошибки не показываются окном).
    pub last_error: Option<String>,
    /// Сервер LSP (rust-analyzer) открытого проекта.
    pub lsp: Option<LspClient>,
    /// Словарь проекта для дополнения без сервера: (когда собран, каталог, слова).
    pub words_cache: Option<(std::time::Instant, PathBuf, std::collections::BTreeMap<String, i64>)>,
    /// Сервер LSP для `.toml` (crates-lsp: версии и features зависимостей в Cargo.toml).
    pub lsp_toml: Option<LspClient>,
    pub lsp_state: LspState,
}

impl Ide {
    pub fn new(cfg: Config, headless: bool) -> Self {
        let help = HelpFile::parse(HELP_SOURCE).unwrap_or_else(|_| HelpFile::parse(".topic Nocontext=0\nСправка недоступна.\n").unwrap());
        Ide {
            project: None,
            cfg,
            job: None,
            help: Rc::new(help),
            headless,
            inspector: None,
            project_win: None,
            messages: None,
            problems: None,
            build_diags: Vec::new(),
            problems_dirty: true,
            marks: HashMap::new(),
            win_no: 0,
            tree_dirty: false,
            last_error: None,
            lsp: None,
            lsp_toml: None,
            words_cache: None,
            lsp_state: LspState::default(),
        }
    }

    // ------------------------------------------------------------------
    // Окна
    // ------------------------------------------------------------------

    /// Нужно ли окно «Инспектор» (компоненты и свойства): нет проекта или
    /// проект tvision-rs. У программы на Rust форм нет — и этого окна тоже.
    pub fn form_tools(&self) -> bool {
        self.project.as_ref().map(|p| p.has_forms()).unwrap_or(true)
    }

    /// Раскладка окон-инструментов по размеру рабочего стола: дерево
    /// проекта — у правого края, редакторы — от левого края до него, внизу —
    /// сообщения. Проект tvision-rs (и пока проекта нет): правый столбец
    /// делят дерево проекта (сверху, около трети) и инспектор (компоненты и
    /// свойства). Программа на Rust: инспектора нет, дерево проекта — во
    /// всю высоту правого столбца.
    /// Порядок: проект, инспектор, сообщения, место для редакторов.
    fn layout(&self, app: &Program) -> [Rect; 4] {
        let s = app.desktop().group.vs.size;
        let lw = (s.x / 5).clamp(18, 26);
        let rw = (s.x / 4).clamp(26, 36);
        // Сообщения — четверть высоты без двух строк: от 3 до 7 строк.
        let bh = (s.y / 4).clamp(5, 9) - 2;
        let h = s.y - bh;
        let messages = Rect::new(0, h, s.x, s.y);
        if self.form_tools() {
            // Дерево проекта — треть столбца, но не меньше 6 строк; инспектору — не меньше 10.
            let ph = (h / 3).max(6).min((h - 10).max(3));
            [Rect::new(s.x - rw, 0, s.x, ph), Rect::new(s.x - rw, ph, s.x, h), messages, Rect::new(0, 0, s.x - rw, h)]
        } else {
            [Rect::new(s.x - lw, 0, s.x, h), Rect::new(s.x - rw, 0, s.x, h), messages, Rect::new(0, 0, s.x - lw, h)]
        }
    }

    fn alive(app: &Program, id: Option<ViewId>) -> bool {
        id.map(|i| app.desktop().group.index_of(i).is_some()).unwrap_or(false)
    }

    /// id окон типа `T` на рабочем столе снизу вверх.
    pub fn windows<T: View>(app: &Program) -> Vec<ViewId> {
        app.desktop().group.children.iter().filter(|c| c.as_any().is::<T>()).map(|c| c.vs().id).collect()
    }

    /// Окно в фокусе на рабочем столе.
    pub fn current_window(app: &Program) -> Option<ViewId> {
        app.desktop().group.current_view().map(|v| v.vs().id)
    }

    /// Верхнее окно дизайнера.
    pub fn top_designer(app: &Program) -> Option<ViewId> {
        Self::windows::<DesignWindow>(app).last().copied()
    }

    /// Документ верхнего дизайнера.
    pub fn top_doc(app: &Program) -> Option<DocRef> {
        Self::top_designer(app).and_then(|id| app.find::<DesignWindow>(id)).map(|w| w.doc.clone())
    }

    /// Показать (создать при необходимости) окна-инструменты.
    pub fn ensure_tools(&mut self, app: &mut Program) {
        let l = self.layout(app);
        if !Self::alive(app, self.project_win) {
            let mut w = ProjectWindow::new(l[0]);
            w.set_project(self.project.as_ref());
            self.project_win = Some(dialogs::insert(app, w));
        }
        // Инспектор (компоненты и свойства) — только там, где есть формы.
        // Он появился или исчез (сменился тип проекта) — дерево проекта
        // переезжает на свою сторону: у программы на Rust оно справа.
        let mut changed = false;
        if self.form_tools() {
            if !Self::alive(app, self.inspector) {
                self.inspector = Some(dialogs::insert(app, InspectorWindow::new(l[1])));
                changed = true;
            }
        } else if let Some(id) = self.inspector.take() {
            if app.desktop().group.index_of(id).is_some() {
                app.desktop_mut().group.remove(id);
                changed = true;
            }
        }
        if changed {
            if let Some(v) = self.project_win.and_then(|id| app.group.view_mut(id)) {
                v.change_bounds(l[0]);
            }
        }
        if !Self::alive(app, self.messages) {
            self.messages = Some(dialogs::insert(app, MessagesWindow::new(l[2])));
        }
    }

    /// Вернуть окна-инструменты на места по умолчанию.
    fn apply_layout(&mut self, app: &mut Program) {
        self.ensure_tools(app);
        let l = self.layout(app);
        let set = |app: &mut Program, id: Option<ViewId>, r: Rect| {
            if let Some(id) = id {
                if let Some(v) = app.group.view_mut(id) {
                    v.change_bounds(r);
                }
            }
        };
        set(app, self.project_win, l[0]);
        set(app, self.inspector, l[1]);
        set(app, self.messages, l[2]);
    }

    /// Окно сообщений (создаётся, если закрыто).
    fn msgs<'a>(&mut self, app: &'a mut Program) -> &'a mut MessagesWindow {
        if !Self::alive(app, self.messages) {
            let l = self.layout(app);
            let id = dialogs::insert(app, MessagesWindow::new(l[2]));
            self.messages = Some(id);
        }
        app.find_mut::<MessagesWindow>(self.messages.unwrap()).unwrap()
    }

    /// Написать строку в сообщения.
    pub fn say(&mut self, app: &mut Program, text: &str) {
        self.msgs(app).info(text);
    }

    fn error(app: &mut Program, text: &str) {
        message_box(app, &format!("\x03{}", text.replace('\n', "\n\x03")), mf::ERROR | mf::OK_BUTTON);
    }

    fn info_box(app: &mut Program, text: &str) {
        message_box(app, text, mf::INFORMATION | mf::OK_BUTTON);
    }

    fn base_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.dir.clone())
    }

    /// Настройки среды → редактор кода.
    fn editor_cfg(&self, e: &mut crate::codeedit::CodeEditor) {
        e.hl_line = self.cfg.hl_line;
        e.hl_col = self.cfg.hl_col;
        e.auto_close = self.cfg.auto_close;
        e.auto_complete = self.cfg.auto_complete;
        e.fmt_cmd = if self.cfg.fmt_on_save { Some(self.cfg.rustfmt.clone()) } else { None };
    }

    /// Ошибки rustfmt из редакторов — в окно сообщений.
    fn collect_fmt_errors(&mut self, app: &mut Program) {
        let mut errs = Vec::new();
        for id in Self::windows::<CodeWindow>(app) {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                if let Some(e) = w.editor_mut().fmt_error.take() {
                    errs.push((w.path(), e));
                }
            }
        }
        for (p, e) in errs {
            let name = self.project.as_ref().map(|pr| pr.rel(&p)).unwrap_or_else(|| p.display().to_string());
            self.say(app, &format!("rustfmt: {} не отформатирован (сохранён как есть):", name));
            for l in e.lines() {
                // « --> <stdin>:3:5» — место в этом файле: Enter ведёт к нему.
                let entry = match l.trim_start().strip_prefix("--> <stdin>:") {
                    Some(rest) => {
                        let mut it = rest.split(':').map(|x| x.trim().parse::<usize>().unwrap_or(1));
                        let (ln, col) = (it.next().unwrap_or(1), it.next().unwrap_or(1));
                        MsgEntry { text: format!("  {}:{}:{}", name, ln, col), kind: MsgKind::Error, loc: Some((p.display().to_string(), ln, col)) }
                    }
                    None => parse_cargo_line(&format!("  {}", l)),
                };
                self.msgs(app).push(entry);
            }
        }
    }

    /// Открыть файл в редакторе кода (или переключиться на открытый).
    pub fn open_file(&mut self, app: &mut Program, path: &Path, at: Option<(usize, usize)>) -> Option<ViewId> {
        let abs = if path.as_os_str().is_empty() { PathBuf::new() } else { tvision::widgets::editor::absolute(path) };
        let existing = Self::windows::<CodeWindow>(app)
            .into_iter()
            .find(|&id| !abs.as_os_str().is_empty() && app.find::<CodeWindow>(id).map(|w| same_file(&w.path(), &abs)).unwrap_or(false));
        let id = match existing {
            Some(id) => {
                app.focus(id);
                id
            }
            None => {
                let r = self.layout(app)[3];
                let r = if r.width() < 40 || r.height() < 8 { app.desktop().group.vs.extent() } else { r };
                self.win_no = self.win_no % 9 + 1;
                let mut w = CodeWindow::new(r, &abs, self.win_no, self.base_dir());
                if let Some(m) = self.marks.get(&abs) {
                    w.set_marks(m.clone());
                }
                self.editor_cfg(w.editor_mut());
                self.lsp_state.diags_dirty = true;
                dialogs::insert(app, w)
            }
        };
        if let Some((l, c)) = at {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                w.goto(l, c);
            }
        }
        Some(id)
    }

    // ------------------------------------------------------------------
    // Проект
    // ------------------------------------------------------------------

    /// Есть ли несохранённые изменения в окнах.
    fn dirty_windows(app: &Program) -> bool {
        let code =
            Self::windows::<CodeWindow>(app).into_iter().any(|id| app.find::<CodeWindow>(id).map(|w| w.editor().fe.editor.modified).unwrap_or(false));
        let forms =
            Self::windows::<DesignWindow>(app).into_iter().any(|id| app.find::<DesignWindow>(id).map(|w| w.doc.borrow().modified).unwrap_or(false));
        code || forms
    }

    /// Сохранить всё: файлы, формы, проект. Возвращает ошибки.
    pub fn save_all(&mut self, app: &mut Program) -> Vec<String> {
        let mut errs = Vec::new();
        for id in Self::windows::<CodeWindow>(app) {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                if w.editor().fe.editor.modified {
                    w.editor_mut().format_now();
                }
                let fe = &mut w.editor_mut().fe;
                if fe.editor.modified && !fe.file_name.as_os_str().is_empty() && !fe.save_file() {
                    errs.push(fe.last_error.take().unwrap_or_default());
                }
            }
        }
        for id in Self::windows::<DesignWindow>(app) {
            if let Some(w) = app.find::<DesignWindow>(id) {
                let mut d = w.doc.borrow_mut();
                if d.modified {
                    if let Err(e) = d.save() {
                        errs.push(e);
                    }
                }
            }
        }
        if let Some(p) = &self.project {
            if let Err(e) = p.save() {
                errs.push(e);
            }
        }
        errs
    }

    /// Закрыть проект (спросив о сохранении). `false` — пользователь передумал.
    pub fn close_project(&mut self, app: &mut Program) -> bool {
        if Self::dirty_windows(app) {
            let r = message_box(app, "\x03Есть несохранённые изменения.\n\x03Сохранить их?", mf::CONFIRMATION | mf::YES_NO_CANCEL);
            match r {
                cm::YES => {
                    let e = self.save_all(app);
                    if !e.is_empty() {
                        Self::error(app, &e.join("\n"));
                        return false;
                    }
                }
                cm::NO => {}
                _ => return false,
            }
        }
        // Запоминаем открытые файлы.
        if let Some(p) = &mut self.project {
            let files: Vec<String> = Self::windows::<CodeWindow>(app)
                .into_iter()
                .filter_map(|id| app.find::<CodeWindow>(id).map(|w| w.path()))
                .filter(|f| !f.as_os_str().is_empty())
                .map(|f| p.rel(&f))
                .collect();
            p.open_files = files;
            let _ = p.save();
        }
        for id in Self::windows::<CodeWindow>(app).into_iter().chain(Self::windows::<DesignWindow>(app)) {
            app.desktop_mut().group.remove(id);
        }
        self.lsp_stop(app);
        self.project = None;
        self.marks.clear();
        self.tree_dirty = true;
        true
    }

    /// Открыть проект из файла `.tvsp`.
    pub fn open_project(&mut self, app: &mut Program, file: &Path) {
        if self.project.is_some() && !self.close_project(app) {
            return;
        }
        match Project::load(file) {
            Ok(p) => {
                let f = p.file().display().to_string();
                let files: Vec<PathBuf> = p.open_files.iter().map(|r| p.dir.join(r)).collect();
                let name = p.name.clone();
                self.project = Some(p);
                self.cfg.add_recent(&f);
                if !self.headless {
                    self.cfg.save();
                }
                self.tree_dirty = true;
                self.ensure_tools(app);
                self.say(app, &format!("Открыт проект {} ({})", name, f));
                self.lsp_start(app);
                for path in files {
                    if path.exists() {
                        self.open_file(app, &path, None);
                    }
                }
            }
            Err(e) => Self::error(app, &e),
        }
    }

    fn new_project(&mut self, app: &mut Program) {
        let (cmd, d) = dialogs::exec(app, dialogs::new_project_dialog(&self.cfg, "my_app", "Новый проект", true));
        if cmd != cm::OK {
            return;
        }
        let np = match dialogs::read_new_project(&d.get_data(), true) {
            Ok(n) => n,
            Err(e) => return Self::error(app, &e),
        };
        if self.project.is_some() && !self.close_project(app) {
            return;
        }
        let Some(dep) = np.dep else {
            // Обычная программа на Rust: без форм, модели и генерации кода.
            match Project::create_rust(&np.parent, &np.name) {
                Ok(p) => {
                    let file = p.file();
                    let main = p.dir.join("src").join("main.rs");
                    self.open_project(app, &file);
                    if let Some(id) = self.open_file(app, &main, None) {
                        app.focus(id);
                    }
                    self.say(app, "Новый проект на Rust создан. F9 — собрать, Ctrl+F9 — запустить.");
                }
                Err(e) => Self::error(app, &e),
            }
            return;
        };
        match Project::create(&np.parent, &np.name, &dep) {
            Ok(p) => {
                // Сразу генерируем код — проект собирается «из коробки».
                let forms = self.load_forms(app, &p);
                if let Err(e) = codegen::generate_all(&p, &forms) {
                    Self::error(app, &e);
                }
                let file = p.file();
                self.open_project(app, &file);
                self.open_form(app, "about");
                self.say(app, "Новый проект создан. F9 — собрать, Ctrl+F9 — запустить.");
            }
            Err(e) => Self::error(app, &e),
        }
    }

    /// Импорт готовой программы на tvision-rs: исходники копируются в новый
    /// проект, меню, строка состояния и команды переходят в модель.
    fn import(&mut self, app: &mut Program) {
        let d = FileDialog::new("*.rs", "Импорт программы tvision-rs — главный файл", "~И~мя", fd::OPEN_BUTTON, 112);
        let (cmd, d) = dialogs::exec(app, d);
        if cmd == cm::CANCEL {
            return;
        }
        let main = d.file_name();
        // Имя проекта: имя файла, а для main.rs — имя его каталога.
        let stem = if main.file_name().map(|f| f == "main.rs").unwrap_or(false) {
            main.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        } else {
            main.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        };
        let (cmd, d) = dialogs::exec(app, dialogs::new_project_dialog(&self.cfg, &stem, "Импорт: новый проект", false));
        if cmd != cm::OK {
            return;
        }
        let np = match dialogs::read_new_project(&d.get_data(), false) {
            Ok(n) => n,
            Err(e) => return Self::error(app, &e),
        };
        if self.project.is_some() && !self.close_project(app) {
            return;
        }
        let Some(dep) = np.dep else { return };
        match crate::importer::import_program(&main, &np.parent, &np.name, &dep) {
            Ok((p, report)) => {
                let file = p.file();
                self.open_project(app, &file);
                self.say(app, &format!("Импортировано из {}: {}", main.display(), report.summary()));
                let src = p.dir.join("src").join("main.rs");
                self.open_file(app, &src, None);
            }
            Err(e) => Self::error(app, &e),
        }
    }

    fn choose_project_file(&mut self, app: &mut Program) {
        let d = FileDialog::new(&format!("*.{}", PROJECT_EXT), "Открыть проект", "~И~мя", fd::OPEN_BUTTON, 110);
        let (cmd, d) = dialogs::exec(app, d);
        if cmd != cm::CANCEL {
            let f = d.file_name();
            self.open_project(app, &f);
        }
    }

    fn recent(&mut self, app: &mut Program) {
        if self.cfg.recent.is_empty() {
            return Self::info_box(app, "\x03Недавних проектов пока нет.");
        }
        let (d, list) = dialogs::recent_dialog(&self.cfg.recent);
        let (cmd, d) = dialogs::exec(app, d);
        if cmd == cm::OK {
            let i = d.win.group.find::<ListBox>(list).map(|l| l.focused).unwrap_or(0);
            if let Some(f) = self.cfg.recent.get(i).cloned() {
                self.open_project(app, Path::new(&f));
            }
        }
    }

    /// Формы проекта: открытые — из дизайнеров (свежие), остальные — с диска.
    fn load_forms(&self, app: &Program, p: &Project) -> Vec<Form> {
        let open: Vec<DocRef> =
            Self::windows::<DesignWindow>(app).into_iter().filter_map(|id| app.find::<DesignWindow>(id).map(|w| w.doc.clone())).collect();
        p.forms
            .iter()
            .filter_map(|name| {
                let path = p.form_path(name);
                if let Some(d) = open.iter().find(|d| same_file(&d.borrow().path, &path)) {
                    return Some(d.borrow().form.clone());
                }
                Form::load(&path).ok()
            })
            .collect()
    }

    /// Сгенерировать код. `false` — генерация не удалась.
    pub fn generate(&mut self, app: &mut Program, quiet: bool) -> bool {
        let Some(p) = self.project.clone() else { return false };
        // Программе на Rust генерировать нечего: весь код — её.
        if !p.has_forms() {
            if !quiet {
                self.say(app, "Проект на Rust: генерировать нечего — весь код ваш.");
            }
            return true;
        }
        let forms = self.load_forms(app, &p);
        let warnings = codegen::check_project(&p, &forms);
        for w in &warnings {
            self.msgs(app).push(MsgEntry { text: format!("модель: {}", w), kind: MsgKind::Warning, loc: None });
        }
        match codegen::generate_all(&p, &forms) {
            Ok(changed) => {
                // Открытые сгенерированные файлы перечитываем.
                for id in Self::windows::<CodeWindow>(app) {
                    if let Some(w) = app.find_mut::<CodeWindow>(id) {
                        let path = w.path();
                        if changed.iter().any(|c| same_file(c, &path)) {
                            let fe = &mut w.editor_mut().fe;
                            fe.load_file();
                            fe.editor.modified = false;
                        }
                    }
                }
                if !quiet || !changed.is_empty() {
                    let list: Vec<String> = changed.iter().map(|c| p.rel(c)).collect();
                    let text = if list.is_empty() {
                        "Код не изменился.".to_string()
                    } else {
                        format!("Сгенерировано: {}", list.join(", "))
                    };
                    self.say(app, &text);
                }
                self.tree_dirty = true;
                true
            }
            Err(e) => {
                Self::error(app, &e);
                false
            }
        }
    }

    /// Запустить cargo с аргументами.
    pub fn cargo(&mut self, app: &mut Program, args: &[&str], what: &str, after: After) {
        let Some(dir) = self.base_dir() else { return };
        if self.job.is_some() {
            return Self::info_box(app, "\x03Сборка уже идёт.\n\x03Сборка → Остановить — прервать её.");
        }
        if self.cfg.autosave {
            let errs = self.save_all(app);
            if !errs.is_empty() {
                return Self::error(app, &errs.join("\n"));
            }
        }
        if !self.generate(app, true) {
            return;
        }
        // Параметры сборки проекта: профиль, платформа и пакет — для всех
        // команд (и для clean), features и прочее — для check/build/test.
        let opts = self.project.as_ref().map(|p| p.build.clone()).unwrap_or_default();
        let mut all: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        match args.first() {
            Some(&"clean") => all.extend(opts.profile_args()),
            Some(_) => all.extend(opts.cargo_args()),
            None => {}
        }
        let args: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
        self.msgs(app).clear();
        self.build_diags.clear();
        self.problems_dirty = true;
        self.say(app, &format!("— {} ({}): {} {}", what, opts.profile_title(), self.cfg.cargo, args.join(" ")));
        match Job::start(&dir, &self.cfg.cargo.clone(), &args, what, after) {
            Ok(j) => self.job = Some(j),
            Err(e) => {
                self.say(app, &e);
                Self::error(app, &e);
            }
        }
    }

    fn poll_job(&mut self, app: &mut Program) {
        let Some(job) = &mut self.job else { return };
        let msgs = job.poll();
        if msgs.is_empty() {
            return;
        }
        let finished = job.finished;
        let what = job.what.clone();
        let after = job.after;
        let secs = job.started.elapsed().as_secs_f32();
        let mut code_done = None;
        for m in msgs {
            match m {
                JobMsg::Line(l) => {
                    if !l.trim().is_empty() {
                        let e = parse_cargo_line(&l);
                        if e.loc.is_some() && matches!(e.kind, MsgKind::Error | MsgKind::Warning) {
                            self.build_diags.push(e.clone());
                        }
                        self.msgs(app).push(e);
                    }
                }
                JobMsg::Done(c) => code_done = Some(c),
            }
        }
        if !finished {
            return;
        }
        self.job = None;
        let code = code_done.flatten();
        let (errs, warns) = self.msgs(app).counts();
        let ok = code == Some(0);
        self.say(
            app,
            &format!("— {} {} за {:.1} с: ошибок {}, предупреждений {}", what, if ok { "завершена" } else { "не удалась" }, secs, errs, warns),
        );
        self.update_marks(app);
        if !ok && errs > 0 {
            // Сразу к первой ошибке.
            if let Some(id) = self.messages {
                let first = app.find::<MessagesWindow>(id).and_then(|m| m.entries.iter().position(|e| e.kind == MsgKind::Error && e.loc.is_some()));
                if let Some(i) = first {
                    if let Some(m) = app.find_mut::<MessagesWindow>(id) {
                        m.focus(i);
                    }
                    app.focus(id);
                }
            }
        }
        if ok && after == After::Run && !self.headless {
            if let Some(dir) = self.base_dir() {
                let cargo = self.cfg.cargo.clone();
                // Workspace: программа участника, в котором верхний открытый файл.
                let opts = self.project.as_ref().map(|p| p.build.clone()).unwrap_or_default();
                // Пакет из параметров сборки, иначе — участник с верхним открытым файлом.
                let mut extra = opts.cargo_args();
                if opts.package.trim().is_empty() {
                    let file = Self::windows::<CodeWindow>(app).last().and_then(|&id| app.find::<CodeWindow>(id)).map(|w| w.path());
                    if let Some(p) = crate::model::cargo_layout(&dir).run_package(file.as_deref()) {
                        self.say(app, &format!("— Запуск пакета {} (cargo run -p {})", p, p));
                        extra.extend(["-p".to_string(), p]);
                    }
                }
                let prog = opts.program_args();
                if !prog.is_empty() {
                    extra.push("--".into());
                    extra.extend(prog);
                }
                let extra: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();
                let code = app.suspended(|| crate::runner::run_foreground(&dir, &cargo, &extra));
                self.say(app, &format!("— Программа завершилась, код {}", code.map(|c| c.to_string()).unwrap_or("?".into())));
            }
        }
    }

    /// Пометки ошибок в открытых редакторах по последней сборке.
    fn update_marks(&mut self, app: &mut Program) {
        let Some(dir) = self.base_dir() else { return };
        let mut marks: HashMap<PathBuf, Vec<(usize, MsgKind)>> = HashMap::new();
        if let Some(m) = self.messages.and_then(|id| app.find::<MessagesWindow>(id)) {
            for e in &m.entries {
                if let (Some((f, l, _)), MsgKind::Error | MsgKind::Warning) = (&e.loc, e.kind) {
                    let p = resolve(&dir, f);
                    let v = marks.entry(p).or_default();
                    // Ошибка «главнее» предупреждения на той же строке.
                    if let Some(x) = v.iter_mut().find(|(ln, _)| *ln == l - 1) {
                        if e.kind == MsgKind::Error {
                            x.1 = MsgKind::Error;
                        }
                    } else {
                        v.push((l.saturating_sub(1), e.kind));
                    }
                }
            }
        }
        for id in Self::windows::<CodeWindow>(app) {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                let p = w.path();
                let m = marks.iter().find(|(k, _)| same_file(k, &p)).map(|(_, v)| v.clone()).unwrap_or_default();
                w.set_marks(m);
            }
        }
        self.marks = marks;
        // Где есть свежая диагностика сервера — она важнее.
        self.lsp_state.diags_dirty = true;
    }

    /// Показать окно «Ошибки и предупреждения» (создать на месте окна сообщений).
    fn show_problems(&mut self, app: &mut Program) {
        if !Self::alive(app, self.problems) {
            let l = self.layout(app);
            self.problems = Some(dialogs::insert(app, ProblemsWindow::new(l[2])));
            self.problems_dirty = true;
            self.update_problems(app);
        }
        if let Some(id) = self.problems {
            app.focus(id);
        }
    }

    /// Обновить окно ошибок, если оно открыто и данные изменились.
    fn update_problems(&mut self, app: &mut Program) {
        if !self.problems_dirty || !Self::alive(app, self.problems) {
            return;
        }
        self.problems_dirty = false;
        let list = self.collect_problems(app);
        if let Some(w) = self.problems.and_then(|id| app.find_mut::<ProblemsWindow>(id)) {
            w.set_problems(list);
        }
    }

    /// Замечания сборки — для файлов, о которых сервер LSP ещё не присылал диагностику.
    fn build_problems(&self) -> Vec<Problem> {
        let dir = self.base_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let mut v: Vec<Problem> = Vec::new();
        for e in &self.build_diags {
            let Some((f, l, c)) = &e.loc else { continue };
            let path = resolve(&dir, f);
            if self.lsp_state.diags.keys().any(|k| same_file(k, &path)) {
                continue;
            }
            // Текст без «файл:строка:столбец: » в начале.
            // «error[E0308]: текст» → «[E0308] текст»: вид замечания показывает значок.
            let text = e.text.trim().strip_prefix(&format!("{}:{}:{}:", f, l, c)).unwrap_or(&e.text).trim();
            let text = ["error", "warning"].iter().find_map(|w| text.strip_prefix(w)).unwrap_or(text);
            let text = match text.split_once(':') {
                Some((code, rest)) if code.is_empty() || (code.starts_with('[') && code.ends_with(']')) => {
                    format!("{}{}{}", code, if code.is_empty() { "" } else { " " }, rest.trim())
                }
                _ => text.trim().to_string(),
            };
            let p = Problem { kind: e.kind, shown: f.replace('\\', "/"), path, line: *l, col: *c, text, source: "cargo" };
            if !v.contains(&p) {
                v.push(p);
            }
        }
        v
    }

    /// Перейти к месту из строки сообщений.
    fn goto_message(&mut self, app: &mut Program, index: usize) {
        let Some(id) = self.messages else { return };
        let Some(e) = app.find::<MessagesWindow>(id).and_then(|m| m.entries.get(index).cloned()) else { return };
        let Some((f, l, c)) = e.loc else { return };
        let dir = self.base_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let path = resolve(&dir, &f);
        if !path.exists() {
            return Self::error(app, &format!("Файл не найден: {}", path.display()));
        }
        if path.extension().map(|x| x == FORM_EXT).unwrap_or(false) {
            let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            self.open_form(app, &name);
            return;
        }
        self.open_file(app, &path, Some((l, c)));
    }

    fn next_error(&mut self, app: &mut Program, forward: bool) {
        let Some(id) = self.messages else { return };
        let Some(i) = app.find::<MessagesWindow>(id).and_then(|m| m.next_location(forward)) else { return };
        if let Some(m) = app.find_mut::<MessagesWindow>(id) {
            m.focus(i);
        }
        self.goto_message(app, i);
    }

    /// Поиск текста во всех файлах проекта.
    fn find_in_files(&mut self, app: &mut Program) {
        let Some(dir) = self.base_dir() else { return };
        let Some(what) = input_box(app, "Найти в файлах", "~Т~екст", "", 128) else { return };
        if what.is_empty() {
            return;
        }
        let low = what.to_lowercase();
        let mut files = Vec::new();
        collect_files(&dir, &mut files, 0);
        self.msgs(app).clear();
        self.say(app, &format!("— Поиск «{}»", what));
        let mut n = 0;
        for f in files {
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            for (i, line) in text.lines().enumerate() {
                if let Some(pos) = line.to_lowercase().find(&low) {
                    let col = line[..pos.min(line.len())].chars().count() + 1;
                    let rel = f.strip_prefix(&dir).unwrap_or(&f).display().to_string();
                    self.msgs(app).push(MsgEntry {
                        text: format!("{}:{}:{}: {}", rel, i + 1, col, line.trim()),
                        kind: MsgKind::Warning,
                        loc: Some((rel, i + 1, col)),
                    });
                    n += 1;
                }
            }
        }
        self.say(app, &format!("— Найдено: {}", n));
        if let Some(id) = self.messages {
            app.focus(id);
        }
    }

    // ------------------------------------------------------------------
    // Формы
    // ------------------------------------------------------------------

    /// Открыть форму в дизайнере.
    pub fn open_form(&mut self, app: &mut Program, name: &str) {
        let Some(p) = &self.project else { return };
        let path = p.form_path(name);
        if let Some(id) = Self::windows::<DesignWindow>(app)
            .into_iter()
            .find(|&id| app.find::<DesignWindow>(id).map(|w| same_file(&w.doc.borrow().path, &path)).unwrap_or(false))
        {
            app.focus(id);
            return;
        }
        match Form::load(&path) {
            Ok(mut f) => {
                // Форма не должна оказаться за краем рабочего стола.
                let s = app.desktop().group.vs.size;
                f.x = f.x.clamp(0, (s.x - f.w).max(0));
                f.y = f.y.clamp(0, (s.y - f.h).max(0));
                let doc = FormDoc::new(f, path);
                dialogs::insert(app, DesignWindow::new(doc));
            }
            Err(e) => Self::error(app, &e),
        }
    }

    fn new_form(&mut self, app: &mut Program) {
        let Some(p) = &self.project else { return };
        let existing = p.forms.clone();
        let (cmd, d) = dialogs::exec(app, dialogs::new_form_dialog(&existing));
        if cmd != cm::OK {
            return;
        }
        let f = match dialogs::read_new_form(&d.get_data(), &existing) {
            Ok(f) => f,
            Err(e) => return Self::error(app, &e),
        };
        let p = self.project.as_mut().unwrap();
        let path = p.form_path(&f.name);
        if let Err(e) = f.save(&path) {
            return Self::error(app, &e);
        }
        p.forms.push(f.name.clone());
        let _ = p.save();
        self.tree_dirty = true;
        let name = f.name.clone();
        self.open_form(app, &name);
    }

    fn choose_form(&mut self, app: &mut Program) {
        let Some(p) = &self.project else { return };
        if p.forms.is_empty() {
            return Self::info_box(app, "\x03В проекте нет форм.");
        }
        let (mut d, list) = dialogs::recent_dialog(&p.forms);
        d.win.title = "Открыть форму".into();
        let (cmd, d) = dialogs::exec(app, d);
        if cmd == cm::OK {
            let i = d.win.group.find::<ListBox>(list).map(|l| l.focused).unwrap_or(0);
            if let Some(n) = self.project.as_ref().and_then(|p| p.forms.get(i).cloned()) {
                self.open_form(app, &n);
            }
        }
    }

    fn remove_form(&mut self, app: &mut Program) {
        let Some(doc) = Self::top_doc(app) else { return };
        let name = doc.borrow().form.name.clone();
        let r = message_box(
            app,
            &format!("\x03Удалить форму «{}» из проекта?\n\x03Файл формы тоже будет удалён.", name),
            mf::CONFIRMATION | mf::YES_BUTTON | mf::NO_BUTTON,
        );
        if r != cm::YES {
            return;
        }
        if let Some(id) = Self::top_designer(app) {
            app.desktop_mut().group.remove(id);
        }
        let path = doc.borrow().path.clone();
        let _ = std::fs::remove_file(&path);
        if let Some(p) = &mut self.project {
            p.forms.retain(|f| *f != name);
            let _ = p.save();
        }
        self.tree_dirty = true;
        self.say(app, &format!("Форма {} удалена из проекта.", name));
    }

    /// Форма переименована в инспекторе → переименовать файл и ссылки.
    fn sync_form_names(&mut self, app: &mut Program) {
        let Some(p) = &mut self.project else { return };
        for id in Self::windows::<DesignWindow>(app) {
            let Some(w) = app.find::<DesignWindow>(id) else { continue };
            let mut d = w.doc.borrow_mut();
            let stem = d.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let new = d.form.name.clone();
            if stem == new || !crate::model::is_ident(&new) {
                continue;
            }
            if p.forms.contains(&new) {
                d.error = Some(format!("Форма «{}» уже есть в проекте — имя не изменено.", new));
                d.form.name = stem.clone();
                d.changed();
                continue;
            }
            let new_path = p.form_path(&new);
            let _ = std::fs::remove_file(&d.path);
            d.path = new_path;
            d.modified = true;
            let _ = d.save();
            for f in p.forms.iter_mut() {
                if *f == stem {
                    *f = new.clone();
                }
            }
            for c in p.app.commands.iter_mut() {
                if c.action == Action::Form(stem.clone()) {
                    c.action = Action::Form(new.clone());
                }
            }
            if p.app.startup_form == stem {
                p.app.startup_form = new.clone();
            }
            let _ = p.save();
            self.tree_dirty = true;
        }
    }

    /// Проверить форму: показать настоящий диалог модально.
    fn test_form(&mut self, app: &mut Program) {
        let Some(doc) = Self::top_doc(app) else { return };
        let form = doc.borrow().form.clone();
        let model = self.project.as_ref().map(|p| p.app.clone());
        let resolve = move |name: &str| model.as_ref().and_then(|a| a.command_value(name)).unwrap_or(cm::OK);
        let (d, _) = build_dialog(&form, &resolve);
        let (cmd, d) = dialogs::exec(app, d);
        let name = BUILTIN_NAME.iter().find(|(c, _)| *c == cmd).map(|(_, n)| n.to_string()).unwrap_or_else(|| cmd.to_string());
        let text = format!("Диалог закрыт командой {}.\n\n{}", name, describe_data(&form, &d.get_data()));
        message_box_rect(app, Rect::new(0, 0, 60, 16), &text, mf::INFORMATION | mf::OK_BUTTON, true);
    }

    /// F12: из дизайнера — в сгенерированный код формы и обратно.
    fn form_code(&mut self, app: &mut Program) {
        let cur = Self::current_window(app);
        if let Some(w) = cur.and_then(|id| app.find::<CodeWindow>(id)) {
            let p = w.path();
            let is_form_code = p.parent().map(|d| d.ends_with("forms")).unwrap_or(false);
            if is_form_code {
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if self.project.as_ref().map(|pr| pr.forms.contains(&name)).unwrap_or(false) {
                    self.open_form(app, &name);
                    return;
                }
            }
        }
        let Some(doc) = Self::top_doc(app) else { return };
        let name = doc.borrow().form.name.clone();
        if self.generate(app, true) {
            if let Some(path) = self.project.as_ref().map(|p| p.form_code_path(&name)) {
                self.open_file(app, &path, None);
            }
        }
    }

    fn form_props(&mut self, app: &mut Program) {
        let Some(doc) = Self::top_doc(app) else { return };
        let (d, list) = {
            let b = doc.borrow();
            dialogs::props_dialog(&b.form, b.primary())
        };
        let (cmd, d) = dialogs::exec(app, d);
        if cmd != cm::OK {
            return;
        }
        let r = {
            let mut b = doc.borrow_mut();
            let sel = b.primary();
            let mut f = b.form.clone();
            match dialogs::apply_props(&mut f, sel, &list, &d.get_data()) {
                Ok(()) => {
                    b.edit(|form, _| *form = f);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        };
        if let Err(e) = r {
            Self::error(app, &e);
        }
        if let Some(id) = Self::top_designer(app) {
            app.focus(id);
        }
    }

    // ------------------------------------------------------------------
    // Модель приложения
    // ------------------------------------------------------------------

    fn edit_model(&mut self, app: &mut Program, kind: LeKind) {
        let Some(p) = &self.project else { return };
        let ed = ListEditor::new(kind, &p.app, &p.forms);
        let (cmd, ed) = dialogs::exec(app, ed);
        if cmd != cm::OK {
            return;
        }
        let mut model = ed.app.clone();
        if kind == LeKind::Menu {
            model.menu = ed.menu();
        }
        let p = self.project.as_mut().unwrap();
        p.app = model;
        if let Err(e) = p.save() {
            Self::error(app, &e);
        }
        self.say(app, "Модель приложения изменена (код обновится при генерации или сборке).");
    }

    /// Диалоги из кода программы → формы проекта.
    fn extract_forms(&mut self, app: &mut Program) {
        let errs = self.save_all(app);
        if !errs.is_empty() {
            return Self::error(app, &errs.join("\n"));
        }
        let Some(p) = &mut self.project else { return };
        match crate::formextract::extract_into_project(p) {
            Ok(r) => {
                self.tree_dirty = true;
                let text = format!("Формы из кода: {}", r.summary());
                self.say(app, &text);
                // Открытые файлы могли измениться (mod forms, константы команд).
                for id in Self::windows::<CodeWindow>(app) {
                    if let Some(w) = app.find_mut::<CodeWindow>(id) {
                        let fe = &mut w.editor_mut().fe;
                        if !fe.editor.modified {
                            fe.load_file();
                        }
                    }
                }
                if let Some(id) = self.messages {
                    app.focus(id);
                }
            }
            Err(e) => Self::error(app, &e),
        }
    }

    fn app_options(&mut self, app: &mut Program) {
        let Some(p) = &self.project else { return };
        let (cmd, d) = dialogs::exec(app, dialogs::app_options_dialog(&p.app, &p.forms));
        if cmd != cm::OK {
            return;
        }
        let p = self.project.as_mut().unwrap();
        let forms = p.forms.clone();
        if let Err(e) = dialogs::read_app_options(&mut p.app, &d.get_data(), &forms) {
            return Self::error(app, &e);
        }
        let _ = p.save();
    }

    /// «Параметры сборки»: «Собрать» — сохранить и собрать, «Сохранить» — только сохранить.
    fn build_options(&mut self, app: &mut Program) {
        let Some(p) = &self.project else { return };
        let packages: Vec<String> = crate::model::cargo_layout(&p.dir).members.iter().map(|m| m.name.clone()).collect();
        let (cmd, d) = dialogs::exec(app, dialogs::build_options_dialog(&p.build, &packages));
        if cmd != cm::OK && cmd != cm::YES {
            return;
        }
        let opts = match dialogs::read_build_options(&d.get_data()) {
            Ok(o) => o,
            Err(e) => return Self::error(app, &e),
        };
        let p = self.project.as_mut().unwrap();
        p.build = opts;
        if let Err(e) = p.save() {
            return Self::error(app, &e);
        }
        if cmd == cm::OK {
            self.cargo(app, &["build", "--message-format=short"], "Сборка", After::Nothing);
        }
    }

    // ------------------------------------------------------------------
    // Запуск и завершение
    // ------------------------------------------------------------------

    /// Начало работы: окна-инструменты, проект или файлы из командной строки.
    pub fn startup(&mut self, app: &mut Program, args: Vec<String>) {
        app.palette = self.cfg.palette();
        let diag_hint = self.lsp_state.hint.clone();
        let hint: Box<dyn Fn(u16) -> String> = Box::new(move |ctx| match ctx {
            HC_DESIGNER => "Стрелки — сдвиг, Shift — размер".into(),
            HC_PALETTE => "Enter — добавить на форму".into(),
            HC_PROJECT => "Enter — открыть".into(),
            HC_MESSAGES => "Enter — к месту ошибки".into(),
            HC_EDITOR => diag_hint.borrow().clone(),
            _ => String::new(),
        });
        if let Some(sl) = app.status_line_mut() {
            sl.hint = Some(hint);
        }
        self.ensure_tools(app);
        let mut opened = false;
        for a in &args {
            let p = PathBuf::from(a);
            if p.extension().map(|x| x == PROJECT_EXT).unwrap_or(false) {
                self.open_project(app, &p);
                opened = true;
            } else if p.is_dir() {
                if let Some(f) = find_project_file(&p) {
                    self.open_project(app, &f);
                    opened = true;
                } else if p.join("Cargo.toml").is_file() {
                    // Исходники на Rust без файла проекта: создаём проект и открываем.
                    match Project::adopt_rust(&p) {
                        Ok(proj) => {
                            let f = proj.file();
                            self.open_project(app, &f);
                            self.say(app, &format!("Создан проект на Rust для каталога {} ({}).", proj.dir.display(), f.display()));
                            opened = true;
                        }
                        Err(e) => Self::error(app, &e),
                    }
                } else {
                    self.say(
                        app,
                        &format!("Это не исходники Rust: в каталоге {} нет ни файла проекта .{}, ни Cargo.toml.", p.display(), PROJECT_EXT),
                    );
                }
            } else {
                self.open_file(app, &p, None);
                opened = true;
            }
        }
        if !opened && !self.headless {
            if let Some(f) = self.cfg.recent.first().cloned().filter(|f| Path::new(f).exists()) {
                self.open_project(app, Path::new(&f));
                opened = true;
            }
        }
        if !opened {
            self.say(app, "Добро пожаловать в TV Studio — среду визуальной разработки на tvision-rs.");
            self.say(app, "Файл → Новый проект — начать; F1 — справка; F10 — меню.");
        }
        self.idle(app);
    }

    /// Завершение: запомнить открытые файлы и настройки.
    pub fn shutdown(&mut self, app: &mut Program) {
        if let Some(job) = &self.job {
            job.kill();
        }
        for c in [self.lsp.take(), self.lsp_toml.take()].into_iter().flatten() {
            c.stop();
        }
        if let Some(p) = &mut self.project {
            let files: Vec<String> = Self::windows::<CodeWindow>(app)
                .into_iter()
                .filter_map(|id| app.find::<CodeWindow>(id).map(|w| w.path()))
                .filter(|f| !f.as_os_str().is_empty())
                .map(|f| p.rel(&f))
                .collect();
            p.open_files = files;
            let _ = p.save();
        }
        if !self.headless {
            self.cfg.save();
        }
    }

    /// Разрешить/запретить команды по состоянию среды.
    fn update_commands(&self, app: &mut Program) {
        let has_p = self.project.is_some();
        let running = self.job.is_some();
        let has_d = Self::top_designer(app).is_some();
        let set = |app: &mut Program, c: u16, on: bool| {
            if on {
                app.enable_command(c)
            } else {
                app.disable_command(c)
            }
        };
        // Формы и модель — только у проекта tvision-rs.
        let has_forms = self.project.as_ref().map(|p| p.has_forms()).unwrap_or(false);
        for c in [CM_CHECK, CM_BUILD, CM_RUN, CM_TEST, CM_CLEAN, CM_BUILD_OPTS] {
            set(app, c, has_p && !running);
        }
        set(app, CM_GENERATE, has_forms && !running);
        set(app, CM_STOP, running);
        for c in [CM_EXTRACT_FORMS, CM_NEW_FORM, CM_OPEN_FORM, CM_EDIT_MENU, CM_EDIT_STATUS, CM_EDIT_COMMANDS, CM_APP_OPTIONS] {
            set(app, c, has_forms);
        }
        for c in [CM_CLOSE_PROJECT, CM_FIND_FILES] {
            set(app, c, has_p);
        }
        // CM_SHOW_PALETTE / CM_SHOW_INSPECTOR (номера > 255) выключить нельзя;
        // в проекте на Rust они просто ничего не показывают (см. ensure_tools).
        for c in [CM_DS_PROPS, CM_FORM_TEST, CM_DS_DUP, CM_GRID, CM_TABNUM, CM_DS_FRONT, CM_DS_BACK, CM_DS_FORWARD, CM_DS_BACKWARD] {
            set(app, c, has_d);
        }
        set(app, CM_FORM_CODE, has_d && has_p);
        set(app, CM_REMOVE_FORM, has_d && has_p);
        for i in 0..Align::ALL.len() as u16 {
            set(app, CM_ALIGN_BASE + i, has_d);
        }
        for i in 0..Kind::ALL.len() as u16 {
            set(app, CM_INS_BASE + i, has_d);
        }
        let code = Self::current_window(app).map(|id| app.find::<CodeWindow>(id).is_some()).unwrap_or(false);
        set(app, CM_GOTO, code);
        for c in [CM_COMPLETE, CM_GOTO_DEF, CM_FIND_REFS, CM_CODE_ACTIONS] {
            set(app, c, code && self.any_lsp());
        }
        set(app, CM_NAV_BACK, !self.lsp_state.nav.is_empty());
        set(app, CM_DIAGNOSTICS, self.any_lsp());
        let locs = self.messages.and_then(|id| app.find::<MessagesWindow>(id)).map(|m| m.entries.iter().any(|e| e.loc.is_some())).unwrap_or(false);
        set(app, CM_NEXT_ERR, locs);
        set(app, CM_PREV_ERR, locs);
    }

    fn sync_windows(&mut self, app: &mut Program) {
        self.sync_form_names(app);
        let mut errors = Vec::new();
        for id in Self::windows::<DesignWindow>(app) {
            if let Some(w) = app.find_mut::<DesignWindow>(id) {
                w.sync_from_doc();
                if let Some(e) = w.doc.borrow_mut().error.take() {
                    errors.push(e);
                }
            }
        }
        let doc = Self::top_doc(app);
        if let Some(id) = self.inspector {
            if let Some(w) = app.find_mut::<InspectorWindow>(id) {
                w.attach(doc);
            }
        }
        if self.tree_dirty {
            self.tree_dirty = false;
            if let Some(id) = self.project_win {
                let p = self.project.clone();
                if let Some(w) = app.find_mut::<ProjectWindow>(id) {
                    w.set_project(p.as_ref());
                }
            }
        }
        for e in errors {
            if self.headless {
                // В тестах окно с ошибкой ждало бы нажатия клавиши.
                self.say(app, &format!("Ошибка: {}", e));
                self.last_error = Some(e);
            } else {
                Self::error(app, &e);
            }
        }
    }

    fn about(app: &mut Program) {
        message_box_rect(
            app,
            Rect::new(0, 0, 58, 13),
            "\x03TV Studio 0.1\n\n\x03Среда визуальной разработки\n\x03текстовых приложений на tvision-rs\n\n\x03Формы • модель приложения • код на Rust\n\x03редактор • сборка cargo",
            mf::INFORMATION | mf::OK_BUTTON,
            true,
        );
    }

    fn show_help(&mut self, app: &mut Program, ctx: u16) {
        let ctx = if self.help.topic_for_context(ctx).is_some() { ctx } else { 0 };
        let mut w = HelpWindow::new(self.help.clone(), ctx);
        let s = app.desktop().group.vs.size;
        let r = Rect::new(0, 0, (s.x - 4).min(76), (s.y - 2).min(22));
        w.win.group.change_bounds(r);
        dialogs::insert(app, w);
    }

    fn options(&mut self, app: &mut Program) {
        let (cmd, d) = dialogs::exec(app, dialogs::options_dialog(&self.cfg));
        if cmd != cm::OK {
            return;
        }
        let before = (self.cfg.lsp_enabled, self.cfg.lsp_command.clone(), self.cfg.toml_lsp_command.clone());
        dialogs::read_options(&mut self.cfg, &d.get_data());
        for id in Self::windows::<CodeWindow>(app) {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                self.editor_cfg(w.editor_mut());
            }
        }
        // Сервер LSP включили, выключили или сменили — перезапуск.
        if before != (self.cfg.lsp_enabled, self.cfg.lsp_command.clone(), self.cfg.toml_lsp_command.clone()) {
            if self.cfg.lsp_enabled && self.project.is_some() {
                self.lsp_restart(app);
            } else {
                self.lsp_stop(app);
            }
        }
        app.palette = self.cfg.palette();
        if !self.headless {
            self.cfg.save();
        }
    }

    /// Обработать команду. `true` — команда наша.
    fn command(&mut self, app: &mut Program, c: u16, info: &Info) -> bool {
        match c {
            CM_NEW_PROJECT => self.new_project(app),
            CM_OPEN_PROJECT => self.choose_project_file(app),
            CM_RECENT => self.recent(app),
            CM_IMPORT => self.import(app),
            CM_CLOSE_PROJECT => {
                self.close_project(app);
            }
            cm::NEW => {
                self.open_file(app, Path::new(""), None);
            }
            cm::OPEN => {
                let d = FileDialog::new("*.rs", "Открыть файл", "~И~мя", fd::OPEN_BUTTON, 111);
                let (cmd, d) = dialogs::exec(app, d);
                if cmd != cm::CANCEL {
                    let f = d.file_name();
                    if f.extension().map(|x| x == PROJECT_EXT).unwrap_or(false) {
                        self.open_project(app, &f);
                    } else {
                        self.open_file(app, &f, None);
                    }
                }
            }
            cm::SAVE_ALL => {
                let e = self.save_all(app);
                if !e.is_empty() {
                    Self::error(app, &e.join("\n"));
                }
            }
            cm::DOS_SHELL => app.dos_shell("Введите exit, чтобы вернуться в TV Studio."),
            cm::CLOSE_ALL => {
                for id in Self::windows::<CodeWindow>(app).into_iter().chain(Self::windows::<DesignWindow>(app)) {
                    app.put_event(Event::Command(Message::with(cm::CLOSE, Info::Id(id))));
                }
            }
            cm::UNDO | cm::REDO => {
                // Отмена из меню, когда фокус в инспекторе, — в верхней форме.
                let Some(doc) = Self::top_doc(app) else { return false };
                if c == cm::UNDO {
                    doc.borrow_mut().undo()
                } else {
                    doc.borrow_mut().redo()
                }
            }
            CM_GOTO => {
                let Some(id) = Self::current_window(app).filter(|&id| app.find::<CodeWindow>(id).is_some()) else { return true };
                if let Some(s) = input_box(app, "Перейти к строке", "~Н~омер строки", "", 10) {
                    if let Ok(n) = s.trim().parse::<usize>() {
                        if let Some(w) = app.find_mut::<CodeWindow>(id) {
                            w.goto(n.max(1), 1);
                        }
                    }
                }
            }
            CM_FIND_FILES => self.find_in_files(app),
            CM_COMPLETE => {
                let quiet = matches!(&info, Info::Str(s) if s == crate::codeedit::AUTO_COMPLETE);
                let trigger = match info {
                    Info::Str(s) if !quiet => s.chars().last(),
                    _ => None,
                };
                self.lsp_complete(app, trigger, quiet);
            }
            CM_GOTO_DEF => self.lsp_goto_def(app),
            CM_FIND_REFS => self.lsp_find_refs(app),
            CM_NAV_BACK => self.nav_back(app),
            CM_DIAGNOSTICS => self.list_diagnostics(app),
            CM_CODE_ACTIONS => self.lsp_code_actions(app),
            CM_LSP_RESTART => self.lsp_restart(app),
            CM_NEXT_ERR => self.next_error(app, true),
            CM_PREV_ERR => self.next_error(app, false),
            CM_MSG_GOTO => {
                let i = match info {
                    Info::Int(n) => *n as usize,
                    _ => self.messages.and_then(|id| app.find::<MessagesWindow>(id)).map(|m| m.focused()).unwrap_or(0),
                };
                self.goto_message(app, i);
            }
            CM_NEW_FORM => self.new_form(app),
            CM_OPEN_FORM => self.choose_form(app),
            CM_REMOVE_FORM => self.remove_form(app),
            CM_DS_PROPS => self.form_props(app),
            CM_FORM_TEST => self.test_form(app),
            CM_FORM_CODE => self.form_code(app),
            CM_GRID | CM_TABNUM | CM_DS_DUP | CM_DS_FRONT | CM_DS_BACK | CM_DS_FORWARD | CM_DS_BACKWARD => {
                let Some(doc) = Self::top_doc(app) else { return true };
                let mut d = doc.borrow_mut();
                match c {
                    CM_GRID => {
                        d.grid = !d.grid;
                        d.touch();
                    }
                    CM_TABNUM => {
                        d.show_order = !d.show_order;
                        d.touch();
                    }
                    CM_DS_DUP => d.duplicate(),
                    CM_DS_FRONT => d.reorder(Reorder::Front),
                    CM_DS_BACK => d.reorder(Reorder::Back),
                    CM_DS_FORWARD => d.reorder(Reorder::Forward),
                    _ => d.reorder(Reorder::Backward),
                }
            }
            c if (CM_ALIGN_BASE..CM_ALIGN_BASE + Align::ALL.len() as u16).contains(&c) => {
                if let Some(doc) = Self::top_doc(app) {
                    doc.borrow_mut().align(Align::ALL[(c - CM_ALIGN_BASE) as usize]);
                }
            }
            c if (CM_INS_BASE..CM_INS_BASE + Kind::ALL.len() as u16).contains(&c) => {
                self.insert_kind(app, Kind::ALL[(c - CM_INS_BASE) as usize]);
            }
            CM_PAL_INSERT => {
                let kind = self.inspector.and_then(|id| app.find::<InspectorWindow>(id)).map(|w| w.selected_kind());
                if let Some(k) = kind {
                    self.insert_kind(app, k);
                }
            }
            CM_PROJ_OPEN => {
                let data = self.project_win.and_then(|id| app.find::<ProjectWindow>(id)).and_then(|w| w.current_data()).unwrap_or_default();
                self.open_tree_item(app, &data);
            }
            CM_EDIT_MENU => self.edit_model(app, LeKind::Menu),
            CM_EDIT_STATUS => self.edit_model(app, LeKind::Status),
            CM_EDIT_COMMANDS => self.edit_model(app, LeKind::Commands),
            CM_APP_OPTIONS => self.app_options(app),
            CM_EXTRACT_FORMS => self.extract_forms(app),
            CM_GENERATE => {
                self.save_all(app);
                self.generate(app, false);
            }
            CM_CHECK => self.cargo(app, &["check", "--message-format=short"], "Проверка", After::Nothing),
            CM_BUILD => self.cargo(app, &["build", "--message-format=short"], "Сборка", After::Nothing),
            CM_RUN => self.cargo(app, &["build", "--message-format=short"], "Сборка", After::Run),
            CM_TEST => self.cargo(app, &["test", "--message-format=short"], "Тесты", After::Nothing),
            CM_CLEAN => self.cargo(app, &["clean"], "Очистка", After::Nothing),
            CM_BUILD_OPTS => self.build_options(app),
            CM_STOP => {
                if let Some(j) = &self.job {
                    j.kill();
                    self.say(app, "— Остановлено.");
                }
            }
            CM_SHOW_PALETTE | CM_SHOW_INSPECTOR | CM_SHOW_PROJECT | CM_SHOW_MESSAGES => {
                self.ensure_tools(app);
                let id = match c {
                    CM_SHOW_PALETTE | CM_SHOW_INSPECTOR => self.inspector,
                    CM_SHOW_PROJECT => self.project_win,
                    _ => self.messages,
                };
                if let Some(id) = id {
                    // F11: повторное нажатие из свойств инспектора возвращает в дизайнер.
                    let in_props = app.find::<InspectorWindow>(id).map(|w| !w.components_focused()).unwrap_or(false);
                    if c == CM_SHOW_INSPECTOR && Self::current_window(app) == Some(id) && in_props {
                        if let Some(d) = Self::top_designer(app) {
                            app.focus(d);
                            return true;
                        }
                    }
                    app.focus(id);
                    // «Компоненты» — к списку компонентов, F11 — к свойствам.
                    if let Some(w) = app.find_mut::<InspectorWindow>(id) {
                        w.focus_part(c == CM_SHOW_PALETTE);
                    }
                }
            }
            CM_SHOW_PROBLEMS => self.show_problems(app),
            CM_PROB_GOTO => {
                let p = self.problems.and_then(|id| app.find::<ProblemsWindow>(id)).and_then(|w| w.current().cloned());
                if let Some(p) = p {
                    if !p.path.exists() {
                        Self::error(app, &format!("Файл не найден: {}", p.path.display()));
                        return true;
                    }
                    self.open_file(app, &p.path, Some((p.line, p.col)));
                }
            }
            CM_LAYOUT => self.apply_layout(app),
            CM_OPTIONS => self.options(app),
            CM_ABOUT => Self::about(app),
            CM_KEYS => self.show_help(app, HC_KEYS),
            cm::HELP => {
                let ctx = app.help_ctx();
                self.show_help(app, ctx);
            }
            _ => return false,
        }
        true
    }

    /// Добавить элемент на верхнюю форму и перейти в её дизайнер.
    fn insert_kind(&mut self, app: &mut Program, kind: Kind) {
        let Some(id) = Self::top_designer(app) else {
            return Self::info_box(app, "\x03Сначала откройте форму:\n\x03Форма → Новая форма или дерево проекта.");
        };
        if let Some(doc) = Self::top_doc(app) {
            doc.borrow_mut().insert(kind);
        }
        app.focus(id);
    }

    fn open_tree_item(&mut self, app: &mut Program, data: &str) {
        if let Some(f) = data.strip_prefix("form:") {
            self.open_form(app, f);
        } else if let Some(f) = data.strip_prefix("file:") {
            self.open_file(app, Path::new(f), None);
        } else {
            match data {
                "model:menu" => self.edit_model(app, LeKind::Menu),
                "model:status" => self.edit_model(app, LeKind::Status),
                "model:commands" => self.edit_model(app, LeKind::Commands),
                "model:options" => self.app_options(app),
                _ => {}
            }
        }
    }
}

/// Имена стандартных команд закрытия диалога.
const BUILTIN_NAME: &[(u16, &str)] = &[(cm::OK, "cm::OK"), (cm::CANCEL, "cm::CANCEL"), (cm::YES, "cm::YES"), (cm::NO, "cm::NO")];

/// Один и тот же файл (с учётом относительных путей и регистра в Windows).
pub fn same_file(a: &Path, b: &Path) -> bool {
    if a.as_os_str().is_empty() || b.as_os_str().is_empty() {
        return false;
    }
    let ca = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let cb = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    if cfg!(windows) {
        ca.to_string_lossy().to_lowercase() == cb.to_string_lossy().to_lowercase()
    } else {
        ca == cb
    }
}

/// Путь из сообщения cargo → абсолютный.
fn resolve(dir: &Path, f: &str) -> PathBuf {
    let p = PathBuf::from(f);
    if p.is_absolute() {
        p
    } else {
        dir.join(p)
    }
}

/// Файлы проекта для поиска: исходники, формы, Cargo.toml (без target).
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() {
            if name != "target" && !name.starts_with('.') {
                collect_files(&p, out, depth + 1);
            }
        } else if ["rs", "toml", FORM_EXT, "md", "txt"].iter().any(|e| p.extension().map(|x| x == *e).unwrap_or(false)) {
            out.push(p);
        }
    }
}

/// Файл проекта в каталоге.
fn find_project_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().map(|x| x == PROJECT_EXT).unwrap_or(false))
}

impl AppHandler for Ide {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        if let Event::Command(m) = ev.clone() {
            if self.command(app, m.command, &m.info) {
                ev.clear();
            }
        }
    }

    fn idle(&mut self, app: &mut Program) {
        self.poll_job(app);
        self.collect_fmt_errors(app);
        self.lsp_idle(app);
        self.sync_windows(app);
        self.update_problems(app);
        self.update_commands(app);
    }
}
