//! Проверка среды на «невидимом» терминале (`HeadlessBackend`):
//! события подаются в очередь программы, результат проверяется по
//! модели формы, проекту и тексту на экране.

use std::path::PathBuf;

use tvision::platform::HeadlessBackend;
use tvision::*;
use tvstudio::cmds::*;
use tvstudio::codeedit::CodeWindow;
use tvstudio::designer::DesignWindow;
use tvstudio::dialogs::Config;
use tvstudio::ide::{menu_bar, status_line, Ide};
use tvstudio::inspector::InspectorWindow;
use tvstudio::model::{Dependency, Kind, MenuNode, Project, ProjectKind};

struct T {
    app: Program,
    ide: Ide,
    dir: PathBuf,
}

fn tvision_path() -> String {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent().unwrap().join("tvision-rs").canonicalize().unwrap().display().to_string()
}

impl T {
    fn new(name: &str) -> T {
        let base = std::env::temp_dir().join(format!("tvs_ui_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let p = Project::create(&base, "demo", &Dependency::Path(tvision_path())).unwrap();
        let mut b = HeadlessBackend::new(120, 40);
        // Без автоматического Esc при простое: он снимал бы выделение в дизайнере.
        b.esc_budget = 0;
        let mut app = Program::new(Box::new(b), Some(menu_bar()), Some(status_line()));
        // Сервер LSP проверяется отдельно (tests/lsp.rs).
        let cfg = Config { lsp_enabled: false, ..Config::default() };
        let mut ide = Ide::new(cfg, true);
        ide.startup(&mut app, vec![p.file().display().to_string()]);
        app.pump(&mut ide);
        T { app, ide, dir: p.dir }
    }

    fn pump(&mut self) {
        self.app.pump(&mut self.ide);
        // Синхронизация окон — в простое.
        self.app.pump(&mut self.ide);
    }

    fn key(&mut self, k: KeyEvent) {
        self.app.put_event(Event::KeyDown(k));
        self.pump();
    }

    fn keys(&mut self, ks: &[KeyEvent]) {
        for k in ks {
            self.app.put_event(Event::KeyDown(*k));
        }
        self.pump();
    }

    fn typed(&mut self, s: &str) -> Vec<KeyEvent> {
        s.chars().map(|c| KeyEvent::new(Key::Char(c), Mods::NONE)).collect()
    }

    fn cmd(&mut self, c: u16) {
        self.app.put_event(Event::command_event(c));
        self.pump();
    }

    fn screen(&self) -> String {
        self.app.screen_text()
    }

    fn doc_controls(&self) -> Vec<tvstudio::model::Control> {
        let d = Ide::top_doc(&self.app).expect("форма открыта");
        let c = d.borrow().form.controls.clone();
        c
    }

    fn open_about(&mut self) {
        self.ide.open_form(&mut self.app, "about");
        self.pump();
    }
}

impl Drop for T {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
    }
}

fn mouse(x: i32, y: i32) -> MouseEvent {
    MouseEvent { pos: Point::new(x, y), buttons: 1, ..Default::default() }
}

#[test]
fn startup_shows_tools_and_project() {
    let t = T::new("start");
    let s = t.screen();
    for title in ["Проект", "Компоненты", "Инспектор", "Сообщения"] {
        assert!(s.contains(title), "нет окна «{}»:\n{}", title, s);
    }
    assert!(s.contains("Формы") && s.contains("about"), "дерево проекта:\n{}", s);
    assert!(s.contains("Открыт проект demo"), "{}", s);
    assert!(s.contains("Фо") && s.contains("Сборка"), "строка меню");
}

/// Компоненты и свойства — одно окно «Инспектор» у правого края: Enter в
/// компонентах добавляет элемент на форму, свойства тут же показывают его;
/// «Окно → Компоненты» ведёт к списку, F11 — к свойствам.
#[test]
fn inspector_holds_components_on_the_right() {
    let mut t = T::new("insp_tools");
    t.open_about();
    let id = t.ide.inspector.expect("окно инспектора");
    let desk = t.app.desktop().group.vs.size;
    let b = t.app.group.view(id).unwrap().vs().bounds();
    assert_eq!(b.b.x, desk.x, "инспектор у правого края: {:?}", b);
    // Отдельного окна «Компоненты» нет: заголовок встречается в инспекторе.
    let w = t.app.find::<InspectorWindow>(id).unwrap();
    assert_eq!(w.win.title, "Инспектор");
    let s = t.screen();
    assert_eq!(s.matches("Компоненты").count(), 1, "{}", s);
    assert!(!s.contains("═ Компоненты ") && !s.contains("─ Компоненты "), "{}", s);
    // Правый столбец: сверху дерево проекта, под ним инспектор до сообщений.
    let pb = t.app.group.view(t.ide.project_win.unwrap()).unwrap().vs().bounds();
    let mb = t.app.group.view(t.ide.messages.unwrap()).unwrap().vs().bounds();
    assert_eq!((pb.a.x, pb.b.x, pb.a.y), (b.a.x, desk.x, 0), "проект над инспектором: {:?}", pb);
    assert_eq!((b.a.y, b.b.y), (pb.b.y, mb.a.y), "инспектор — от проекта до сообщений: {:?}", b);

    // «Окно → Компоненты»: фокус на списке; ↓↓ Enter — третий вид элемента.
    t.cmd(CM_SHOW_PALETTE);
    assert!(t.app.find::<InspectorWindow>(id).unwrap().components_focused());
    t.key(KeyEvent::new(Key::Down, Mods::NONE));
    t.key(KeyEvent::new(Key::Down, Mods::NONE));
    assert_eq!(t.app.find::<InspectorWindow>(id).unwrap().selected_kind(), Kind::ALL[2]);
    t.key(kb::ENTER);
    let c = t.doc_controls();
    assert_eq!(c.last().unwrap().kind, Kind::ALL[2]);
    assert!(t.screen().contains(&format!("{}: {}", c.last().unwrap().name, Kind::ALL[2].title())), "{}", t.screen());
    // F11 — к свойствам того же окна; ещё раз F11 — обратно в дизайнер.
    t.cmd(CM_SHOW_INSPECTOR);
    assert_eq!(Ide::current_window(&t.app), Some(id));
    assert!(!t.app.find::<InspectorWindow>(id).unwrap().components_focused());
    t.cmd(CM_SHOW_INSPECTOR);
    assert_eq!(Ide::current_window(&t.app), Ide::top_designer(&t.app));
}

#[test]
fn designer_insert_move_resize_undo() {
    let mut t = T::new("design");
    t.open_about();
    let s = t.screen();
    assert!(s.contains("О программе") && s.contains("Создано в TV Studio"), "форма в дизайнере:\n{}", s);
    // Добавить строку ввода через палитру (команда), фокус — в дизайнер.
    t.cmd(CM_INS_BASE + 3);
    let c = t.doc_controls();
    assert_eq!(c.len(), 3);
    assert_eq!(c[2].kind, Kind::InputLine);
    assert!(t.screen().contains("input1: Строка ввода"), "инспектор:\n{}", t.screen());
    let (x0, y0, h0) = (c[2].x, c[2].y, c[2].h);
    // Стрелка вправо — сдвиг, Shift+Down — выше.
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    t.key(KeyEvent::new(Key::Down, Mods::SHIFT));
    let c = t.doc_controls();
    assert_eq!((c[2].x, c[2].h), (x0 + 2, h0 + 1));
    // Отмена: два сдвига стрелками — одна запись, размер — вторая.
    t.key(kb::ctrl('Z'));
    t.key(kb::ctrl('Z'));
    let c = t.doc_controls();
    assert_eq!((c[2].x, c[2].y, c[2].h), (x0, y0, h0));
    t.key(kb::ctrl('Y'));
    assert_eq!(t.doc_controls()[2].x, x0 + 2);
    // Del — удалить.
    t.key(KeyEvent::new(Key::Del, Mods::NONE));
    assert_eq!(t.doc_controls().len(), 2);
}

#[test]
fn designer_mouse_drag_and_band() {
    let mut t = T::new("mouse");
    t.open_about();
    let doc = Ide::top_doc(&t.app).unwrap();
    let (fx, fy) = {
        let d = doc.borrow();
        (d.form.x, d.form.y)
    };
    let ok = t.doc_controls()[1].clone();
    // Экран: рабочий стол начинается со строки 1.
    let (sx, sy) = (fx + ok.x + 1, fy + 1 + ok.y);
    t.app.put_event(Event::MouseDown(mouse(sx, sy)));
    t.app.put_event(Event::MouseMove(mouse(sx + 3, sy - 1)));
    t.app.put_event(Event::MouseUp(MouseEvent { buttons: 0, ..mouse(sx + 3, sy - 1) }));
    t.pump();
    let c = t.doc_controls();
    assert_eq!((c[1].x, c[1].y), (ok.x + 3, ok.y - 1), "перетаскивание");
    assert_eq!(doc.borrow().sel, vec![1]);
    // Рамка по пустому месту, захватывающая оба элемента.
    let (w, h) = (doc.borrow().form.w, doc.borrow().form.h);
    t.app.put_event(Event::MouseDown(mouse(fx + 1, fy + 2)));
    t.app.put_event(Event::MouseMove(mouse(fx + w - 2, fy + h - 1)));
    t.app.put_event(Event::MouseUp(MouseEvent { buttons: 0, ..mouse(fx + w - 2, fy + h - 1) }));
    t.pump();
    let mut sel = doc.borrow().sel.clone();
    sel.sort();
    assert_eq!(sel, vec![0, 1], "выделение рамкой");
    // Выравнивание по левому краю основного.
    t.cmd(CM_ALIGN_BASE);
    let c = t.doc_controls();
    assert_eq!(c[0].x, c[1].x);
}

#[test]
fn inspector_edits_property() {
    let mut t = T::new("insp");
    t.open_about();
    t.cmd(CM_INS_BASE + 2); // кнопка
    t.cmd(CM_SHOW_INSPECTOR);
    // Строки: Имя, X, Y, Ширина, Высота, Надпись…
    for _ in 0..5 {
        t.key(KeyEvent::new(Key::Down, Mods::NONE));
    }
    let mut ks = vec![kb::f(2), KeyEvent::new(Key::Home, Mods::NONE), KeyEvent::new(Key::End, Mods::SHIFT), KeyEvent::new(Key::Del, Mods::NONE)];
    ks.extend(t.typed("~П~ривет"));
    ks.push(kb::ENTER);
    t.keys(&ks);
    let c = t.doc_controls();
    assert_eq!(c[2].text, "~П~ривет", "надпись кнопки");
    assert!(Ide::top_doc(&t.app).unwrap().borrow().modified);
    // Неверное значение не принимается: ширина «abc».
    t.keys(&[KeyEvent::new(Key::Up, Mods::NONE), KeyEvent::new(Key::Up, Mods::NONE)]);
    let mut ks = t.typed("abc");
    ks.push(kb::ENTER);
    t.keys(&ks);
    assert_eq!(t.doc_controls()[2].w, 12);
    assert!(t.ide.last_error.as_deref().unwrap_or("").contains("Ширина"), "{:?}", t.ide.last_error);
}

#[test]
fn props_dialog_renames_control() {
    let mut t = T::new("props");
    t.open_about();
    t.cmd(CM_INS_BASE + 3);
    // Диалог свойств: первое поле — имя (весь текст выделен, набор заменяет).
    t.app.put_event(Event::command_event(CM_DS_PROPS));
    for c in "user_name".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::ENTER));
    t.pump();
    assert_eq!(t.doc_controls()[2].name, "user_name");
}

