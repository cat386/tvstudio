//! Диспетчер команд меню и горячих клавиш.

use std::path::{Path, PathBuf};

use tvision::{cm, fd, Event, FileDialog, Info, Message, Program};

use super::*;
use crate::appdesign::LeKind;
use crate::codeedit::CodeWindow;
use crate::designer::{Align, DesignWindow, Reorder};
use crate::dialogs::{self, input_box};
use crate::inspector::InspectorWindow;
use crate::model::{Kind, FORM_EXT, PROJECT_EXT};
use crate::runner::After;
use crate::toolwins::{MessagesWindow, MsgEntry, MsgKind, ProblemsWindow, ProjectWindow};

impl Ide {
  /// Обработать команду. `true` — команда наша.
  pub(super) fn command(&mut self, app: &mut Program, c: u16, info: &Info) -> bool {
    if self.git_command(app, c) {
      return true;
    }
    match c {
      crate::start_page::OPEN_RECENT => {
        if let Info::Str(path) = info {
          self.open_project(app, Path::new(path));
        }
      }
      CM_NEW_PROJECT => self.new_project(app),
      CM_OPEN_PROJECT => self.choose_project_file(app),
      CM_RECENT => self.recent(app),
      CM_IMPORT => self.import(app),
      CM_CLOSE_PROJECT => {
        if self.close_project(app) {
          self.show_start(app);
        }
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
      CM_DEBUG => self.debug_build(app),
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
          let in_props = app.find::<InspectorWindow>(id).map(|w| !w.components_focused()).unwrap_or(false);
          if c == CM_SHOW_INSPECTOR && Self::current_window(app) == Some(id) && in_props {
            if let Some(d) = Self::top_designer(app) {
              app.focus(d);
              return true;
            }
          }
          app.focus(id);
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
}

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