use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lfm-example-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("left")).unwrap();
    std::fs::create_dir_all(d.join("right")).unwrap();
    d
}

fn key(k: Key, m: Mods) -> RawEvent {
    RawEvent::Key(KeyEvent::new(k, m))
}

fn text(s: &str) -> Vec<RawEvent> {
    s.chars().map(|c| key(Key::Char(c), Mods::NONE)).collect()
}

fn run(d: &Path, input: Vec<RawEvent>) -> (Program, Lfm) {
    let mut b = HeadlessBackend::new(100, 30);
    b.input.extend(input);
    let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
    let mut lfm = Lfm::build(&mut app, &d.join("left"), &d.join("right"));
    app.pump(&mut lfm);
    app.redraw();
    (app, lfm)
}

#[test]
fn mkdir_copy_delete() {
    let d = tmp("ops");
    std::fs::write(d.join("left/a.txt"), "привет").unwrap();
    std::fs::write(d.join("left/b.txt"), "мир").unwrap();
    let mut input = vec![key(Key::F(7), Mods::NONE)];
    input.extend(text("новый"));
    input.push(key(Key::Enter, Mods::NONE));
    // Выделить *.txt и скопировать в правую панель.
    input.push(key(Key::Char('+'), Mods::NONE));
    input.push(key(Key::Backspace, Mods::NONE));
    input.extend(text("*.txt"));
    input.push(key(Key::Enter, Mods::NONE));
    input.push(key(Key::F(5), Mods::NONE));
    input.push(key(Key::Enter, Mods::NONE));
    let (app, _) = run(&d, input);
    assert!(d.join("left/новый").is_dir(), "{}", app.screen_text());
    assert_eq!(std::fs::read_to_string(d.join("right/a.txt")).unwrap(), "привет");
    assert!(d.join("right/b.txt").exists());

    // Удалить в правой панели b.txt (Tab, к файлу, F8, Да).
    let mut input = vec![key(Key::Tab, Mods::NONE)];
    input.push(key(Key::Char('b'), Mods::ALT));
    input.push(key(Key::F(8), Mods::NONE));
    input.push(key(Key::Enter, Mods::NONE));
    let (_, _) = run(&d, input);
    assert!(!d.join("right/b.txt").exists());
    assert!(d.join("right/a.txt").exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn letters_tabs_and_filter() {
    let d = tmp("letters");
    std::fs::write(d.join("left/x.rs"), "").unwrap();
    std::fs::write(d.join("left/y.o"), "").unwrap();
    let mut input = vec![key(Key::Char(':'), Mods::NONE), key(Key::Char('f'), Mods::CTRL)];
    input.extend(text("*.o"));
    input.push(key(Key::Enter, Mods::NONE));
    input.push(key(Key::Char('.'), Mods::NONE)); // одна панель
    let (mut app, lfm) = run(&d, input);
    let s = app.screen_text();
    assert!(s.contains("1:left") && s.contains("2:left"), "{}", s);
    assert!(s.contains("x.rs") && !s.contains("y.o"), "{}", s);
    let dp = lfm.dp(&mut app);
    assert!(dp.single);
    assert_eq!(dp.active().tab_count(), 2);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn powercli_expands_and_quotes() {
    let v = Vars { dir: "/d".into(), other: "/o".into(), selected: vec!["a b".into()], all: vec![], bookmarks: HashMap::from([('1', "/bm".into())]) };
    let e = PanelEntry::from_path(Path::new("/nonexistent/it's.tar.gz"), "it's.tar.gz");
    let s = expand("cp $f $o/$E.$e $b1 $s $i $$", &v, Some(&e), 3);
    if cfg!(windows) {
        assert_eq!(s, "cp \"it's.tar.gz\" \"/o\"/\"it's\".\"tar.gz\" \"/bm\" \"a b\" 3 $");
    } else {
        assert_eq!(s, "cp 'it'\\''s.tar.gz' '/o'/'it'\\''s'.'tar.gz' '/bm' 'a b' 3 $");
    }
    assert_eq!(split_ext("x.TAR.GZ"), ("x".into(), "TAR.GZ".into()));
    assert_eq!(plural(21, "файл", "файла", "файлов"), "файл");
    assert_eq!(plural(12, "файл", "файла", "файлов"), "файлов");
}

#[cfg(unix)]
#[test]
fn powercli_runs_in_loop() {
    let d = tmp("cli");
    std::fs::write(d.join("left/one.txt"), "1").unwrap();
    std::fs::write(d.join("left/two.txt"), "2").unwrap();
    let mut input = vec![key(Key::Char('+'), Mods::NONE), key(Key::Backspace, Mods::NONE)];
    input.extend(text("*.txt"));
    input.push(key(Key::Enter, Mods::NONE));
    input.push(key(Key::Char('x'), Mods::CTRL));
    input.extend(text("cp $f $o/$E.bak"));
    input.push(key(Key::Enter, Mods::NONE));
    let (_, _) = run(&d, input);
    assert!(d.join("right/one.bak").exists() && d.join("right/two.bak").exists());
    let _ = std::fs::remove_dir_all(&d);
}
