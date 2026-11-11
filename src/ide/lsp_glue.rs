//! Связь среды с серверами LSP: rust-analyzer для файлов `.rs` и
//! crates-lsp (cargo LSP) для `.toml` — версии и features зависимостей
//! в Cargo.toml.
//!
//! Серверов два, каждый со своей кодировкой столбцов и своими номерами
//! запросов ([`Srv`]); какой из них отвечает за файл — решает расширение.
//!
//! Здесь — всё, что знает и о редакторах кода, и о сервере:
//! * синхронизация открытых файлов (`didOpen`, `didChange` с задержкой
//!   после набора, `didSave` после сохранения, `didClose`);
//! * диагностика: пометки на полосе номеров, подсветка мест в тексте,
//!   сообщение текущей строки в строке состояния, список по команде;
//! * дополнение (Ctrl+Space, а также само после `.` и `::`);
//! * переход к определению (F12), назад (Alt+←), поиск ссылок (Shift+F12);
//! * подсказки в тексте от сервера Cargo.toml (crates-lsp: «✓» или новая
//!   версия зависимости) — серым в конце строки;
//! * быстрые исправления (Alt+Enter, Ctrl+.) — список действий сервера
//!   для места под курсором и применение выбранной правки;
//! * ход индексации — в заголовке окна сообщений.
//!
//! Всё асинхронно: запрос уходит сразу, ответ разбирается в простое
//! ([`Ide::lsp_idle`]) — среда не замирает, пока сервер думает.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use tvision::*;

use super::{same_file, Ide};
use crate::codeedit::{CodeWindow, Underline};
use crate::lsp::{
    col_from_lsp, col_to_lsp, CodeAction, Diagnostic, Encoding, InlayHint, LPos, LRange, Location, LspClient, LspEvent, Severity, TextEdit,
};
use crate::toolwins::{MessagesWindow, MsgEntry, MsgKind};

/// Пауза после последней правки, через которую текст уходит серверу.
pub const CHANGE_DELAY: Duration = Duration::from_millis(300);

/// Состояние синхронизации одного документа.
#[derive(Default)]
pub struct DocSync {
    /// Что сервер знает о тексте.
    sent: String,
    /// Когда текст впервые разошёлся с отправленным.
    changed: Option<Instant>,
    /// Флаг «изменён» на прошлой проверке (true → false значит «сохранён»).
    modified: bool,
}

/// Какой сервер отвечает за файл.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Srv {
    /// rust-analyzer: файлы `.rs`.
    Rust,
    /// crates-lsp (cargo LSP): файлы `.toml`, прежде всего Cargo.toml.
    Toml,
}

impl Srv {
    pub const ALL: [Srv; 2] = [Srv::Rust, Srv::Toml];

    /// Сервер для файла (по расширению); `None` — файл серверам не показывается.
    pub fn for_path(p: &Path) -> Option<Srv> {
        match p.extension().and_then(|x| x.to_str()) {
            Some("rs") => Some(Srv::Rust),
            Some("toml") => Some(Srv::Toml),
            _ => None,
        }
    }

    /// `languageId` документов.
    pub fn language(self) -> &'static str {
        match self {
            Srv::Rust => "rust",
            Srv::Toml => "toml",
        }
    }

    /// Название в сообщениях: «Сервер LSP …», «Сервер Cargo.toml …».
    pub fn title(self) -> &'static str {
        match self {
            Srv::Rust => "LSP",
            Srv::Toml => "Cargo.toml",
        }
    }
}

/// Чего ждём от сервера по номеру запроса.
#[derive(Clone, Debug)]
pub enum Wait {
    /// Дополнение в окне `win`; позиция курсора на момент запроса.
    Completion {
        win: ViewId,
        at: Pos,
    },
    Definition {
        from: (PathBuf, usize, usize),
    },
    References {
        word: String,
    },
    /// Подсказки в тексте для файла.
    Hints {
        path: PathBuf,
    },
    /// Быстрые исправления, запрошенные в окне `win`.
    Actions {
        win: ViewId,
    },
}

/// Всё, что среда держит для LSP.
#[derive(Default)]
pub struct LspState {
    pub docs: HashMap<PathBuf, DocSync>,
    /// Последние диагностики по файлам (как прислал сервер).
    pub diags: HashMap<PathBuf, Vec<Diagnostic>>,
    pub diags_dirty: bool,
    /// Подсказки в тексте по файлам (от сервера Cargo.toml — crates-lsp).
    pub inlay: HashMap<PathBuf, Vec<InlayHint>>,
    pub inlay_dirty: bool,
    /// Файлы, для которых пора запросить подсказки заново.
    pub inlay_want: HashSet<PathBuf>,
    /// Ожидаемые ответы: (сервер, номер запроса).
    pub wait: HashMap<(Srv, i64), Wait>,
    /// Откуда уходили переходом к определению (для Alt+←).
    pub nav: Vec<(PathBuf, usize, usize)>,
    /// Ход индексации («Indexing 40%»), пусто — сервер свободен.
    pub status: String,
    /// Имя сервера из ответа initialize.
    pub name: String,
    /// Имя сервера TOML (crates-lsp) из ответа initialize.
    pub toml_name: String,
    /// Сообщение диагностики текущей строки — для строки состояния.
    pub hint: Rc<RefCell<String>>,
    /// Запуск не удался — не пытаться снова до перезапуска вручную.
    pub failed: bool,
    pub toml_failed: bool,
}

