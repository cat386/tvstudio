//! О «программе», справка и параметры среды.

use tvision::{cm, mf, HelpWindow, Program, Rect};

use super::*;
use crate::codeedit::CodeWindow;
use crate::dialogs::{self, message_box_rect};

impl Ide {
    pub(super) fn about(app: &mut Program) {
        message_box_rect(
            app,
            Rect::new(0, 0, 58, 13),
            "\x03TV Studio 0.1\n\n\x03Среда визуальной разработки\n\x03текстовых приложений на tvision-rs\n\n\x03Формы • модель приложения • код на Rust\n\x03редактор • сборка cargo",
            mf::INFORMATION | mf::OK_BUTTON,
            true,
        );
    }

    pub(super) fn show_help(&mut self, app: &mut Program, ctx: u16) {
        let ctx = if self.help.topic_for_context(ctx).is_some() { ctx } else { 0 };
        let mut w = HelpWindow::new(self.help.clone(), ctx);
        let s = app.desktop().group.vs.size;
        let r = Rect::new(0, 0, (s.x - 4).min(76), (s.y - 2).min(22));
        w.win.group.change_bounds(r);
        dialogs::insert(app, w);
    }

    pub(super) fn options(&mut self, app: &mut Program) {
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
}