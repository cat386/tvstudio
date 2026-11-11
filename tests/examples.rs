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

/// Файл с переводами строк «\n»: копия, выложенная git на Windows со
/// старыми настройками (до `.gitattributes`), может быть с «\r\n».
fn read_lf(p: &std::path::Path) -> String {
    std::fs::read_to_string(p).unwrap().replace("\r\n", "\n")
}

#[test]
fn examples_are_consistent() {
    let ps = projects();
    assert_eq!(ps.len(), 23, "все примеры tvision-rs");
    for p in &ps {
        let forms: Vec<Form> = p.forms.iter().map(|n| Form::load(&p.form_path(n)).unwrap()).collect();
        let w = codegen::check_project(p, &forms);
        assert!(w.is_empty(), "{}: {:?}", p.name, w);
        assert_eq!(read_lf(&p.dir.join("src/ui.rs")), codegen::ui_rs(p, &forms), "{}: ui.rs устарел", p.name);
        for f in &forms {
            assert_eq!(read_lf(&p.form_code_path(&f.name)), codegen::form_rs(p, f), "{}: форма {}", p.name, f.name);
        }
    }
    // Формы — у всех примеров с диалогами (25 форм в 13 проектах).
    let with_forms: Vec<&str> = ps.iter().filter(|p| !p.forms.is_empty()).map(|p| p.name.as_str()).collect();
    assert_eq!(
        with_forms,
        vec![
            "demo",
            "guide_contacts",
            "hello",
            "json_load",
            "json_table",
            "learn_04_coords",
            "learn_05_dialog",
            "learn_06_form",
            "lfm",
            "progress",
            "table",
            "treeview",
            "tvdemo"
        ]
    );
    assert_eq!(ps.iter().map(|p| p.forms.len()).sum::<usize>(), 25);
    // Повторное извлечение ничего не добавляет (формы уже есть).
    for p in &ps {
        let src = std::fs::read_to_string(p.dir.join("src/main.rs")).unwrap();
        let found = tvstudio::formextract::extract(&src, &std::collections::HashMap::new());
        for x in found {
            assert!(p.forms.contains(&x.form.name), "{}: форма {} не извлечена", p.name, x.form.name);
        }
    }
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
        // Вывод тестов примера (stdout) — тоже: по нему видно, что упало.
        assert!(out.status.success(), "{}:\n{}\n{}", p.name, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
}
