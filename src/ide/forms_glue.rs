//! Дизайнер форм: открытие, создание, свойства, тест, переход к коду.

use tvision::{bf, cm, mf, Button, Dialog, ListBox, Program, Rect, StaticText};

use super::*;
use crate::codeedit::CodeWindow;
use crate::builder::{build_dialog, describe_data};
use crate::designer::{DesignWindow, FormDoc};
use crate::dialogs::{self, message_box_rect};
use crate::model::{Form, is_ident};

const BUILTIN_NAME: &[(u16, &str)] = &[(cm::OK, "cm::OK"), (cm::CANCEL, "cm::CANCEL"), (cm::YES, "cm::YES"), (cm::NO, "cm::NO")];

impl Ide {
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
                let s = app.desktop().group.vs.size;
                f.x = f.x.clamp(0, (s.x - f.w).max(0));
                f.y = f.y.clamp(0, (s.y - f.h).max(0));
                let doc = FormDoc::new(f, path);
                dialogs::insert(app, DesignWindow::new(doc));
            }
            Err(e) => Self::error(app, &e),
        }
    }

    pub(super) fn new_form(&mut self, app: &mut Program) {
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

    pub(super) fn choose_form(&mut self, app: &mut Program) {
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

    pub(super) fn remove_form(&mut self, app: &mut Program) {
        let Some(doc) = Self::top_doc(app) else { return };
        let name = doc.borrow().form.name.clone();
        let path = doc.borrow().path.clone();
        let mut confirmation = Dialog::centered(66, 11, "Удалить форму и файл");
        confirmation
            .add(StaticText::new(Rect::new(2, 2, 64, 6), &format!("Форма: {}\nФайл: {}\nФайл формы будет удалён с диска.", name, path.display())));
        let cancel = confirmation.add(Button::new(Rect::new(34, 8, 48, 10), "~О~тмена", cm::CANCEL, bf::DEFAULT));
        confirmation.add(Button::new(Rect::new(18, 8, 32, 10), "~У~далить", cm::YES, bf::NORMAL));
        confirmation.select(cancel);
        let r = dialogs::exec(app, confirmation).0;
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

    pub(super) fn sync_form_names(&mut self, app: &mut Program) {
        let Some(p) = &mut self.project else { return };
        for id in Self::windows::<DesignWindow>(app) {
            let Some(w) = app.find::<DesignWindow>(id) else { continue };
            let mut d = w.doc.borrow_mut();
            let stem = d.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let new = d.form.name.clone();
            if stem == new || !is_ident(&new) {
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
                if c.action == crate::model::Action::Form(stem.clone()) {
                    c.action = crate::model::Action::Form(new.clone());
                }
            }
            if p.app.startup_form == stem {
                p.app.startup_form = new.clone();
            }
            let _ = p.save();
            self.tree_dirty = true;
        }
    }

    pub(super) fn test_form(&mut self, app: &mut Program) {
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

    pub(super) fn form_code(&mut self, app: &mut Program) {
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

    pub(super) fn form_props(&mut self, app: &mut Program) {
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
}