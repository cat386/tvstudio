//! Сборка cargo, вывод в сообщения, пометки в редакторе и окно проблем.

use std::collections::HashMap;
use std::path::PathBuf;

use super::*;
use crate::codeedit::CodeWindow;
use crate::dialogs;
use crate::runner::{After, Job, JobMsg};
use crate::toolwins::{parse_cargo_line, MessagesWindow, MsgKind, Problem, ProblemsWindow};

impl Ide {
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

    pub(super) fn poll_job(&mut self, app: &mut Program) {
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
        if ok && !self.headless {
            if after == After::Run {
                if let Some(dir) = self.base_dir() {
                    let cargo = self.cfg.cargo.clone();
                    let opts = self.project.as_ref().map(|p| p.build.clone()).unwrap_or_default();
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
            } else if after == After::Debug {
                if let Some(dir) = self.base_dir() {
                    let Some(dbg) = crate::debug::resolve(&self.cfg.debugger) else {
                        let msg = if self.cfg.debugger.trim().is_empty() {
                            "Не найден отладчик (rust-lldb, rust-gdb, lldb или gdb в PATH).\nУстановите инструменты Rust или укажите путь в Параметрах среды."
                        } else {
                            "Указанный отладчик недоступен — проверьте путь в Параметрах среды."
                        };
                        self.say(app, &format!("— {msg}"));
                        Self::error(app, msg);
                        return;
                    };
                    let mut opts = self.project.as_ref().map(|p| p.build.clone()).unwrap_or_default();
                    if opts.package.trim().is_empty() {
                        let file = Self::windows::<CodeWindow>(app).last().and_then(|&id| app.find::<CodeWindow>(id)).map(|w| w.path());
                        if let Some(p) = crate::model::cargo_layout(&dir).run_package(file.as_deref()) {
                            opts.package = p;
                        }
                    }
                    let prog = opts.program_args();
                    let file = Self::windows::<CodeWindow>(app).last().and_then(|&id| app.find::<CodeWindow>(id)).map(|w| w.path());
                    let cargo = self.cfg.cargo.clone();
                    let dbg_name = dbg.program.clone();
                    self.say(app, &format!("— Отладка через {}", dbg_name));
                    let r = app.suspended(|| crate::debug::debug_foreground(&dir, &dbg, &cargo, &opts, &prog, file.as_deref()));
                    match r {
                        Ok(code) => self.say(app, &format!("— Отладчик завершился, код {}", code.map(|c| c.to_string()).unwrap_or("?".into()))),
                        Err(e) => {
                            self.say(app, &format!("— {e}"));
                            Self::error(app, &e);
                        }
                    }
                }
            }
        }
    }

    pub fn debug_build(&mut self, app: &mut Program) {
        self.cargo(app, &["build", "--message-format=short"], "Сборка для отладки", After::Debug);
    }

    fn update_marks(&mut self, app: &mut Program) {
        let Some(dir) = self.base_dir() else { return };
        let mut marks: HashMap<PathBuf, Vec<(usize, MsgKind)>> = HashMap::new();
        if let Some(m) = self.messages.and_then(|id| app.find::<MessagesWindow>(id)) {
            for e in &m.entries {
                if let (Some((f, l, _)), MsgKind::Error | MsgKind::Warning) = (&e.loc, e.kind) {
                    let p = super::resolve(&dir, f);
                    let v = marks.entry(p).or_default();
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
                let m = marks.iter().find(|(k, _)| super::same_file(k, &p)).map(|(_, v)| v.clone()).unwrap_or_default();
                w.set_marks(m);
            }
        }
        self.marks = marks;
        self.lsp_state.diags_dirty = true;
    }

    pub(super) fn show_problems(&mut self, app: &mut Program) {
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

    pub(super) fn update_problems(&mut self, app: &mut Program) {
        if !self.problems_dirty || !Self::alive(app, self.problems) {
            return;
        }
        self.problems_dirty = false;
        let list = self.collect_problems(app);
        if let Some(w) = self.problems.and_then(|id| app.find_mut::<ProblemsWindow>(id)) {
            w.set_problems(list);
        }
    }

    pub(super) fn build_problems(&self) -> Vec<Problem> {
        let dir = self.base_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let mut v: Vec<Problem> = Vec::new();
        for e in &self.build_diags {
            let Some((f, l, c)) = &e.loc else { continue };
            let path = super::resolve(&dir, f);
            if self.lsp_state.diags.keys().any(|k| super::same_file(k, &path)) {
                continue;
            }
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

    pub(super) fn build_options(&mut self, app: &mut Program) {
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
}