#[test]
fn menu_editor_adds_item_and_command() {
    let mut t = T::new("menu");
    t.app.put_event(Event::command_event(CM_EDIT_MENU));
    // Курсор на «Файл»; Ins — новый пункт внутри подменю.
    t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Ins, Mods::NONE)));
    for c in "~П~ривет".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::TAB));
    for c in "hello".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::TAB));
    for c in "Ctrl+H".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::ENTER));
    t.app.put_event(Event::command_event(cm::OK));
    t.pump();
    let p = t.ide.project.as_ref().unwrap();
    let MenuNode::Sub { children, .. } = &p.app.menu[0] else { panic!("подменю") };
    assert!(
        children
            .iter()
            .any(|c| matches!(c, MenuNode::Item { title, command, key, .. } if title == "~П~ривет" && command == "HELLO" && key == "Ctrl+H")),
        "{:?}",
        children
    );
    assert!(p.app.commands.iter().any(|c| c.name == "HELLO"));
    // Модель сохранена в файл проекта.
    let saved = Project::load(&p.file()).unwrap();
    assert_eq!(saved.app, p.app);
}

#[test]
fn f12_opens_generated_code_with_highlight() {
    let mut t = T::new("f12");
    t.open_about();
    t.cmd(CM_FORM_CODE);
    let ids = Ide::windows::<CodeWindow>(&t.app);
    assert_eq!(ids.len(), 1);
    let w = t.app.find::<CodeWindow>(ids[0]).unwrap();
    assert!(w.path().ends_with("src/forms/about.rs"));
    assert!(w.editor().fe.editor.text().contains("pub struct AboutData"));
    assert!(t.screen().contains("Сгенерировано TV Studio"), "{}", t.screen());
    assert!(t.dir.join("src/ui.rs").exists());
    // F12 из кода формы — обратно в дизайнер.
    t.cmd(CM_FORM_CODE);
    let top = t.app.desktop().group.current_view().map(|v| v.vs().id);
    assert_eq!(top, Ide::top_designer(&t.app));
    assert!(Ide::windows::<DesignWindow>(&t.app).len() == 1);
}

