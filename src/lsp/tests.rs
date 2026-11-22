use super::*;

#[test]
fn command_quotes_preserve_windows_paths_and_arguments() {
    assert_eq!(
        command_parts(r#""C:\Program Files\Rust\rust-analyzer.exe" --log-file "C:\My Logs\ra.log" """#).unwrap(),
        vec![r"C:\Program Files\Rust\rust-analyzer.exe", "--log-file", r"C:\My Logs\ra.log", ""]
    );
    assert!(command_parts("\"unfinished").is_err());
    assert!(command_parts("  ").is_err());
}

#[test]
fn writer_reports_io_errors() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (writer, errors) = start_writer(Broken);
    writer.try_send("{}".into()).unwrap();
    assert!(errors.recv_timeout(Duration::from_secs(2)).unwrap().contains("closed"));
}

#[test]
fn uris() {
    let p = Path::new("/home/кот/my app/src/main.rs");
    let u = path_to_uri(p);
    assert_eq!(u, "file:///home/%D0%BA%D0%BE%D1%82/my%20app/src/main.rs");
    assert_eq!(uri_to_path(&u), p);
    assert_eq!(uri_to_path("file:///C%3A/work/a.rs"), PathBuf::from("C:/work/a.rs"));
    assert_eq!(uri_to_path("file:///c:/work/a.rs"), PathBuf::from("c:/work/a.rs"));
}

#[test]
fn columns() {
    let l = "let 😀 = ы;";
    assert_eq!(col_to_lsp(l, 6, Encoding::Utf16), 7);
    assert_eq!(col_from_lsp(l, 7, Encoding::Utf16), 6);
    assert_eq!(col_to_lsp(l, 6, Encoding::Utf32), 6);
    assert_eq!(col_to_lsp(l, 9, Encoding::Utf8), 13);
    assert_eq!(col_from_lsp(l, 13, Encoding::Utf8), 9);
}

#[test]
fn snippets() {
    assert_eq!(strip_snippet("push_str(${1:string})$0"), "push_str(string)");
    assert_eq!(strip_snippet("fn ${1:name}($2) {\n    $0\n}"), "fn name() {\n    \n}");
    assert_eq!(strip_snippet("a\\$b"), "a$b");
    assert_eq!(strip_snippet("${1|one,two|}"), "one");
}

#[test]
fn parses_server_replies() {
    let c = json::parse(r#"{"isIncomplete":false,"items":[{"label":"push_str(…)","kind":2,"detail":"fn(&mut self, &str)","insertTextFormat":2,"textEdit":{"range":{"start":{"line":1,"character":6},"end":{"line":1,"character":8}},"newText":"push_str(${1:s})$0"},"sortText":"b"},{"label":"len","sortText":"a"}]}"#).unwrap();
    let v = parse_completion(&c);
    assert_eq!(v[0].label, "len");
    assert_eq!(v[0].insert, "len");
    assert_eq!(v[1].insert, "push_str(s)");
    assert_eq!(v[1].range.unwrap().start, LPos { line: 1, col: 6 });
    let l = json::parse(r#"[{"targetUri":"file:///a/b.rs","targetRange":{"start":{"line":3,"character":0},"end":{"line":9,"character":1}},"targetSelectionRange":{"start":{"line":3,"character":3},"end":{"line":3,"character":7}}}]"#).unwrap();
    let v = parse_locations(&l);
    assert_eq!(v[0].path, PathBuf::from("/a/b.rs"));
    assert_eq!(v[0].range.start, LPos { line: 3, col: 3 });
    let d = json::parse(r#"{"uri":"file:///a/b.rs","diagnostics":[{"range":{"start":{"line":2,"character":4},"end":{"line":2,"character":9}},"severity":1,"code":"E0308","source":"rustc","message":"mismatched types"}]}"#).unwrap();
    let (p, v) = parse_diagnostics(&d);
    assert_eq!(p, PathBuf::from("/a/b.rs"));
    assert_eq!((v[0].severity, v[0].code.as_str()), (Severity::Error, "E0308"));
}

/// Подсказки в тексте: надпись — строка или части; пустые отброшены.
#[test]
fn inlay_hints_parse() {
    let r = json::parse(
        r#"[{"position":{"line":4,"character":11},"label":"✓","paddingLeft":true},
                {"position":{"line":5,"character":3},"label":[{"value":"❌ "},{"value":"1.0.210"}]},
                {"position":{"line":6,"character":0},"label":"  "}]"#,
    )
    .unwrap();
    let h = parse_inlay_hints(&r);
    assert_eq!(h.len(), 2);
    assert_eq!((h[0].pos, h[0].label.as_str()), (LPos { line: 4, col: 11 }, "✓"));
    assert_eq!(h[1].label, "❌ 1.0.210");
}

/// Быстрые исправления: обе формы WorkspaceEdit, команды без правок
/// отброшены, предпочтительные и quickfix — первыми.
#[test]
fn code_actions_parse() {
    let r = json::parse(
            r#"[{"title":"cmd","command":"x"},
                {"title":"only command","command":{"title":"x","command":"x"}},
                {"title":"refactor","kind":"refactor.rewrite","edit":{"documentChanges":[
                    {"textDocument":{"uri":"file:///p/Cargo.toml","version":3},"edits":[{"range":{"start":{"line":1,"character":0},"end":{"line":2,"character":0}},"newText":""}]},
                    {"kind":"create","uri":"file:///p/new.rs"}]}},
                {"title":"fix","kind":"quickfix","edit":{"changes":{"file:///p/Cargo.toml":[{"range":{"start":{"line":4,"character":9},"end":{"line":4,"character":12}},"newText":"1.0.210"}]}}},
                {"title":"best","kind":"refactor","isPreferred":true,"edit":{"changes":{"file:///p/a.rs":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"newText":"use x;\n"}]}}},
                {"title":"empty","edit":{"changes":{}}}]"#,
        )
        .unwrap();
    let a = parse_code_actions(&r);
    let titles: Vec<&str> = a.iter().map(|x| x.title.as_str()).collect();
    assert_eq!(titles, vec!["best", "fix", "refactor"]);
    assert_eq!(
        a[1].edits,
        vec![(
            PathBuf::from("/p/Cargo.toml"),
            vec![TextEdit { range: LRange { start: LPos { line: 4, col: 9 }, end: LPos { line: 4, col: 12 } }, text: "1.0.210".into() }]
        )]
    );
    assert_eq!(a[2].edits[0].1[0].text, "");
}
