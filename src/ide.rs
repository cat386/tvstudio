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

use crate::cmds::*;
use crate::dialogs::Config;
use crate::lsp::LspClient;
use crate::model::{Project, PROJECT_EXT};
use crate::toolwins::{MsgEntry, MsgKind};
use crate::runner::Job;
mod app_model_glue;
mod menu;
mod build_glue;
mod commands_glue;
mod forms_glue;
mod git_glue;
mod lifecycle_glue;
mod lsp_glue;
mod project_glue;
mod ui_glue;
mod windows_glue;
pub use lsp_glue::{LspState, Srv};
pub use menu::{menu_bar, status_line};

/// Текст справки (формат tvhc), встроен в программу.
pub const HELP_SOURCE: &str = include_str!("help.txt");

// ======================================================================
// Среда
// ======================================================================

/// Обработчик среды.
pub struct Ide {
    start_page: Option<ViewId>,
    pub project: Option<Project>,
    pub cfg: Config,
    pub job: Option<Job>,
    git: git_glue::State,
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
        let help = HelpFile::parse(&HELP_SOURCE.replace("{{COMMAND_KEYS}}", &crate::ui_commands::help_keys()))
            .unwrap_or_else(|_| HelpFile::parse(".topic Nocontext=0\nСправка недоступна.\n").unwrap());
        Ide {
            start_page: None,
            project: None,
            cfg,
            job: None,
            git: git_glue::State::default(),
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

}

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

/// Файл проекта в каталоге.
pub(super) fn find_project_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().map(|x| x == PROJECT_EXT).unwrap_or(false))
}

/// Путь из сообщения cargo → абсолютный.
pub(super) fn resolve(dir: &Path, f: &str) -> PathBuf {
    let p = PathBuf::from(f);
    if p.is_absolute() {
        p
    } else {
        dir.join(p)
    }
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
        Ide::idle(self, app);
    }
}
