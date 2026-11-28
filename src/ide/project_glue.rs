//! Открытие, сохранение, закрытие проекта и генерация кода.

use std::path::{Path, PathBuf};

use tvision::{cm, fd, mf, FileDialog, ListBox, Program};

use super::*;
use crate::codeedit::CodeWindow;
use crate::codegen;
use crate::designer::{DesignWindow, DocRef};
use crate::dialogs::{self, message_box};
use crate::model::{Form, Project, PROJECT_EXT};
use crate::toolwins::{MsgEntry, MsgKind};

impl Ide {
    pub fn base_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.dir.clone())
    }

    pub(super) fn dirty_windows(app: &Program) -> bool {
        let code =
            Self::windows::<CodeWindow>(app).into_iter().any(|id| app.find::<CodeWindow>(id).map(|w| w.editor().fe.editor.modified).unwrap_or(false));
        let forms =
            Self::windows::<DesignWindow>(app).into_iter().any(|id| app.find::<DesignWindow>(id).map(|w| w.doc.borrow().modified).unwrap_or(false));
        code || forms
    }

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
        self.git.reset_panel(app);
        self.marks.clear();
        self.tree_dirty = true;
        true
    }

    pub fn open_project(&mut self, app: &mut Program, file: &Path) {
        if self.project.is_some() && !self.close_project(app) {
            return;
        }
        match Project::load(file) {
            Ok(p) => {
                self.dismiss_start(app);
                let f = p.file().display().to_string();
                let files: Vec<PathBuf> = p.open_files.iter().map(|r| p.dir.join(r)).collect();
                let name = p.name.clone();
                self.git.reset_panel(app);
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

    pub(super) fn new_project(&mut self, app: &mut Program) {
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
                if let Err(e) = self.load_forms(app, &p).and_then(|forms| codegen::generate_all(&p, &forms)) {
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

    pub(super) fn import(&mut self, app: &mut Program) {
        let d = FileDialog::new("*.rs", "Импорт программы tvision-rs — главный файл", "~И~мя", fd::OPEN_BUTTON, 112);
        let (cmd, d) = dialogs::exec(app, d);
        if cmd == cm::CANCEL {
            return;
        }
        let main = d.file_name();
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

    pub(super) fn choose_project_file(&mut self, app: &mut Program) {
        let d = FileDialog::new(&format!("*.{}", PROJECT_EXT), "Открыть проект", "~И~мя", fd::OPEN_BUTTON, 110);
        let (cmd, d) = dialogs::exec(app, d);
        if cmd != cm::CANCEL {
            let f = d.file_name();
            self.open_project(app, &f);
        }
    }

    pub(super) fn recent(&mut self, app: &mut Program) {
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

    fn load_forms(&self, app: &Program, p: &Project) -> Result<Vec<Form>, String> {
        let open: Vec<DocRef> =
            Self::windows::<DesignWindow>(app).into_iter().filter_map(|id| app.find::<DesignWindow>(id).map(|w| w.doc.clone())).collect();
        p.forms
            .iter()
            .map(|name| {
                let path = p.form_path(name);
                if let Some(d) = open.iter().find(|d| same_file(&d.borrow().path, &path)) {
                    return Ok(d.borrow().form.clone());
                }
                Form::load(&path)
            })
            .collect()
    }

    pub fn generate(&mut self, app: &mut Program, quiet: bool) -> bool {
        let Some(p) = self.project.clone() else { return false };
        if !p.has_forms() {
            if !quiet {
                self.say(app, "Проект на Rust: генерировать нечего — весь код ваш.");
            }
            return true;
        }
        let forms = match self.load_forms(app, &p) {
            Ok(forms) => forms,
            Err(e) => {
                Self::error(app, &e);
                return false;
            }
        };
        let warnings = codegen::check_project(&p, &forms);
        for w in &warnings {
            self.msgs(app).push(MsgEntry { text: format!("модель: {}", w), kind: MsgKind::Warning, loc: None });
        }
        match codegen::generate_all(&p, &forms) {
            Ok(changed) => {
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
}