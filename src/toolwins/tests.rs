use super::*;

#[test]
fn problems_sorted_and_filtered() {
    let mut w = ProblemsWindow::new(Rect::new(0, 0, 80, 10));
    assert!(w.lines()[0].contains("нет"));
    let p =
        |kind, f: &str, line, src| Problem { kind, path: PathBuf::from(f), shown: f.into(), line, col: 1, text: format!("t{}", line), source: src };
    w.set_problems(vec![
        p(MsgKind::Warning, "src/a.rs", 3, "cargo"),
        p(MsgKind::Error, "src/b.rs", 9, "LSP"),
        p(MsgKind::Error, "src/a.rs", 7, "cargo"),
    ]);
    let l = w.lines();
    assert_eq!(l.len(), 3);
    assert!(l[0].starts_with("■ src/a.rs:7:1") && l[0].ends_with("[cargo]"), "{:?}", l);
    assert!(l[1].starts_with("■ src/b.rs:9:1") && l[1].ends_with("[LSP]"));
    assert!(l[2].starts_with("▲ src/a.rs:3:1"));
    assert_eq!(w.counts(), (2, 1));
    assert!(w.win.title.contains("■ 2") && w.win.title.contains("▲ 1"));
    // Только предупреждения.
    w.set_filter(PF_WARNINGS);
    assert_eq!(w.lines().len(), 1);
    assert_eq!(w.current().unwrap().line, 3);
    // Только ошибки.
    w.set_filter(PF_ERRORS);
    assert_eq!(w.lines().len(), 2);
    assert_eq!(w.current().unwrap().kind, MsgKind::Error);
}

#[test]
fn cargo_lines() {
    let e = parse_cargo_line("src/main.rs:12:5: error[E0425]: cannot find value `x`");
    assert_eq!(e.kind, MsgKind::Error);
    assert_eq!(e.loc, Some(("src/main.rs".into(), 12, 5)));
    let e = parse_cargo_line("C:\\work\\app\\src\\ui.rs:3:1: warning: unused import");
    assert_eq!(e.kind, MsgKind::Warning);
    assert_eq!(e.loc.unwrap().0, "C:\\work\\app\\src\\ui.rs");
    let e = parse_cargo_line("error: could not compile `app` (bin \"app\") due to 1 previous error");
    assert_eq!(e.kind, MsgKind::Error);
    assert!(e.loc.is_none());
    let e = parse_cargo_line("   Compiling app v0.1.0 (/x)");
    assert_eq!(e.kind, MsgKind::Info);
}