#[test]
fn new_form_via_dialog() {
    let mut t = T::new("newform");
    t.app.put_event(Event::command_event(CM_NEW_FORM));
    // Имя выделено — набор заменяет.
    for c in "settings".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::ENTER));
    t.pump();
    let p = t.ide.project.as_ref().unwrap();
    assert!(p.forms.contains(&"settings".to_string()), "{:?}", p.forms);
    assert!(p.form_path("settings").exists());
    let d = Ide::top_doc(&t.app).unwrap();
    assert_eq!(d.borrow().form.name, "settings");
    assert!(t.screen().contains("settings"), "дерево проекта обновлено");
}

/// Полный цикл: проект собирается cargo, ошибка в main.rs находится,
/// F8 открывает файл на строке ошибки, в полосе номеров — пометка.
#[test]
fn build_finds_error_and_jumps() {
    let mut t = T::new("build");
    std::env::set_var("CARGO_TARGET_DIR", std::env::temp_dir().join("tvstudio_ui_target"));
    let main = t.dir.join("src/main.rs");
    let mut src = std::fs::read_to_string(&main).unwrap();
    src.push_str("\nfn broken() -> u32 {\n    \"не число\"\n}\n");
    std::fs::write(&main, src.clone()).unwrap();
    let err_line = src.lines().position(|l| l.contains("не число")).unwrap() + 1;
    t.cmd(CM_CHECK);
    let t0 = std::time::Instant::now();
    while t.ide.job.is_some() && t0.elapsed().as_secs() < 240 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        t.pump();
    }
    assert!(t.ide.job.is_none(), "cargo не завершился");
    let s = t.screen();
    assert!(s.contains("не удалась"), "итог сборки:\n{}", s);
    t.cmd(CM_NEXT_ERR);
    let ids = Ide::windows::<CodeWindow>(&t.app);
    let msgs: Vec<String> = t
        .ide
        .messages
        .and_then(|id| t.app.find::<tvstudio::toolwins::MessagesWindow>(id))
        .map(|m| m.entries.iter().map(|e| format!("{:?} {:?} {}", e.kind, e.loc, e.text)).collect())
        .unwrap_or_default();
    let Some(&last) = ids.last() else { panic!("F8 не открыл файл; сообщения:\n{}", msgs.join("\n")) };
    let w = t.app.find::<CodeWindow>(last).unwrap();
    assert!(w.path().ends_with("src/main.rs"));
    assert_eq!(w.editor().fe.editor.cursor().line + 1, err_line);
    assert!(t.screen().contains('●'), "пометка ошибки в полосе номеров");
    // Окно «Ошибки и предупреждения» (Ctrl+F8): ошибка сборки, Enter — к месту.
    t.key(kb::ctrl_f(8));
    let pid = t.ide.problems.expect("окно ошибок открыто");
    let lines = t.app.find::<tvstudio::toolwins::ProblemsWindow>(pid).unwrap().lines();
    let want = format!("■ src/main.rs:{}:", err_line);
    assert!(lines.iter().any(|l| l.starts_with(&want) && l.ends_with("[cargo]")), "{:?}", lines);
    assert!(t.screen().contains("Ошибки и предупреждения: ■"), "{}", t.screen());
    let i = lines.iter().position(|l| l.starts_with(&want)).unwrap();
    for _ in 0..i {
        t.key(KeyEvent::new(Key::Down, Mods::NONE));
    }
    t.key(KeyEvent::new(Key::Enter, Mods::NONE));
    let cur = Ide::current_window(&t.app).and_then(|id| t.app.find::<CodeWindow>(id)).expect("Enter открыл файл");
    assert_eq!(cur.editor().fe.editor.cursor().line + 1, err_line);
    // Исправляем и собираем снова — ошибок нет.
    std::fs::write(&main, src.replace("\"не число\"", "42")).unwrap();
    {
        let w = t.app.find_mut::<CodeWindow>(*ids.last().unwrap()).unwrap();
        w.editor_mut().fe.load_file();
    }
    t.cmd(CM_CHECK);
    let t0 = std::time::Instant::now();
    while t.ide.job.is_some() && t0.elapsed().as_secs() < 240 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        t.pump();
    }
    // Окно ошибок поверх сообщений — итог сборки смотрим в самих сообщениях.
    let done = t
        .ide
        .messages
        .and_then(|id| t.app.find::<tvstudio::toolwins::MessagesWindow>(id))
        .map(|m| m.entries.iter().any(|e| e.text.contains("завершена")))
        .unwrap_or(false);
    assert!(done, "{}", t.screen());
    // После исправления ошибок в окне больше нет.
    let pw = t.app.find::<tvstudio::toolwins::ProblemsWindow>(pid).unwrap();
    assert_eq!(pw.counts().0, 0, "{:?}", pw.lines());
}

