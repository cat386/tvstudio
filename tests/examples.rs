//! Примеры tvision-rs, импортированные в проекты TV Studio (`examples/`).
//!
//! Быстрый тест: каждый проект открывается, модель без ошибок, а
//! сгенерированный код совпадает с моделью (ничего не устарело).
//! Долгий (`--ignored`): каждый проект собирается и проходит свои тесты.

use std::path::PathBuf;

use tvstudio::codegen;
use tvstudio::model::{Form, Project};

fn projects() -> Vec<Project> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v.iter()
        .map(|d| {
            let name = d.file_name().unwrap().to_string_lossy().to_string();
            Project::load(&d.join(format!("{}.tvsp", name))).unwrap_or_else(|e| panic!("{}: {}", name, e))
        })
        .collect()
}

#[test]
fn examples_are_consistent() {
    let ps = projects();
    assert_eq!(ps.len(), 23, "все примеры tvision-rs");
    for p in &ps {
        let forms: Vec<Form> = p.forms.iter().map(|n| Form::load(&p.form_path(n)).unwrap()).collect();
        let w = codegen::check_project(p, &forms);
        assert!(w.is_empty(), "{}: {:?}", p.name, w);
        assert_eq!(std::fs::read_to_string(p.dir.join("src/ui.rs")).unwrap(), codegen::ui_rs(p, &forms), "{}: ui.rs устарел", p.name);
        for f in &forms {
            assert_eq!(std::fs::read_to_string(p.form_code_path(&f.name)).unwrap(), codegen::form_rs(p, f), "{}: форма {}", p.name, f.name);
        }
    }
    // Формы дизайнера есть у учебных примеров.
    let with_forms: Vec<&str> = ps.iter().filter(|p| !p.forms.is_empty()).map(|p| p.name.as_str()).collect();
    assert_eq!(with_forms, vec!["hello", "learn_05_dialog", "learn_06_form"]);
}

#[test]
#[ignore]
fn examples_build_and_pass_tests() {
    let target = std::env::temp_dir().join("tvstudio_examples_target");
    for p in projects() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["test", "--offline", "--message-format=short"])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&p.dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}:\n{}", p.name, String::from_utf8_lossy(&out.stderr));
    }
}
