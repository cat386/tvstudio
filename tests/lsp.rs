//! Сервер LSP в редакторе кода: клиент разговаривает с учебным сервером
//! `tvs-fake-lsp` (тот же протокол, что у rust-analyzer), среда работает
//! на «невидимом» терминале. Проверяются диагностика в реальном времени,
//! дополнение (по Ctrl+Space и само после «.»), переход к определению и
//! назад, поиск ссылок, список диагностик, перезапуск сервера.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tvision::platform::HeadlessBackend;
use tvision::*;
use tvstudio::cmds::*;
use tvstudio::codeedit::{CodeEditor, CodeWindow, Gutter};
use tvstudio::dialogs::Config;
use tvstudio::ide::{menu_bar, status_line, Ide};
use tvstudio::model::{Dependency, Project};
use tvstudio::toolwins::{MessagesWindow, MsgKind};

const SRC: &str = "fn helper() -> i32 {
    1
}

fn main() {
    let mut s = String::new();
    s
    let x = helper() + helper();
    let y = ОШИБКА;
    warn_me();
}
";

struct T {
    app: Program,
    ide: Ide,
    win: ViewId,
    file: PathBuf,
}

impl T {
    fn new(name: &str) -> T {
        let base = std::env::temp_dir().join(format!("tvs_lsp_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let p = Project::create(&base, "demo", &Dependency::Git("https://example.invalid/tvision-rs".into())).unwrap();
        let file = p.dir.join("src").join("demo.rs");
        std::fs::write(&file, SRC).unwrap();
        let mut b = HeadlessBackend::new(120, 40);
        b.esc_budget = 0;
        let mut app = Program::new(Box::new(b), Some(menu_bar()), Some(status_line()));
        let cfg = Config { lsp_command: env!("CARGO_BIN_EXE_tvs-fake-lsp").into(), ..Config::default() };
        let mut ide = Ide::new(cfg, true);
        ide.startup(&mut app, vec![p.file().display().to_string()]);
        app.pump(&mut ide);
        let win = ide.open_file(&mut app, &file, None).unwrap();
        let mut t = T { app, ide, win, file };
        assert!(t.wait(|t| t.ide.lsp.as_ref().map(|c| c.ready).unwrap_or(false)), "сервер не ответил на initialize");
        t
    }

    fn pump(&mut self) {
        self.app.pump(&mut self.ide);
        self.app.pump(&mut self.ide);
    }

    /// Крутить цикл, пока не выполнится условие (до 5 с).
    fn wait(&mut self, cond: impl Fn(&T) -> bool) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(5) {
            self.pump();
            if cond(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn key(&mut self, k: KeyEvent) {
        self.app.put_event(Event::KeyDown(k));
        self.pump();
    }

    fn typed(&mut self, s: &str) {
        for c in s.chars() {
            self.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
        }
        self.pump();
    }

    fn cmd(&mut self, c: u16) {
        self.app.put_event(Event::command_event(c));
        self.pump();
    }

    fn w(&self) -> &CodeWindow {
        self.app.find::<CodeWindow>(self.win).unwrap()
    }

    fn ed(&self) -> &CodeEditor {
        self.w().editor()
    }

    fn goto(&mut self, line: usize, col: usize) {
        self.app.focus(self.win);
        self.app.find_mut::<CodeWindow>(self.win).unwrap().goto(line, col);
        self.pump();
    }

    fn msgs(&self) -> &MessagesWindow {
        self.app.find::<MessagesWindow>(self.ide.messages.unwrap()).unwrap()
    }

    fn located(&self) -> Vec<(String, usize, usize)> {
        self.msgs().entries.iter().filter_map(|e| e.loc.clone()).collect()
    }
}

impl Drop for T {
    fn drop(&mut self) {
        self.ide.shutdown(&mut self.app);
    }
}

#[test]
fn diagnostics_follow_typing() {
    let mut t = T::new("diag");
    // Сразу после открытия: ошибка в строке 9, предупреждение в строке 10.
    assert!(t.wait(|t| t.ed().diags.len() == 2), "нет диагностики: {:?}", t.ed().diags);
    let d = t.ed().diags.clone();
    assert_eq!((d[0].line, d[0].c0, d[0].c1, d[0].error), (8, 12, 18, true));
    assert_eq!((d[1].line, d[1].error), (9, false));
    let gid = t.w().gutter_id;
    let marks = t.app.find::<CodeWindow>(t.win).unwrap().win.group.find::<Gutter>(gid).unwrap().marks.clone();
    assert!(marks.contains(&(8, MsgKind::Error)) && marks.contains(&(9, MsgKind::Warning)), "{:?}", marks);
    // Сообщение строки под курсором — в строке состояния.
    t.goto(9, 1);
    assert_eq!(*t.ide.lsp_state.hint.borrow(), "■ найдена ОШИБКА");
    assert!(t.app.screen_text().contains("найдена ОШИБКА"));
    // Дописали ещё одну ошибку — через паузу сервер видит новый текст.
    t.goto(2, 6);
    t.typed(" + ОШИБКА");
    assert_eq!(t.ed().diags.len(), 2, "текст уходит не сразу, а после паузы");
    assert!(t.wait(|t| t.ed().diags.len() == 3));
    assert!(t.ed().diags.iter().any(|d| d.line == 1 && d.error));
    // Список всех диагностик — в окне сообщений, Enter ведёт к месту.
    t.cmd(CM_DIAGNOSTICS);
    let locs = t.located();
    assert_eq!(locs.len(), 3, "{:?}", t.msgs().entries);
    assert_eq!((locs[0].1, locs[0].2), (2, 9));
}

#[test]
fn completion_popup() {
    let mut t = T::new("compl");
    // Само после «.»: строка 7 «    s» → «    s.».
    t.goto(7, 6);
    t.typed(".");
    assert!(t.wait(|t| t.ed().completion.is_some()), "список не открылся");
    let c = t.ed().completion.clone().unwrap();
    assert_eq!(c.shown.len(), 3);
    assert_eq!(c.start, Pos::new(6, 6));
    // Список рисуется под курсором.
    assert!(t.app.screen_text().contains("push_str(…)"), "{}", t.app.screen_text());
    // Набор фильтрует: «ca» — только capacity.
    t.typed("ca");
    let c = t.ed().completion.clone().unwrap();
    assert_eq!(c.shown.len(), 1);
    assert_eq!(c.items[c.shown[0]].label, "capacity");
    // Esc — закрыть, набранное остаётся.
    t.key(KeyEvent::new(Key::Esc, Mods::NONE));
    assert!(t.ed().completion.is_none());
    assert_eq!(t.ed().fe.editor.line(6), "    s.ca");
    // Ctrl+Space: набранное «ca» сразу фильтрует; Tab заменяет его выбранным.
    t.key(KeyEvent::new(Key::Char(' '), Mods::CTRL));
    assert!(t.wait(|t| t.ed().completion.is_some()));
    assert_eq!(t.ed().completion.as_ref().unwrap().start, Pos::new(6, 6));
    assert_eq!(t.ed().completion.as_ref().unwrap().shown.len(), 1);
    t.key(KeyEvent::new(Key::Tab, Mods::NONE));
    assert_eq!(t.ed().fe.editor.line(6), "    s.capacity");
    // Стереть слово и снова: весь список, выбор стрелками, сниппет разворачивается.
    for _ in 0..8 {
        t.key(KeyEvent::new(Key::Backspace, Mods::NONE));
    }
    assert_eq!(t.ed().fe.editor.line(6), "    s.");
    t.key(KeyEvent::new(Key::Char(' '), Mods::CTRL));
    assert!(t.wait(|t| t.ed().completion.is_some()));
    t.key(KeyEvent::new(Key::Up, Mods::NONE)); // по кругу — на последний
    assert_eq!(t.ed().selected_completion().unwrap().label, "capacity");
    t.key(KeyEvent::new(Key::Down, Mods::NONE)); // снова первый
    t.key(KeyEvent::new(Key::Enter, Mods::NONE));
    assert!(t.ed().completion.is_none());
    assert_eq!(t.ed().fe.editor.line(6), "    s.push_str(s)");
    // Не идентификатор (скобка) закрывает список.
    t.typed(".");
    assert!(t.wait(|t| t.ed().completion.is_some()));
    t.typed("(");
    assert!(t.ed().completion.is_none());
    assert_eq!(t.ed().fe.editor.line(6), "    s.push_str(s).(");
}

#[test]
fn definition_and_references() {
    let mut t = T::new("nav");
    // Строка 8: «    let x = helper() + helper();» — курсор на втором helper.
    t.goto(8, 26);
    t.key(KeyEvent::new(Key::F(12), Mods::NONE));
    assert!(t.wait(|t| t.ed().fe.editor.cursor() == Pos::new(0, 3)), "курсор: {:?}", t.ed().fe.editor.cursor());
    // Alt+← — назад.
    t.key(KeyEvent::new(Key::Left, Mods::ALT));
    assert_eq!(t.ed().fe.editor.cursor(), Pos::new(7, 25));
    // Shift+F12 — ссылки: определение и два вызова.
    t.key(KeyEvent::new(Key::F(12), Mods::SHIFT));
    assert!(t.wait(|t| t.located().len() == 3), "{:?}", t.msgs().entries);
    let locs = t.located();
    assert_eq!(locs.iter().map(|l| (l.1, l.2)).collect::<Vec<_>>(), vec![(1, 4), (8, 13), (8, 24)]);
    assert!(t.msgs().entries[0].text.contains("«helper»: 3"));
    assert_eq!(std::path::Path::new(&locs[0].0), t.file);
    // Enter в сообщениях — к месту.
    assert_eq!(t.app.desktop().group.current_view().map(|v| v.vs().id), t.ide.messages);
    t.app.put_event(Event::Command(Message::with(CM_MSG_GOTO, Info::Int(2))));
    t.pump();
    assert_eq!(t.ed().fe.editor.cursor(), Pos::new(7, 12));
    // Определения нет — сообщение, курсор на месте.
    t.goto(6, 9);
    t.key(KeyEvent::new(Key::F(12), Mods::NONE));
    assert!(t.wait(|t| t.msgs().entries.iter().any(|e| e.text.contains("Определение не найдено"))));
}

#[test]
fn restart_and_close() {
    let mut t = T::new("restart");
    assert!(t.wait(|t| !t.ed().diags.is_empty()));
    t.cmd(CM_LSP_RESTART);
    assert!(t.ide.lsp.is_some());
    assert!(t.wait(|t| t.ide.lsp.as_ref().unwrap().ready && t.ed().diags.len() == 2), "после перезапуска документ открыт заново");
    // Закрыли окно — документ закрыт на сервере.
    t.app.put_event(Event::Command(Message::with(cm::CLOSE, Info::Id(t.win))));
    t.pump();
    assert!(t.wait(|t| t.ide.lsp.as_ref().unwrap().open_docs().is_empty()));
    // Закрыли проект — сервер остановлен.
    t.cmd(CM_CLOSE_PROJECT);
    assert!(t.ide.lsp.is_none());
}

#[test]
fn missing_server_is_reported() {
    let base = std::env::temp_dir().join(format!("tvs_lsp_missing_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let p = Project::create(&base, "demo", &Dependency::Git("https://example.invalid/x".into())).unwrap();
    let mut b = HeadlessBackend::new(120, 40);
    b.esc_budget = 0;
    let mut app = Program::new(Box::new(b), Some(menu_bar()), Some(status_line()));
    let cfg = Config { lsp_command: "rust-analyzer-нет-такого".into(), ..Config::default() };
    let mut ide = Ide::new(cfg, true);
    ide.startup(&mut app, vec![p.file().display().to_string()]);
    app.pump(&mut ide);
    assert!(ide.lsp.is_none());
    let m = app.find::<MessagesWindow>(ide.messages.unwrap()).unwrap();
    assert!(m.entries.iter().any(|e| e.text.contains("Сервер LSP недоступен")), "{:?}", m.entries);
    assert!(m.entries.iter().any(|e| e.text.contains("rustup component add rust-analyzer")));
}
