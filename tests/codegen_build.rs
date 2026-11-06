//! Сгенерированный проект действительно собирается cargo.
//!
//! Тест долгий (сборка tvision-rs в отдельном каталоге), поэтому помечен
//! `#[ignore]`: `cargo test --test codegen_build -- --ignored`.

use std::path::PathBuf;
use std::process::Command;

use tvstudio::codegen::generate_all;
use tvstudio::model::*;

fn tvision_path() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent().unwrap().join("tvision-rs").canonicalize().expect("tvision-rs рядом с tvstudio")
}

/// Форма, где есть все виды элементов и все необычные свойства.
fn big_form() -> Form {
    let mut f = Form::new("all_controls", "Все элементы");
    f.w = 76;
    f.h = 22;
    f.palette = Pal::Blue;
    f.help_ctx = 7;
    let mut y = 2;
    for k in Kind::ALL {
        if k == Kind::Label {
            continue;
        }
        let mut c = f.new_control(k, 16, y);
        match k {
            Kind::InputLine => {
                c.history = 2;
                c.validator = "picture:###-##".into();
                c.text = "123-45".into();
            }
            Kind::CheckBoxes => c.value = 3,
            Kind::ListBox => {
                c.columns = 2;
                c.value = 1;
                c.h = 3;
            }
            Kind::Memo => {
                c.text = "строка 1\nстрока \"2\"".into();
                c.h = 2;
            }
            Kind::ScrollBar => {
                c.x = 60;
                c.y = 2;
                c.h = 10;
            }
            Kind::Button => {
                c.command = "HELLO".into();
                c.disabled = true;
            }
            _ => {}
        }
        if k != Kind::ScrollBar {
            y += c.h;
        }
        f.controls.push(c);
    }
    let mut l = Control::new(Kind::Label, "input_label", 2, 4);
    l.text = "~С~трока:".into();
    l.link = "input1".into();
    f.controls.push(l);
    f
}

#[test]
#[ignore]
fn generated_project_builds() {
    let base = std::env::temp_dir().join(format!("tvstudio_build_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let dep = Dependency::Path(tvision_path().display().to_string());
    let mut p = Project::create(&base, "gen_test", &dep).unwrap();
    p.app
        .commands
        .push(CommandDef { name: "HELLO".into(), value: 101, action: Action::Message("Привет, \"мир\"!".into()), comment: "".into() });
    p.app
        .commands
        .push(CommandDef { name: "ALL".into(), value: 102, action: Action::Form("all_controls".into()), comment: "все элементы".into() });
    p.app.startup_form = "about".into();
    p.app.menu.push(MenuNode::Sub {
        title: "~Т~ест".into(),
        key: "Alt+T".into(),
        help: 4,
        children: vec![
            MenuNode::Item { title: "~В~се элементы".into(), command: "ALL".into(), key: "Ctrl+F2".into(), help: 3 },
            MenuNode::Sub {
                title: "~В~ложенное".into(),
                key: String::new(),
                help: 0,
                children: vec![MenuNode::Item { title: "~П~ривет".into(), command: "HELLO".into(), key: "".into(), help: 0 }],
            },
        ],
    });
    p.app.status.push(StatusEntry { text: "~Привет~".into(), key: "".into(), command: "HELLO".into() });
    let forms = vec![about_form("gen_test"), big_form()];
    p.forms = forms.iter().map(|f| f.name.clone()).collect();
    let warnings = tvstudio::codegen::check_project(&p, &forms);
    assert!(warnings.is_empty(), "{:?}", warnings);
    generate_all(&p, &forms).unwrap();
    let target = std::env::temp_dir().join("tvstudio_build_target");
    let out = Command::new(env!("CARGO"))
        .args(["build", "--offline", "--message-format=short"])
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(&p.dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{}", text);
    assert!(!text.contains("warning"), "предупреждения в сгенерированном коде:\n{}", text);
    if std::env::var("TVS_KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&base);
    }
}
