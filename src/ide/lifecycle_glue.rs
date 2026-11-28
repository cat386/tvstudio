//! Запуск, завершение, idle-синхронизация и доступность команд.

use std::path::{Path, PathBuf};

use tvision::Program;

use super::*;
use crate::codeedit::CodeWindow;
use crate::designer::{Align, DesignWindow};
use crate::inspector::InspectorWindow;
use crate::model::{Kind, PROJECT_EXT, Project};
use crate::toolwins::{MenuClock, MessagesWindow, ProjectWindow};

impl Ide {
    /// Начало работы: окна-инструменты, проект или файлы из командной строки.
    pub fn startup(&mut self, app: &mut Program, args: Vec<String>) {
        app.palette = self.cfg.palette();
        MenuClock::install(app);
        let diag_hint = self.lsp_state.hint.clone();
        let hint: Box<dyn Fn(u16) -> String> = Box::new(move |ctx| match ctx {
            HC_DESIGNER => String::new(),
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
                if let Some(f) = super::find_project_file(&p) {
                    self.open_project(app, &f);
                    opened = true;
                } else if p.join("Cargo.toml").is_file() {
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
        if !opened && args.is_empty() {
            self.show_start(app);
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

    pub(super) fn sync_windows(&mut self, app: &mut Program) {
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
                self.say(app, &format!("Ошибка: {}", e));
                self.last_error = Some(e);
            } else {
                Self::error(app, &e);
            }
        }
    }

    pub(super) fn update_commands(&self, app: &mut Program) {
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
        let has_forms = self.project.as_ref().map(|p| p.has_forms()).unwrap_or(false);
        for c in [CM_CHECK, CM_BUILD, CM_RUN, CM_DEBUG, CM_TEST, CM_CLEAN, CM_BUILD_OPTS] {
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
        for c in [CM_SHOW_PALETTE, CM_SHOW_INSPECTOR] {
            set(app, c, has_forms);
        }
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
        set(app, CM_DS_DUP, has_d && !code);
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

    pub(super) fn idle(&mut self, app: &mut Program) {
        self.poll_job(app);
        self.poll_git(app);
        self.collect_fmt_errors(app);
        self.lsp_idle(app);
        self.sync_windows(app);
        self.update_problems(app);
        self.update_commands(app);
    }
}