impl Ide {
    // ------------------------------------------------------------------
    // Запуск и остановка
    // ------------------------------------------------------------------

    /// Клиент сервера `srv` (если запущен).
    pub fn client(&self, srv: Srv) -> Option<&LspClient> {
        match srv {
            Srv::Rust => self.lsp.as_ref(),
            Srv::Toml => self.lsp_toml.as_ref(),
        }
    }

    pub fn client_mut(&mut self, srv: Srv) -> Option<&mut LspClient> {
        match srv {
            Srv::Rust => self.lsp.as_mut(),
            Srv::Toml => self.lsp_toml.as_mut(),
        }
    }

    fn slot(&mut self, srv: Srv) -> &mut Option<LspClient> {
        match srv {
            Srv::Rust => &mut self.lsp,
            Srv::Toml => &mut self.lsp_toml,
        }
    }

    /// Запущен ли хотя бы один сервер.
    pub fn any_lsp(&self) -> bool {
        self.lsp.is_some() || self.lsp_toml.is_some()
    }

    /// Команда запуска сервера (пусто — не запускать).
    fn server_command(&self, srv: Srv) -> String {
        match srv {
            Srv::Rust => self.cfg.lsp_command.clone(),
            Srv::Toml => self.cfg.toml_lsp_command.trim().to_string(),
        }
    }

    /// Запустить серверы для открытого проекта (если включены в настройках).
    pub fn lsp_start(&mut self, app: &mut Program) {
        if !self.cfg.lsp_enabled {
            return;
        }
        let Some(dir) = self.base_dir() else { return };
        for srv in Srv::ALL {
            let cmd = self.server_command(srv);
            if self.client(srv).is_some() || cmd.is_empty() {
                continue;
            }
            let ok = match LspClient::start(&cmd, &dir, srv.language()) {
                Ok(c) => {
                    *self.slot(srv) = Some(c);
                    if srv == Srv::Rust {
                        self.lsp_state.status = "запуск…".into();
                    }
                    self.say(app, &format!("— Сервер {}: {} запускается", srv.title(), cmd));
                    true
                }
                Err(e) => {
                    self.say(app, &format!("Сервер {} недоступен: {}", srv.title(), e));
                    if srv == Srv::Rust && cmd.starts_with("rust-analyzer") {
                        self.say(app, "  Установите его: rustup component add rust-analyzer (или укажите путь в Настройки → Параметры среды).");
                    }
                    if srv == Srv::Toml && cmd.starts_with("crates-lsp") {
                        self.say(
                            app,
                            "  Установите его: cargo install --locked crates-lsp (или очистите поле «Сервер Cargo.toml» в Настройки → Параметры среды).",
                        );
                    }
                    false
                }
            };
            match srv {
                Srv::Rust => self.lsp_state.failed = !ok,
                Srv::Toml => self.lsp_state.toml_failed = !ok,
            }
        }
        self.show_lsp_status(app);
    }

    /// Остановить серверы и забыть их состояние.
    pub fn lsp_stop(&mut self, app: &mut Program) {
        for c in [self.lsp.take(), self.lsp_toml.take()].into_iter().flatten() {
            c.stop();
        }
        let hint = self.lsp_state.hint.clone();
        hint.borrow_mut().clear();
        self.lsp_state = LspState { hint, ..LspState::default() };
        self.apply_diags(app);
        self.show_lsp_status(app);
    }

    pub fn lsp_restart(&mut self, app: &mut Program) {
        self.lsp_stop(app);
        if !self.cfg.lsp_enabled {
            self.say(app, "Сервер LSP выключен в параметрах среды.");
            return;
        }
        if self.project.is_none() {
            self.say(app, "Сервер LSP работает с открытым проектом.");
            return;
        }
        self.lsp_start(app);
    }