#[test]
fn cursor_line_and_column_highlight() {
    let mut t = T::new("hl");
    let f = t.dir.join("src").join("hl.rs");
    std::fs::write(&f, "aaaa1\nbbbb2\ncccc3\ndddd4\n").unwrap();
    let id = t.ide.open_file(&mut t.app, &f, Some((3, 5))).unwrap();
    t.pump();
    // Где на экране строки файла.
    let text = t.screen();
    let rows: Vec<&str> = text.lines().collect();
    let find = |s: &str| {
        let y = rows.iter().position(|r| r.contains(s)).unwrap();
        let cs: Vec<char> = rows[y].chars().collect();
        let x = cs.windows(s.chars().count()).position(|w| w.iter().collect::<String>() == s).unwrap();
        (x as i32, y as i32)
    };
    let (xa, ya) = find("aaaa1");
    let (xc, yc) = find("cccc3");
    let at = |t: &T, x: i32, y: i32| t.app.screen().get(x as usize, y as usize).attr;
    // Строка курсора — другим фоном, чем обычная строка.
    assert_ne!(at(&t, xc + 1, yc), at(&t, xa + 1, ya), "строка курсора не подсвечена");
    // Столбец курсора (5-й символ) подсвечен и в других строках — тем же фоном.
    assert_ne!(at(&t, xa + 4, ya), at(&t, xa + 3, ya), "столбец курсора не подсвечен");
    assert_eq!(at(&t, xa + 4, ya), at(&t, xc + 1, yc));
    // Выключили — всё одним цветом.
    {
        let w = t.app.find_mut::<CodeWindow>(id).unwrap();
        w.editor_mut().hl_line = false;
        w.editor_mut().hl_col = false;
    }
    t.pump();
    assert_eq!(at(&t, xc + 1, yc), at(&t, xa + 1, ya));
    assert_eq!(at(&t, xa + 4, ya), at(&t, xa + 3, ya));
}

