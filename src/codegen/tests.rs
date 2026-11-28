use super::*;
use crate::model::{about_form, Dependency};

#[test]
fn generates_form_code() {
    let dir = std::env::temp_dir().join(format!("tvs_cg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tv = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tvision-rs");
    let p = Project::create(&dir, "demo_app", &Dependency::Path(tv.display().to_string())).unwrap();
    let mut f = about_form("x");
    let mut i = f.new_control(Kind::InputLine, 3, 6);
    i.validator = "range:1..9".into();
    i.history = 3;
    f.controls.push(i);
    let code = form_rs(&p, &f);
    assert!(code.contains("pub struct AboutData"));
    assert!(code.contains("RangeValidator::new(1, 9)"));
    assert!(code.contains("History::new("));
    let ui = ui_rs(&p, &[f.clone()]);
    assert!(ui.contains("pub const ABOUT: u16 = 100;"));
    assert!(ui.contains("crate::forms::about::run"));
    assert!(check_project(&p, &[f.clone()]).is_empty());
    // Та же форма как окно: build() → Window, open()/read() вместо run().
    f.class = FormClass::Window;
    f.number = 3;
    f.resizable = false;
    let code = form_rs(&p, &f);
    assert!(code.contains("pub fn build() -> (Window, AboutIds)"));
    assert!(code.contains("Window::new(") && code.contains(", 3);"));
    assert!(code.contains("d.flags = wf::MOVE | wf::CLOSE;"));
    assert!(code.contains("d.group.vs.options |= of::CENTERED;"));
    assert!(code.contains("pub fn open(") && code.contains("pub fn read(") && !code.contains("pub fn run("));
    let ui = ui_rs(&p, &[f.clone()]);
    assert!(ui.contains("crate::forms::about::open(app"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn line_code_roundtrip() {
    let dir = std::env::temp_dir().join(format!("tvs_cg_line_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tv = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tvision-rs");
    let p = Project::create(&dir, "line_app", &Dependency::Path(tv.display().to_string())).unwrap();
    let mut f = about_form("x");
    let mut h = f.new_control(Kind::Line, 1, 5);
    h.double = true;
    h.to_frame = true;
    f.controls.push(h);
    let mut v = f.new_control(Kind::Line, 10, 1);
    v.vertical = true;
    v.w = 1;
    v.h = 4;
    f.controls.push(v);
    let code = form_rs(&p, &f);
    assert!(code.contains("Line::horizontal(") && code.contains(".double().to_frame()"), "{code}");
    assert!(code.contains("Line::vertical("), "{code}");
    let x = crate::formextract::extract(&code, &std::collections::HashMap::new());
    let g = &x[0].form;
    let lines: Vec<_> = g.controls.iter().filter(|c| c.kind == Kind::Line).collect();
    assert_eq!(lines.len(), 2);
    assert!(!lines[0].vertical && lines[0].double && lines[0].to_frame);
    assert_eq!((lines[0].x, lines[0].y, lines[0].w, lines[0].h), (1, 5, 20, 1));
    assert!(lines[1].vertical && !lines[1].double && !lines[1].to_frame);
    assert_eq!((lines[1].x, lines[1].y, lines[1].w, lines[1].h), (10, 1, 1, 4));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incremental_codegen_skips_unchanged() {
    let dir = std::env::temp_dir().join(format!("tvs_cg_incr_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tv = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tvision-rs");
    let p = Project::create(&dir, "incr_app", &Dependency::Path(tv.display().to_string())).unwrap();
    let f = about_form("x");
    let first = generate_all(&p, &[f.clone()]).unwrap();
    assert!(!first.is_empty());
    let ui = p.dir.join("src/ui.rs");
    let body = std::fs::read_to_string(&ui).unwrap();
    assert!(body.lines().nth(1).unwrap().starts_with(DIGEST_MARK));
    let second = generate_all(&p, &[f]).unwrap();
    assert!(second.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_codegen_digest_replaces_stamp() {
    let raw = format!("{GENERATED_MARK}\n// old\nfoo();\n");
    let d = 0xabc_du64;
    let stamped = add_codegen_digest(&raw, d);
    assert!(stamped.contains(&format!("{DIGEST_MARK} {d:016x}")));
    let again = add_codegen_digest(&stamped, d);
    assert_eq!(stamped, again);
}