    /// Состояние серверов — в заголовке окна сообщений.
    fn show_lsp_status(&mut self, app: &mut Program) {
        let rust = match &self.lsp {
            None => None,
            Some(_) if self.lsp_state.status.is_empty() => {
                Some(if self.lsp_state.name.is_empty() { "LSP".to_string() } else { self.lsp_state.name.clone() })
            }
            Some(_) => Some(format!("LSP: {}", self.lsp_state.status)),
        };
        let toml =
            self.lsp_toml
                .as_ref()
                .map(|_| if self.lsp_state.toml_name.is_empty() { "crates-lsp".to_string() } else { self.lsp_state.toml_name.clone() });
        let parts: Vec<String> = rust.into_iter().chain(toml).collect();
        let title = if parts.is_empty() { "Сообщения".to_string() } else { format!("Сообщения — {}", parts.join(" · ")) };
        if Self::alive(app, self.messages) {
            if let Some(m) = app.find_mut::<MessagesWindow>(self.messages.unwrap()) {
                if m.win.title != title {
                    m.win.title = title;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Простой: события сервера и синхронизация
    // ------------------------------------------------------------------

    /// Работа в простое: разобрать ответы, отправить изменения.
    pub fn lsp_idle(&mut self, app: &mut Program) {
        if !self.any_lsp() {
            if self.lsp_state.diags_dirty {
                self.apply_diags(app);
            }
            return;
        }
        for srv in Srv::ALL {
            let Some(c) = self.client_mut(srv) else { continue };
            let events = c.poll();
            for e in events {
                self.lsp_event(app, srv, e);
                if self.client(srv).is_none() {
                    break;
                }
            }
        }
        self.sync_docs(app, false);
        self.request_hints(app);
        if self.lsp_state.diags_dirty {
            self.apply_diags(app);
        }
        if self.lsp_state.inlay_dirty {
            self.apply_hints(app);
        }
        self.update_diag_hint(app);
    }

    // ------------------------------------------------------------------
    // Подсказки в тексте
    // ------------------------------------------------------------------

    /// Подсказки в тексте берутся только у сервера Cargo.toml: у crates-lsp
    /// они стоят в конце строки зависимости. Подсказки rust-analyzer (типы
    /// посреди строки) в конце строки только путали бы.
    fn hints_server(&self) -> Option<&LspClient> {
        self.lsp_toml.as_ref().filter(|c| c.ready && c.has_inlay_hints)
    }

    /// Запросить подсказки для файлов из `inlay_want` (по одному запросу
    /// на файл за раз: новый — когда придёт ответ на прежний).
    fn request_hints(&mut self, app: &Program) {
        if self.lsp_state.inlay_want.is_empty() || self.hints_server().is_none() {
            return;
        }
        let waiting: Vec<PathBuf> =
            self.lsp_state.wait.values().filter_map(|w| if let Wait::Hints { path } = w { Some(path.clone()) } else { None }).collect();
        let want: Vec<PathBuf> = self.lsp_state.inlay_want.iter().filter(|p| !waiting.contains(p)).cloned().collect();
        for path in want {
            self.lsp_state.inlay_want.remove(&path);
            let lines = Self::lsp_windows(app)
                .iter()
                .find(|(_, p, _)| *p == path)
                .and_then(|(id, _, _)| app.find::<CodeWindow>(*id))
                .map(|w| w.editor().fe.editor.line_count())
                .unwrap_or(0);
            let Some(c) = self.lsp_toml.as_mut() else { return };
            if !c.is_open(&path) {
                continue;
            }
            let rid = c.inlay_hints(&path, lines + 1);
            self.lsp_state.wait.insert((Srv::Toml, rid), Wait::Hints { path });
        }
    }

    /// Подсказки — в редакторы: по строке, через два пробела.
    fn apply_hints(&mut self, app: &mut Program) {
        self.lsp_state.inlay_dirty = false;
        for id in Self::windows::<CodeWindow>(app) {
            let Some(path) = app.find::<CodeWindow>(id).map(|w| w.path()) else { continue };
            let list = self.lsp_state.inlay.get(&path).or_else(|| self.lsp_state.inlay.iter().find(|(k, _)| same_file(k, &path)).map(|(_, v)| v));
            let mut by_line: Vec<(usize, String)> = Vec::new();
            for h in list.into_iter().flatten() {
                match by_line.iter_mut().find(|(l, _)| *l == h.pos.line) {
                    Some((_, t)) => {
                        t.push_str("  ");
                        t.push_str(&h.label);
                    }
                    None => by_line.push((h.pos.line, h.label.clone())),
                }
            }
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                w.editor_mut().hints = by_line;
            }
        }
    }

    // ------------------------------------------------------------------
    // Быстрые исправления
    // ------------------------------------------------------------------

    /// Запросить быстрые исправления для места под курсором (или выделения).
    pub fn lsp_code_actions(&mut self, app: &mut Program) {
        let Some((id, path, pos, srv)) = self.lsp_target(app) else { return };
        if !self.client(srv).map(|c| c.has_code_actions).unwrap_or(false) {
            return self.say(app, &format!("Сервер {} не предлагает быстрых исправлений.", srv.title()));
        }
        let sel = app.find::<CodeWindow>(id).and_then(|w| w.editor().fe.editor.selection());
        let (a, b) = sel.unwrap_or((pos, pos));
        let r = LRange { start: self.lsp_pos(app, srv, id, a), end: self.lsp_pos(app, srv, id, b) };
        // Диагностики, задевающие строки запроса.
        let diags: Vec<Diagnostic> = self
            .diags_for(&path)
            .map(|v| v.iter().filter(|d| d.range.start.line <= r.end.line && d.range.end.line >= r.start.line).cloned().collect())
            .unwrap_or_default();
        let Some(rid) = self.client_mut(srv).map(|c| c.code_actions(&path, r, &diags)) else { return };
        self.lsp_state.wait.insert((srv, rid), Wait::Actions { win: id });
    }

    /// Показать список исправлений и применить выбранное.
    fn show_code_actions(&mut self, app: &mut Program, srv: Srv, items: Vec<CodeAction>) {
        if items.is_empty() {
            return self.say(app, "Быстрых исправлений здесь нет.");
        }
        let titles: Vec<String> = items.iter().map(|a| a.title.clone()).collect();
        let Some(i) = crate::dialogs::choose(app, "Быстрые исправления", &titles) else { return };
        let action = items[i].clone();
        self.apply_workspace_edit(app, srv, &action.edits);
        self.say(app, &format!("Исправление: {}", action.title));
    }

    /// Применить правки сервера: файлы открываются в редакторе (если ещё не
    /// открыты), правки делаются там же — сохранить их можно как обычно.
    pub fn apply_workspace_edit(&mut self, app: &mut Program, srv: Srv, edits: &[(PathBuf, Vec<TextEdit>)]) {
        let enc = self.encoding(srv);
        for (path, list) in edits {
            let open = Self::windows::<CodeWindow>(app)
                .into_iter()
                .find(|&id| app.find::<CodeWindow>(id).map(|w| same_file(&w.path(), path)).unwrap_or(false));
            let Some(id) = open.or_else(|| self.open_file(app, path, None)) else { continue };
            let Some(w) = app.find_mut::<CodeWindow>(id) else { continue };
            apply_text_edits(&mut w.editor_mut().fe.editor, list, enc);
        }
        // Полосы прокрутки и индикатор обновятся по «пустому» оповещению.
        app.put_event(Event::Broadcast(Message::new(0xFFFE)));
        self.sync_docs(app, true);
    }

    fn lsp_event(&mut self, app: &mut Program, srv: Srv, e: LspEvent) {
        match e {
            LspEvent::Ready { name } => {
                let name = if name.is_empty() { self.server_command(srv) } else { name };
                match srv {
                    Srv::Rust => {
                        self.lsp_state.name = name.clone();
                        self.lsp_state.status.clear();
                    }
                    Srv::Toml => self.lsp_state.toml_name = name.clone(),
                }
                self.say(app, &format!("— Сервер {} готов: {}", srv.title(), name));
                self.show_lsp_status(app);
            }
            LspEvent::Diagnostics { path, items } => {
                // crates-lsp присылает диагностику, когда узнал версии, — время обновить и подсказки.
                if srv == Srv::Toml {
                    self.lsp_state.inlay_want.insert(path.clone());
                }
                self.lsp_state.diags.insert(path, items);
                self.lsp_state.diags_dirty = true;
            }
            // Ход индексации — только у rust-analyzer.
            LspEvent::Progress { .. } if srv == Srv::Toml => {}
            LspEvent::Progress { title, percent, done } => {
                self.lsp_state.status = if done {
                    String::new()
                } else {
                    match percent {
                        Some(p) => format!("{} {}%", title, p),
                        None => title,
                    }
                };
                self.show_lsp_status(app);
            }
            LspEvent::Message(m) => self.say(app, &format!("{}: {}", srv.title(), m)),
            LspEvent::Error { id, message } => {
                // Отменённые сервером запросы (текст изменился) — не ошибка для пользователя.
                if let Some(w) = self.lsp_state.wait.remove(&(srv, id)) {
                    if !matches!(w, Wait::Completion { .. }) && !message.contains("content modified") {
                        self.say(app, &format!("{}: {}", srv.title(), message));
                    }
                }
            }
            LspEvent::Completion { id, items } => {
                if let Some(Wait::Completion { win, at }) = self.lsp_state.wait.remove(&(srv, id)) {
                    self.show_completion(app, srv, win, at, items);
                }
            }
            LspEvent::Locations { id, items } => match self.lsp_state.wait.remove(&(srv, id)) {
                Some(Wait::Definition { from }) => self.show_definition(app, srv, from, items),
                Some(Wait::References { word }) => self.show_references(app, srv, &word, items),
                _ => {}
            },
            LspEvent::InlayHints { id, items } => {
                if let Some(Wait::Hints { path }) = self.lsp_state.wait.remove(&(srv, id)) {
                    self.lsp_state.inlay.insert(path, items);
                    self.lsp_state.inlay_dirty = true;
                }
            }
            LspEvent::CodeActions { id, items } => {
                if let Some(Wait::Actions { .. }) = self.lsp_state.wait.remove(&(srv, id)) {
                    self.show_code_actions(app, srv, items);
                }
            }
            // Сервер узнал новое (версии с crates.io) — подсказки заново для всех его файлов.
            LspEvent::RefreshHints => {
                let docs: Vec<PathBuf> = self.client(srv).map(|c| c.open_docs()).unwrap_or_default();
                self.lsp_state.inlay_want.extend(docs);
            }
            LspEvent::Exited => {
                let tail = self.client(srv).map(|c| c.stderr_tail.lock().map(|t| t.clone()).unwrap_or_default()).unwrap_or_default();
                self.say(app, &format!("Сервер {} завершил работу. Поиск → Перезапустить сервер LSP — запустить снова.", srv.title()));
                for l in tail.iter().rev().take(3).rev() {
                    self.say(app, &format!("  {}", l));
                }
                if let Some(c) = self.slot(srv).take() {
                    c.stop();
                }
                // Забыть всё, что относилось к этому серверу; второй работает дальше.
                let st = &mut self.lsp_state;
                st.diags.retain(|p, _| Srv::for_path(p) != Some(srv));
                st.docs.retain(|p, _| Srv::for_path(p) != Some(srv));
                st.wait.retain(|(s, _), _| *s != srv);
                st.inlay.retain(|p, _| Srv::for_path(p) != Some(srv));
                st.inlay_want.retain(|p| Srv::for_path(p) != Some(srv));
                st.diags_dirty = true;
                st.inlay_dirty = true;
                match srv {
                    Srv::Rust => {
                        st.status.clear();
                        st.name.clear();
                        st.failed = true;
                    }
                    Srv::Toml => {
                        st.toml_name.clear();
                        st.toml_failed = true;
                    }
                }
                self.show_lsp_status(app);
            }
        }
    }

    /// Окна кода, текст которых показывается серверам (файлы `.rs` и `.toml`).
    fn lsp_windows(app: &Program) -> Vec<(ViewId, PathBuf, Srv)> {
        Self::windows::<CodeWindow>(app)
            .into_iter()
            .filter_map(|id| app.find::<CodeWindow>(id).map(|w| (id, w.path())))
            .filter_map(|(id, p)| Srv::for_path(&p).map(|s| (id, p, s)))
            .collect()
    }

    /// Открыть, обновить и закрыть документы на серверах.
    /// `now` — отправить изменения без задержки (перед запросом).
    pub fn sync_docs(&mut self, app: &mut Program, now: bool) {
        if !self.any_lsp() {
            return;
        }
        let wins = Self::lsp_windows(app);
        for (id, path, srv) in &wins {
            let lsp = match srv {
                Srv::Rust => self.lsp.as_mut(),
                Srv::Toml => self.lsp_toml.as_mut(),
            };
            let Some(lsp) = lsp else { continue };
            let Some(w) = app.find::<CodeWindow>(*id) else { continue };
            let ed = &w.editor().fe.editor;
            let modified = ed.modified;
            if !lsp.is_open(path) {
                let text = ed.text();
                lsp.did_open(path, &text);
                self.lsp_state.docs.insert(path.clone(), DocSync { sent: text, changed: None, modified });
                if *srv == Srv::Toml {
                    self.lsp_state.inlay_want.insert(path.clone());
                }
                continue;
            }
            let doc = self.lsp_state.docs.entry(path.clone()).or_default();
            let text = ed.text();
            if text != doc.sent {
                let t0 = *doc.changed.get_or_insert_with(Instant::now);
                if now || t0.elapsed() >= CHANGE_DELAY {
                    lsp.did_change(path, &text);
                    doc.sent = text;
                    doc.changed = None;
                    if *srv == Srv::Toml {
                        self.lsp_state.inlay_want.insert(path.clone());
                    }
                }
            } else {
                doc.changed = None;
            }
            // Сохранили (флаг «изменён» снялся) — сервер перепроверит файл.
            if doc.modified && !modified && doc.changed.is_none() {
                lsp.did_save(path);
            }
            doc.modified = modified;
        }
        // Закрытые окна.
        for lsp in [self.lsp.as_mut(), self.lsp_toml.as_mut()].into_iter().flatten() {
            for p in lsp.open_docs() {
                if !wins.iter().any(|(_, w, _)| *w == p) {
                    lsp.did_close(&p);
                    self.lsp_state.docs.remove(&p);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Диагностика
    // ------------------------------------------------------------------

    /// Диагностики файла `p` (если сервер о нём что-то прислал).
    fn diags_for(&self, p: &Path) -> Option<&Vec<Diagnostic>> {
        self.lsp_state.diags.get(p).or_else(|| self.lsp_state.diags.iter().find(|(k, _)| same_file(k, p)).map(|(_, v)| v))
    }

    /// Кодировка столбцов сервера `srv`.
    fn encoding(&self, srv: Srv) -> Encoding {
        self.client(srv).map(|c| c.encoding).unwrap_or(Encoding::Utf16)
    }

    /// Кодировка столбцов сервера, отвечающего за файл.
    fn encoding_for(&self, p: &Path) -> Encoding {
        Srv::for_path(p).map(|s| self.encoding(s)).unwrap_or(Encoding::Utf16)
    }

    /// Разнести диагностики по окнам: полоса номеров и подсветка в тексте.
    /// Для файлов, о которых сервер прислал диагностику, она заменяет
    /// пометки последней сборки (она свежее).
    pub fn apply_diags(&mut self, app: &mut Program) {
        self.lsp_state.diags_dirty = false;
        for id in Self::windows::<CodeWindow>(app) {
            let Some(path) = app.find::<CodeWindow>(id).map(|w| w.path()) else { continue };
            let enc = self.encoding_for(&path);
            let lsp = self.diags_for(&path).cloned();
            let build = self.marks.iter().find(|(k, _)| same_file(k, &path)).map(|(_, v)| v.clone()).unwrap_or_default();
            let Some(w) = app.find_mut::<CodeWindow>(id) else { continue };
            match lsp {
                Some(list) => {
                    let mut marks: Vec<(usize, MsgKind)> = Vec::new();
                    let mut under = Vec::new();
                    for d in list.iter().filter(|d| d.severity <= Severity::Warning) {
                        let error = d.severity == Severity::Error;
                        let kind = if error { MsgKind::Error } else { MsgKind::Warning };
                        match marks.iter_mut().find(|(l, _)| *l == d.range.start.line) {
                            Some(m) if error => m.1 = MsgKind::Error,
                            Some(_) => {}
                            None => marks.push((d.range.start.line, kind)),
                        }
                        // Многострочный диапазон — подсвечиваем каждую строку.
                        let ed = &w.editor().fe.editor;
                        for line in d.range.start.line..=d.range.end.line.min(d.range.start.line + 50) {
                            if line >= ed.line_count() {
                                break;
                            }
                            let text = ed.line(line);
                            let c0 = if line == d.range.start.line { col_from_lsp(&text, d.range.start.col, enc) } else { 0 };
                            let c1 = if line == d.range.end.line { col_from_lsp(&text, d.range.end.col, enc) } else { text.chars().count() };
                            under.push(Underline { line, c0, c1: c1.max(c0), error });
                        }
                    }
                    w.set_marks(marks);
                    w.editor_mut().diags = under;
                }
                None => {
                    w.set_marks(build);
                    w.editor_mut().diags.clear();
                }
            }
        }
    }

    /// Сообщение диагностики строки под курсором — в строку состояния.
    fn update_diag_hint(&mut self, app: &mut Program) {
        let mut text = String::new();
        if let Some(w) = Self::current_window(app).and_then(|id| app.find::<CodeWindow>(id)) {
            let line = w.editor().fe.editor.cursor().line;
            if let Some(list) = self.diags_for(&w.path()) {
                let mut on: Vec<&Diagnostic> = list.iter().filter(|d| d.range.start.line <= line && line <= d.range.end.line).collect();
                on.sort_by_key(|d| d.severity);
                if let Some(d) = on.first() {
                    let mark = if d.severity == Severity::Error { "■" } else { "▲" };
                    text = format!("{} {}", mark, d.message.lines().next().unwrap_or(""));
                }
            }
        }
        let mut h = self.lsp_state.hint.borrow_mut();
        if *h != text {
            *h = text;
        }
    }

    /// Все диагностики сервера — в окно сообщений (Enter — к месту).
    pub fn list_diagnostics(&mut self, app: &mut Program) {
        if !self.any_lsp() {
            return self.say(app, "Сервер LSP не запущен.");
        }
        let mut all: Vec<(PathBuf, Diagnostic)> =
            self.lsp_state.diags.iter().flat_map(|(p, v)| v.iter().map(move |d| (p.clone(), d.clone()))).collect();
        all.sort_by(|a, b| (a.1.severity, &a.0, a.1.range.start).cmp(&(b.1.severity, &b.0, b.1.range.start)));
        let base = self.base_dir();
        self.msgs(app).clear();
        self.say(
            app,
            &format!("— Диагностика сервера LSP: {}", if all.is_empty() { "замечаний нет".to_string() } else { format!("{} шт.", all.len()) }),
        );
        for (p, d) in all {
            let line_text = self.line_text(app, &p, d.range.start.line);
            let col = col_from_lsp(&line_text, d.range.start.col, self.encoding_for(&p)) + 1;
            let shown =
                base.as_ref().and_then(|b| p.strip_prefix(b).ok()).map(|r| r.display().to_string()).unwrap_or_else(|| p.display().to_string());
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
                Severity::Hint => "hint",
            };
            let code = if d.code.is_empty() { String::new() } else { format!("[{}]", d.code) };
            let kind = match d.severity {
                Severity::Error => MsgKind::Error,
                Severity::Warning => MsgKind::Warning,
                _ => MsgKind::Note,
            };
            let msg = d.message.lines().next().unwrap_or("").to_string();
            self.msgs(app).push(MsgEntry {
                text: format!("{}:{}:{}: {}{}: {}", shown, d.range.start.line + 1, col, sev, code, msg),
                kind,
                loc: Some((p.display().to_string(), d.range.start.line + 1, col)),
            });
        }
        if let Some(id) = self.messages {
            app.focus(id);
        }
    }

    /// Строка файла: из открытого редактора, иначе с диска.
    fn line_text(&self, app: &Program, p: &Path, line: usize) -> String {
        for id in Self::windows::<CodeWindow>(app) {
            if let Some(w) = app.find::<CodeWindow>(id) {
                if same_file(&w.path(), p) {
                    let e = &w.editor().fe.editor;
                    return if line < e.line_count() { e.line(line) } else { String::new() };
                }
            }
        }
        std::fs::read_to_string(p).ok().and_then(|s| s.lines().nth(line).map(|l| l.to_string())).unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // Запросы
    // ------------------------------------------------------------------

    /// Текущее окно кода с файлом `.rs` или `.toml`: id, путь, курсор, сервер.
    fn lsp_target(&mut self, app: &mut Program) -> Option<(ViewId, PathBuf, Pos, Srv)> {
        let id = Self::current_window(app)?;
        let w = app.find::<CodeWindow>(id)?;
        let path = w.path();
        let pos = w.editor().fe.editor.cursor();
        let Some(srv) = Srv::for_path(&path) else {
            self.say(app, "Сервер LSP работает с файлами .rs и .toml (сохраните файл с именем).");
            return None;
        };
        if self.client(srv).is_none() {
            let why = if !self.cfg.lsp_enabled {
                "Сервер LSP выключен (Настройки → Параметры среды).".to_string()
            } else if self.project.is_none() {
                "Сервер LSP работает с открытым проектом.".to_string()
            } else if self.server_command(srv).is_empty() {
                format!("Сервер {} не задан (Настройки → Параметры среды).", srv.title())
            } else {
                format!("Сервер {} не запущен (Поиск → Перезапустить сервер LSP).", srv.title())
            };
            self.say(app, &why);
            return None;
        }
        // Сервер должен видеть текущий текст.
        self.sync_docs(app, true);
        Some((id, path, pos, srv))
    }

    /// Позиция для сервера.
    fn lsp_pos(&self, app: &Program, srv: Srv, id: ViewId, pos: Pos) -> LPos {
        let line = app.find::<CodeWindow>(id).map(|w| w.editor().fe.editor.line(pos.line)).unwrap_or_default();
        LPos { line: pos.line, col: col_to_lsp(&line, pos.col, self.encoding(srv)) }
    }

    /// Запросить дополнение в позиции курсора.
    pub fn lsp_complete(&mut self, app: &mut Program, trigger: Option<char>) {
        let Some((id, path, pos, srv)) = self.lsp_target(app) else { return };
        let lp = self.lsp_pos(app, srv, id, pos);
        let Some(rid) = self.client_mut(srv).map(|c| c.completion(&path, lp, trigger)) else { return };
        // Старые запросы дополнения больше не нужны.
        self.lsp_state.wait.retain(|_, w| !matches!(w, Wait::Completion { .. }));
        self.lsp_state.wait.insert((srv, rid), Wait::Completion { win: id, at: pos });
    }

    fn show_completion(&mut self, app: &mut Program, srv: Srv, win: ViewId, at: Pos, items: Vec<crate::lsp::CompletionItem>) {
        let enc = self.encoding(srv);
        let Some(w) = app.find_mut::<CodeWindow>(win) else { return };
        let ed = w.editor_mut();
        let cur = ed.fe.editor.cursor();
        // Курсор ушёл на другую строку или левее места запроса — ответ устарел.
        if cur.line != at.line || cur.col < at.col {
            return;
        }
        // Начало замены: из textEdit первого варианта, иначе — начало слова.
        let line = ed.fe.editor.line(cur.line);
        let ws = ed.word_start();
        let start = items
            .iter()
            .find_map(|i| i.range)
            .filter(|r| r.start.line == cur.line)
            .map(|r| Pos::new(cur.line, col_from_lsp(&line, r.start.col, enc)))
            .filter(|p| p.col <= cur.col && p.col >= ws.col.min(at.col))
            .unwrap_or(ws);
        ed.open_completion(items, start);
        if ed.completion.is_none() {
            self.say(app, "Дополнение: подходящих вариантов нет.");
        }
    }

    /// Перейти к определению символа под курсором.
    pub fn lsp_goto_def(&mut self, app: &mut Program) {
        let Some((id, path, pos, srv)) = self.lsp_target(app) else { return };
        let lp = self.lsp_pos(app, srv, id, pos);
        let Some(rid) = self.client_mut(srv).map(|c| c.definition(&path, lp)) else { return };
        self.lsp_state.wait.insert((srv, rid), Wait::Definition { from: (path, pos.line + 1, pos.col + 1) });
    }

    /// Открыть файл места и поставить курсор (столбец сервера `srv` → символы).
    fn open_location(&mut self, app: &mut Program, srv: Srv, loc: &Location) {
        let enc = self.encoding(srv);
        if let Some(id) = self.open_file(app, &loc.path, None) {
            if let Some(w) = app.find_mut::<CodeWindow>(id) {
                let line = loc.range.start.line;
                let text = if line < w.editor().fe.editor.line_count() { w.editor().fe.editor.line(line) } else { String::new() };
                let col = col_from_lsp(&text, loc.range.start.col, enc);
                w.goto(line + 1, col + 1);
            }
            app.focus(id);
        }
    }

    fn show_definition(&mut self, app: &mut Program, srv: Srv, from: (PathBuf, usize, usize), items: Vec<Location>) {
        match items.len() {
            0 => self.say(app, "Определение не найдено."),
            n => {
                self.lsp_state.nav.push(from);
                if self.lsp_state.nav.len() > 50 {
                    self.lsp_state.nav.remove(0);
                }
                if n > 1 {
                    // Несколько определений (например, макрос, типаж) — список и переход к первому.
                    self.show_locations(app, srv, &format!("— Определения: {}", n), &items);
                }
                self.open_location(app, srv, &items[0]);
            }
        }
    }

    /// Вернуться туда, откуда ушли переходом к определению.
    pub fn nav_back(&mut self, app: &mut Program) {
        let Some((p, l, c)) = self.lsp_state.nav.pop() else {
            return self.say(app, "Возвращаться некуда.");
        };
        if let Some(id) = self.open_file(app, &p, Some((l, c))) {
            app.focus(id);
        }
    }

    /// Найти все ссылки на символ под курсором.
    pub fn lsp_find_refs(&mut self, app: &mut Program) {
        let Some((id, path, pos, srv)) = self.lsp_target(app) else { return };
        let word = app.find::<CodeWindow>(id).map(|w| w.editor().word_at_cursor().0).unwrap_or_default();
        let lp = self.lsp_pos(app, srv, id, pos);
        let Some(rid) = self.client_mut(srv).map(|c| c.references(&path, lp)) else { return };
        self.lsp_state.wait.insert((srv, rid), Wait::References { word });
    }

    fn show_references(&mut self, app: &mut Program, srv: Srv, word: &str, items: Vec<Location>) {
        if items.is_empty() {
            return self.say(app, &format!("Ссылок на «{}» не найдено.", word));
        }
        self.msgs(app).clear();
        self.show_locations(app, srv, &format!("— Ссылки на «{}»: {}", word, items.len()), &items);
        if let Some(id) = self.messages {
            if let Some(m) = app.find_mut::<MessagesWindow>(id) {
                m.focus(1);
            }
            app.focus(id);
        }
    }

    /// Места — строками в окне сообщений: «файл:строка:столбец: текст строки».
    fn show_locations(&mut self, app: &mut Program, srv: Srv, title: &str, items: &[Location]) {
        let enc = self.encoding(srv);
        let base = self.base_dir();
        self.say(app, title);
        let mut sorted = items.to_vec();
        sorted.sort_by(|a, b| (&a.path, a.range.start).cmp(&(&b.path, b.range.start)));
        for loc in sorted {
            let text = self.line_text(app, &loc.path, loc.range.start.line);
            let col = col_from_lsp(&text, loc.range.start.col, enc) + 1;
            let shown = base
                .as_ref()
                .and_then(|b| loc.path.strip_prefix(b).ok())
                .map(|r| r.display().to_string())
                .unwrap_or_else(|| loc.path.display().to_string());
            self.msgs(app).push(MsgEntry {
                text: format!("{}:{}:{}: {}", shown, loc.range.start.line + 1, col, text.trim()),
                kind: MsgKind::Note,
                loc: Some((loc.path.display().to_string(), loc.range.start.line + 1, col)),
            });
        }
    }
}

/// Применить правки к тексту редактора. Диапазоны — в единицах сервера
/// (`enc`) относительно исходного текста, поэтому правки идут с конца к
/// началу: более ранние позиции от поздних правок не сдвигаются.
pub fn apply_text_edits(e: &mut tvision::Editor, edits: &[TextEdit], enc: Encoding) {
    let mut list: Vec<&TextEdit> = edits.iter().collect();
    list.sort_by_key(|t| std::cmp::Reverse(t.range.start));
    let to_pos = |e: &tvision::Editor, p: LPos| {
        let n = e.line_count();
        if p.line >= n {
            let last = n.saturating_sub(1);
            return Pos::new(last, e.line(last).chars().count());
        }
        Pos::new(p.line, col_from_lsp(&e.line(p.line), p.col, enc))
    };
    for t in list {
        let a = to_pos(e, t.range.start);
        let b = to_pos(e, t.range.end).max(a);
        e.set_select(a, b);
        e.insert_text(&t.text, false);
    }
    e.track_cursor(false);
}