#[test]
fn rustfmt_on_save() {
    if std::process::Command::new("rustfmt").arg("--version").output().is_err() {
        return;
    }
    let mut t = T::new("fmt");
    let f = t.dir.join("src").join("messy.rs");
    std::fs::write(&f, "pub fn  add(a:i32,b:i32)->i32{a+b}\n").unwrap();
    let id = t.ide.open_file(&mut t.app, &f, None).unwrap();
    t.pump();
    // Правка и F2: файл сохраняется уже отформатированным.
    t.app.find_mut::<CodeWindow>(id).unwrap().goto(1, 37);
    t.keys(&[KeyEvent::new(Key::Enter, Mods::NONE)]);
    let typed = t.typed("pub fn  sub(a:i32,b:i32)->i32{a-b}");
    t.keys(&typed);
    t.key(kb::F2);
    let want = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\npub fn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
    assert_eq!(std::fs::read_to_string(&f).unwrap(), want);
    let w = t.app.find::<CodeWindow>(id).unwrap();
    assert!(!w.editor().fe.editor.modified);
    assert_eq!(w.editor().fe.editor.line(3), "pub fn sub(a: i32, b: i32) -> i32 {");
    // Ошибка синтаксиса: сохраняется как есть, rustfmt пишет, где ошибка.
    t.app.find_mut::<CodeWindow>(id).unwrap().goto(1, 1);
    let typed = t.typed("fn broken( {");
    t.keys(&typed);
    t.key(kb::F2);
    assert!(std::fs::read_to_string(&f).unwrap().starts_with("fn broken( {pub fn add"));
    let m = t.app.find::<tvstudio::toolwins::MessagesWindow>(t.ide.messages.unwrap()).unwrap();
    assert!(m.entries.iter().any(|e| e.text.contains("rustfmt: src/messy.rs не отформатирован")), "{:?}", m.entries);
    assert!(m.entries.iter().any(|e| e.loc.as_ref().map(|l| l.0.ends_with("messy.rs")).unwrap_or(false)));
}

/// «Новый проект» → «Программа на Rust»: только код — окон «Компоненты» и
/// «Инспектор» нет, в дереве нет форм и модели, команды форм выключены,
/// генерации кода нет. Вернулись к проекту tvision-rs — окна снова на месте.
#[test]
fn new_rust_project_hides_form_tools() {
    let mut t = T::new("rustproj");
    let demo = t.ide.project.as_ref().unwrap().file();
    t.app.put_event(Event::command_event(CM_NEW_PROJECT));
    // Первым выбран «Тип»: ↓ — «Программа на Rust»; Tab — к имени (выделено, набор заменяет).
    t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Down, Mods::NONE)));
    t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Tab, Mods::NONE)));
    for c in "plain".chars() {
        t.app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)));
    }
    t.app.put_event(Event::KeyDown(kb::ENTER));
    t.pump();

    let p = t.ide.project.clone().expect("проект создан");
    assert_eq!((p.name.as_str(), p.kind), ("plain", ProjectKind::Rust));
    assert_eq!(p.dir, t.dir.parent().unwrap().join("plain"));
    let cargo = std::fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("tvision"), "{}", cargo);

    let s = t.screen();
    for gone in ["Компоненты", "Инспектор", "Формы", "Модель приложения"] {
        assert!(!s.contains(gone), "«{}» на экране:\n{}", gone, s);
    }
    assert!(s.contains("Проект") && s.contains("Сообщения") && s.contains("main.rs"), "{}", s);
    assert!(t.ide.inspector.is_none());
    // Дерево проекта — у правого края, редактор main.rs — слева от него.
    let desk = t.app.desktop().group.vs.size;
    let pb = t.app.group.view(t.ide.project_win.unwrap()).unwrap().vs().bounds();
    assert_eq!(pb.b.x, desk.x, "проект справа: {:?}", pb);
    // main.rs открыт в редакторе и в фокусе.
    let main = p.dir.join("src").join("main.rs");
    let ed = Ide::windows::<CodeWindow>(&t.app).into_iter().find(|&id| t.app.find::<CodeWindow>(id).unwrap().path() == main).expect("main.rs открыт");
    let eb = t.app.group.view(ed).unwrap().vs().bounds();
    assert_eq!((eb.a.x, eb.b.x), (0, pb.a.x), "редактор — от левого края до дерева проекта");

    for c in [CM_NEW_FORM, CM_OPEN_FORM, CM_EDIT_MENU, CM_GENERATE, CM_EXTRACT_FORMS] {
        assert!(!t.app.command_enabled(c), "команда {} включена", c);
    }
    assert!(t.app.command_enabled(CM_BUILD) && t.app.command_enabled(CM_RUN));
    // F11, «Палитра компонентов» и «Раскладка по умолчанию» окна не возвращают.
    t.cmd(CM_SHOW_INSPECTOR);
    t.cmd(CM_SHOW_PALETTE);
    t.cmd(CM_LAYOUT);
    t.ide.ensure_tools(&mut t.app);
    t.pump();
    assert!(t.ide.inspector.is_none());
    // Генерация (перед сборкой) ничего не пишет.
    assert!(t.ide.generate(&mut t.app, true));
    assert!(!p.dir.join("src").join("ui.rs").exists() && !p.dir.join("src").join("forms").exists());

    // Снова проект tvision-rs — палитра и инспектор вернулись.
    t.ide.open_project(&mut t.app, &demo);
    t.pump();
    let s = t.screen();
    assert!(s.contains("Компоненты") && s.contains("Инспектор") && s.contains("Формы"), "{}", s);
    assert!(t.app.command_enabled(CM_NEW_FORM));
    let pb = t.app.group.view(t.ide.project_win.unwrap()).unwrap().vs().bounds();
    let ib = t.app.group.view(t.ide.inspector.unwrap()).unwrap().vs().bounds();
    assert_eq!((pb.b.x, pb.b.y), (desk.x, ib.a.y), "проект tvision-rs — дерево над инспектором: {:?}", pb);
}

