//! Редакторы модели приложения (меню, строка состояния, команды).

use tvision::{cm, Program};

use super::*;
use crate::appdesign::{LeKind, ListEditor};
use crate::codeedit::CodeWindow;
use crate::dialogs;

impl Ide {
    pub(super) fn edit_model(&mut self, app: &mut Program, kind: LeKind) {
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

    pub(super) fn extract_forms(&mut self, app: &mut Program) {
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

    pub(super) fn app_options(&mut self, app: &mut Program) {
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
}