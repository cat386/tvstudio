//! Раскладка панелей, стартовая страница, сообщения и редактор кода.

use std::path::{Path, PathBuf};

use tvision::{mf, Program, Rect, View, ViewId};

use super::*;
use crate::codeedit::CodeWindow;
use crate::designer::{DesignWindow, DocRef};
use crate::dialogs::{self, message_box};
use crate::inspector::InspectorWindow;
use crate::model::FORM_EXT;
use crate::toolwins::{parse_cargo_line, MessagesWindow, MsgEntry, MsgKind, ProjectWindow};

impl Ide {
    pub fn form_tools(&self) -> bool {
        self.project.as_ref().map(|p| p.has_forms()).unwrap_or(false)
    }

    pub(super) fn dismiss_start(&mut self, app: &mut Program) {
        if let Some(id) = self.start_page.take() {
            app.desktop_mut().group.remove(id);
        }
    }

    pub(super) fn show_start(&mut self, app: &mut Program) {
        self.dismiss_start(app);
        for id in [self.project_win.take(), self.inspector.take(), self.messages.take()].into_iter().flatten() {
            app.desktop_mut().group.remove(id);
        }
        self.start_page = Some(dialogs::insert(app, crate::start_page::StartPage::new(&self.cfg.recent)));
    }

    pub(super) fn layout(&self, app: &Program) -> [Rect; 4] {
        let s = app.desktop().group.vs.size;
        let lw = (s.x / 5).clamp(18, 26);
        let rw = (s.x / 4).clamp(26, 36);
        let bh = (s.y / 4).clamp(5, 9) - 2;
        let h = s.y - bh;
        let messages = Rect::new(0, h, s.x, s.y);
        if self.form_tools() {
            let ph = (h / 3).max(6).min((h - 10).max(3));
            [Rect::new(s.x - rw, 0, s.x, ph), Rect::new(s.x - rw, ph, s.x, h), messages, Rect::new(0, 0, s.x - rw, h)]
        } else {
            [Rect::new(s.x - lw, 0, s.x, h), Rect::new(s.x - rw, 0, s.x, h), messages, Rect::new(0, 0, s.x - lw, h)]
        }
    }

    pub(super) fn alive(app: &Program, id: Option<ViewId>) -> bool {
        id.map(|i| app.desktop().group.index_of(i).is_some()).unwrap_or(false)
    }

    pub fn windows<T: View>(app: &Program) -> Vec<ViewId> {
        app.desktop().group.children.iter().filter(|c| c.as_any().is::<T>()).map(|c| c.vs().id).collect()
    }

    pub fn current_window(app: &Program) -> Option<ViewId> {
        app.desktop().group.current_view().map(|v| v.vs().id)
    }

    pub fn top_designer(app: &Program) -> Option<ViewId> {
        Self::windows::<DesignWindow>(app).last().copied()
    }

    pub fn top_doc(app: &Program) -> Option<DocRef> {
        Self::top_designer(app).and_then(|id| app.find::<DesignWindow>(id)).map(|w| w.doc.clone())
    }

    pub fn ensure_tools(&mut self, app: &mut Program) {
        let l = self.layout(app);
        if !Self::alive(app, self.project_win) {
            let mut w = ProjectWindow::new(l[0]);
            w.set_project(self.project.as_ref());
            self.project_win = Some(dialogs::insert(app, w));
        }
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

    pub(super) fn apply_layout(&mut self, app: &mut Program) {
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

    pub(super) fn msgs<'a>(&mut self, app: &'a mut Program) -> &'a mut MessagesWindow {
        if !Self::alive(app, self.messages) {
            let current = Self::current_window(app);
            let l = self.layout(app);
            let id = dialogs::insert(app, MessagesWindow::new(l[2]));
            self.messages = Some(id);
            if let Some(current) = current.filter(|id| Some(*id) != self.start_page) {
                app.focus(current);
            }
        }
        app.find_mut::<MessagesWindow>(self.messages.unwrap()).unwrap()
    }

    pub fn say(&mut self, app: &mut Program, text: &str) {
        self.msgs(app).info(text);
    }

    pub(super) fn error(app: &mut Program, text: &str) {
        message_box(app, &format!("\x03{}", text.replace('\n', "\n\x03")), mf::ERROR | mf::OK_BUTTON);
    }

    pub(super) fn info_box(app: &mut Program, text: &str) {
        message_box(app, text, mf::INFORMATION | mf::OK_BUTTON);
    }

    pub(super) fn editor_cfg(&self, e: &mut crate::codeedit::CodeEditor) {
        e.hl_line = self.cfg.hl_line;
        e.hl_col = self.cfg.hl_col;
        e.auto_close = self.cfg.auto_close;
        e.auto_complete = self.cfg.auto_complete;
        e.fmt_cmd = if self.cfg.fmt_on_save { Some(self.cfg.rustfmt.clone()) } else { None };
    }

    pub(super) fn collect_fmt_errors(&mut self, app: &mut Program) {
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

    pub fn open_file(&mut self, app: &mut Program, path: &Path, at: Option<(usize, usize)>) -> Option<ViewId> {
        if self.start_page.is_some() {
            self.dismiss_start(app);
            self.ensure_tools(app);
        }
        let abs = if path.as_os_str().is_empty() { PathBuf::new() } else { tvision::widgets::editor::absolute(path) };
        let existing = Self::windows::<CodeWindow>(app)
            .into_iter()
            .find(|&id| !abs.as_os_str().is_empty() && app.find::<CodeWindow>(id).map(|w| super::same_file(&w.path(), &abs)).unwrap_or(false));
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

    pub(super) fn goto_message(&mut self, app: &mut Program, index: usize) {
        let Some(id) = self.messages else { return };
        let Some(e) = app.find::<MessagesWindow>(id).and_then(|m| m.entries.get(index).cloned()) else { return };
        let Some((f, l, c)) = e.loc else { return };
        let dir = self.base_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let path = super::resolve(&dir, &f);
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

    pub(super) fn next_error(&mut self, app: &mut Program, forward: bool) {
        let Some(id) = self.messages else { return };
        let Some(i) = app.find::<MessagesWindow>(id).and_then(|m| m.next_location(forward)) else { return };
        if let Some(m) = app.find_mut::<MessagesWindow>(id) {
            m.focus(i);
        }
        self.goto_message(app, i);
    }
}