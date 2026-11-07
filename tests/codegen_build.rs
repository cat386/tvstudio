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
        if k == Kind::Label || k == Kind::Table || k == Kind::TreeView {
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
                c.x = 39;
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
    // Таблица и дерево — во втором столбце.
    let mut t = f.new_control(Kind::Table, 42, 2);
    t.w = 32;
    t.editable = true;
    t.row_numbers = true;
    t.value = 1;
    t.items.push("Город".into());
    t.text.push_str("\nГлеб|456-78-90|Тула");
    f.controls.push(t);
    let mut tr = f.new_control(Kind::TreeView, 42, 9);
    tr.checks = true;
    tr.h = 10;
    tr.items.push("    Лето".into());
    f.controls.push(tr);
    let mut l = Control::new(Kind::Label, "input_label", 2, 4);
    l.text = "~С~трока:".into();
    l.link = "input1".into();
    f.controls.push(l);
    f
}

/// Форма-окно (Window): немодальное, с номером, растягивается.
fn window_form() -> Form {
    let mut f = Form::new("notes", "Заметки");
    f.class = FormClass::Window;
    f.number = 2;
    f.palette = Pal::Cyan;
    f.help_ctx = 5;
    f.w = 40;
    f.h = 12;
    let mut i = f.new_control(Kind::InputLine, 2, 2);
    i.w = 30;
    f.controls.push(i);
    let mut m = f.new_control(Kind::Memo, 2, 4);
    m.w = 34;
    m.h = 3;
    f.controls.push(m);
    let mut b = f.new_control(Kind::Button, 14, 8);
    b.text = "~З~акрыть".into();
    b.command = "cm::CLOSE".into();
    f.controls.push(b);
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
    p.app
        .commands
        .push(CommandDef { name: "NOTES".into(), value: 103, action: Action::Form("notes".into()), comment: "окно заметок".into() });
    p.app.startup_form = "about".into();
    p.app.menu.push(MenuNode::Sub {
        title: "~Т~ест".into(),
        key: "Alt+T".into(),
        help: 4,
        children: vec![
            MenuNode::Item { title: "~В~се элементы".into(), command: "ALL".into(), key: "Ctrl+F2".into(), help: 3 },
            MenuNode::Item { title: "~З~аметки".into(), command: "NOTES".into(), key: "".into(), help: 0 },
            MenuNode::Sub {
                title: "~В~ложенное".into(),
                key: String::new(),
                help: 0,
                children: vec![MenuNode::Item { title: "~П~ривет".into(), command: "HELLO".into(), key: "".into(), help: 0 }],
            },
        ],
    });
    p.app.status.push(StatusEntry { text: "~Привет~".into(), key: "".into(), command: "HELLO".into() });
    let forms = vec![about_form("gen_test"), big_form(), window_form()];
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