/// `tvstudio КАТАЛОГ`: в каталоге исходники Rust (Cargo.toml), а файла
/// проекта нет — среда создаёт проект на Rust и открывает его; второй
/// запуск открывает уже созданный проект. Каталог без Cargo.toml — сообщение.
#[test]
fn startup_with_cargo_dir_creates_rust_project() {
    let base = std::env::temp_dir().join(format!("tvs_ui_adopt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("tool");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    let empty = base.join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    let run = |args: Vec<String>| {
        let mut b = HeadlessBackend::new(120, 40);
        b.esc_budget = 0;
        let mut app = Program::new(Box::new(b), Some(menu_bar()), Some(status_line()));
        let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
        ide.startup(&mut app, args);
        app.pump(&mut ide);
        app.pump(&mut ide);
        (app, ide)
    };

    let (mut app, mut ide) = run(vec![dir.display().to_string()]);
    let p = ide.project.clone().expect("проект открыт");
    assert_eq!((p.name.as_str(), p.kind), ("tool", ProjectKind::Rust));
    assert!(dir.join("tool.tvsp").is_file());
    let s = app.screen_text();
    assert!(s.contains("Создан проект на Rust"), "{}", s);
    assert!(!s.contains("Компоненты") && !s.contains("Инспектор"), "{}", s);
    assert!(Ide::windows::<CodeWindow>(&app).iter().any(|&id| app.find::<CodeWindow>(id).unwrap().path() == dir.join("src/main.rs")));
    ide.shutdown(&mut app);

    // Второй запуск — тот же проект, новый не создаётся.
    let (mut app, mut ide) = run(vec![dir.display().to_string()]);
    assert_eq!(ide.project.as_ref().map(|p| p.file()), Some(dir.join("tool.tvsp")));
    assert!(!app.screen_text().contains("Создан проект"));
    ide.shutdown(&mut app);

    // Не исходники Rust — только сообщение.
    let (app, ide) = run(vec![empty.display().to_string()]);
    assert!(ide.project.is_none());
    assert!(app.screen_text().contains("Это не исходники Rust"), "{}", app.screen_text());
    assert!(std::fs::read_dir(&empty).unwrap().next().is_none(), "в каталоге ничего не создано");
    let _ = std::fs::remove_dir_all(&base);
}

/// Справка разбирается целиком (строка, начатая с «.», — команда справки;
/// ошибка в ней подменила бы всю справку заглушкой).
#[test]
fn help_file_parses() {
    let h = tvision::HelpFile::parse(tvstudio::ide::HELP_SOURCE).expect("справка разбирается");
    let _ = h;
}

/// Ни у одного окна среды нет тени: инструменты, дизайнер формы, редактор
/// кода, справка; диалоги и сообщения строятся без тени.
#[test]
fn windows_have_no_shadow() {
    let mut t = T::new("shadow");
    t.open_about();
    let main = t.dir.join("src").join("main.rs");
    t.ide.open_file(&mut t.app, &main, None).unwrap();
    t.app.put_event(Event::KeyDown(kb::F1));
    t.pump();
    let ids: Vec<ViewId> = t.app.desktop().group.children.iter().map(|c| c.vs().id).collect();
    assert!(ids.len() >= 6, "окна: {}", ids.len());
    for id in ids {
        let v = t.app.group.view(id).unwrap();
        assert_eq!(v.vs().state & sf::SHADOW, 0, "у окна {} тень", v.as_window().map(|w| w.title.clone()).unwrap_or_default());
    }
    let d = tvstudio::dialogs::build_message_box(Rect::new(0, 0, 40, 9), "x", mf::OK_BUTTON);
    assert_eq!(d.win.group.vs.state & sf::SHADOW, 0);
    assert_eq!(tvstudio::dialogs::flat(Dialog::centered(30, 8, "x")).win.group.vs.state & sf::SHADOW, 0);
}

/// `tvstudio КАТАЛОГ` с workspace (без своего пакета): в дереве проекта —
/// участники по `members` (с шаблонами, без `exclude`), у каждого src и
/// Cargo.toml; в редакторе — main.rs первого участника, где он есть.
#[test]
fn workspace_members_in_project_tree() {
    let base = std::env::temp_dir().join(format!("tvs_ui_ws_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ws = base.join("ws");
    for (rel, name, file) in [("crates/app", "ws-app", "main.rs"), ("crates/core", "ws-core", "lib.rs"), ("crates/skip", "ws-skip", "lib.rs")] {
        let d = ws.join(rel);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("Cargo.toml"), format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n", name)).unwrap();
        std::fs::write(d.join("src").join(file), "fn main() {}\n").unwrap();
    }
    std::fs::create_dir_all(ws.join("crates/app/tests")).unwrap();
    std::fs::write(ws.join("crates/app/tests/it.rs"), "#[test]\nfn t() {}\n").unwrap();
    std::fs::write(ws.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\nexclude = [\"crates/skip\"]\n").unwrap();

    let mut b = HeadlessBackend::new(120, 40);
    b.esc_budget = 0;
    let mut app = Program::new(Box::new(b), Some(menu_bar()), Some(status_line()));
    let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
    ide.startup(&mut app, vec![ws.display().to_string()]);
    app.pump(&mut ide);
    app.pump(&mut ide);
    let s = app.screen_text();
    assert!(s.contains("◇ ws-app") && s.contains("◇ ws-core"), "участники в дереве:\n{}", s);
    assert!(!s.contains("ws-skip"), "exclude:\n{}", s);
    assert!(s.contains("Cargo.toml"), "{}", s);
    let main = ws.join("crates/app/src/main.rs");
    assert!(
        Ide::windows::<CodeWindow>(&app).iter().any(|&id| tvstudio::ide::same_file(&app.find::<CodeWindow>(id).unwrap().path(), &main)),
        "открыт main.rs первого участника"
    );
    ide.shutdown(&mut app);
    let _ = std::fs::remove_dir_all(&base);
}

/// Скобки в редакторе кода: пара при вводе, перешагивание своей
/// закрывающей, Backspace в пустой паре, Enter в {}, обёртка выделения;
/// перед словом, в строке и в комментарии — без пары; выключается настройкой.
#[test]
fn editor_auto_closes_brackets() {
    let mut t = T::new("brackets");
    let f = t.dir.join("src").join("b.rs");
    std::fs::write(&f, "").unwrap();
    let id = t.ide.open_file(&mut t.app, &f, None).unwrap();
    t.app.focus(id);
    t.pump();
    let text = |t: &T| t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.text();
    let cur = |t: &T| t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.cursor();
    let typed = |t: &mut T, s: &str| {
        for c in s.chars() {
            let k = if c == '\n' { KeyEvent::new(Key::Enter, Mods::NONE) } else { KeyEvent::new(Key::Char(c), Mods::NONE) };
            t.key(k);
        }
    };

    // Пара, набор внутри, перешагивание закрывающей.
    typed(&mut t, "fn f(");
    assert_eq!(text(&t), "fn f()");
    assert_eq!(cur(&t).col, 5);
    typed(&mut t, "x: [u8; 2]) ");
    assert_eq!(text(&t), "fn f(x: [u8; 2]) ", "] и ) перешагнули вставленные");
    // Enter внутри {} — по строкам с отступом.
    typed(&mut t, "{\n");
    assert_eq!(text(&t), "fn f(x: [u8; 2]) {\n    \n}");
    assert_eq!((cur(&t).line, cur(&t).col), (1, 4));
    // Backspace в пустой паре — обе.
    typed(&mut t, "g(");
    assert_eq!(text(&t), "fn f(x: [u8; 2]) {\n    g()\n}");
    t.key(KeyEvent::new(Key::Backspace, Mods::NONE));
    assert_eq!(text(&t), "fn f(x: [u8; 2]) {\n    g\n}");
    // Перед словом — без пары; в строке и в комментарии — тоже.
    t.key(KeyEvent::new(Key::Home, Mods::NONE));
    t.key(KeyEvent::new(Key::End, Mods::NONE));
    typed(&mut t, " \"a(\" // b(");
    assert_eq!(t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.line(1), "    g \"a(\" // b(");
    t.key(KeyEvent::new(Key::Home, Mods::NONE));
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    t.key(KeyEvent::new(Key::Right, Mods::NONE));
    typed(&mut t, "(");
    assert_eq!(t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.line(1), "    (g \"a(\" // b(", "перед словом — одна скобка");
    // Закрывающая, которую никто не вставлял, — обычный ввод.
    t.key(KeyEvent::new(Key::End, Mods::NONE));
    typed(&mut t, ")");
    assert!(t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.line(1).ends_with("b()"));
    // Выделение — в скобки.
    {
        let w = t.app.find_mut::<CodeWindow>(id).unwrap();
        let e = &mut w.editor_mut().fe.editor;
        e.set_select(tvision::Pos::new(0, 3), tvision::Pos::new(0, 4));
    }
    typed(&mut t, "[");
    assert!(text(&t).starts_with("fn [f](x"), "{}", text(&t));
    // Кавычки: пара, перешагивание, Backspace в пустых; после слова — нет
    // (кроме r"…" в Rust); между двумя строками Backspace стирает одну.
    t.key(KeyEvent::new(Key::End, Mods::CTRL));
    typed(&mut t, "\nlet s = \"ab\";");
    let last = |t: &T| {
        let e = &t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor;
        e.line(e.line_count() - 1)
    };
    assert_eq!(last(&t), "let s = \"ab\";", "закрывающая кавычка перешагнула вставленную");
    typed(&mut t, " f(\"");
    assert_eq!(last(&t), "let s = \"ab\"; f(\"\")");
    t.key(KeyEvent::new(Key::Backspace, Mods::NONE));
    assert_eq!(last(&t), "let s = \"ab\"; f()");
    typed(&mut t, "r\"");
    assert_eq!(last(&t), "let s = \"ab\"; f(r\"\")", "r\"…\" — пара");
    t.key(KeyEvent::new(Key::End, Mods::NONE));
    typed(&mut t, " x\"");
    assert_eq!(last(&t), "let s = \"ab\"; f(r\"\") x\"", "после слова — одна кавычка");
    // «"a"|"b"»: Backspace стирает только закрывающую первой строки.
    typed(&mut t, "\n");
    {
        let w = t.app.find_mut::<CodeWindow>(id).unwrap();
        w.editor_mut().fe.editor.insert_text("\"a\"\"b\"", false);
        let l = w.editor().fe.editor.cursor().line;
        w.editor_mut().fe.editor.set_select(tvision::Pos::new(l, 3), tvision::Pos::new(l, 3));
    }
    t.key(KeyEvent::new(Key::Backspace, Mods::NONE));
    assert_eq!(last(&t), "\"a\"b\"");
    // Выключено настройкой — только то, что набрано.
    t.app.find_mut::<CodeWindow>(id).unwrap().editor_mut().auto_close = false;
    t.key(KeyEvent::new(Key::End, Mods::CTRL));
    typed(&mut t, "\n(");
    assert!(text(&t).ends_with("\"a\"b\"\n("), "{:?}", text(&t));
}

/// Без сервера LSP подсказки при наборе — из словаря проекта: имена из
/// файлов .rs и открытого текста, модули (имена файлов), крейты из
/// Cargo.toml.
#[test]
fn completion_from_project_words_without_lsp() {
    let mut t = T::new("words");
    std::fs::write(t.dir.join("src").join("netcode.rs"), "pub fn send_packet_now() {}\n").unwrap();
    let f = t.dir.join("src").join("w.rs");
    std::fs::write(&f, "fn local_counter_value() {}\n").unwrap();
    let id = t.ide.open_file(&mut t.app, &f, None).unwrap();
    t.app.focus(id);
    t.pump();
    let ed = |t: &T| t.app.find::<CodeWindow>(id).unwrap().editor().completion.clone();
    t.key(KeyEvent::new(Key::End, Mods::CTRL));
    for c in "send".chars() {
        t.key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    let c = ed(&t).expect("список из файлов проекта");
    let labels: Vec<String> = c.shown.iter().map(|&i| c.items[i].label.clone()).collect();
    assert!(labels.contains(&"send_packet_now".to_string()), "{:?}", labels);
    t.key(KeyEvent::new(Key::Esc, Mods::NONE));
    t.key(KeyEvent::new(Key::Char(' '), Mods::NONE));
    for c in "netc".chars() {
        t.key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    let c = ed(&t).expect("модуль");
    assert_eq!(c.items[c.shown[0]].label, "netcode");
    assert_eq!(c.items[c.shown[0]].kind, 9);
    t.key(KeyEvent::new(Key::Tab, Mods::NONE));
    assert!(t.app.find::<CodeWindow>(id).unwrap().editor().fe.editor.text().ends_with("send netcode"));
    // Имя из открытого текста.
    t.key(KeyEvent::new(Key::Char(' '), Mods::NONE));
    for c in "local_c".chars() {
        t.key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    let c = ed(&t).expect("из открытого текста");
    assert_eq!(c.items[c.shown[0]].label, "local_counter_value");
}
