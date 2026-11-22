use super::*;

#[test]
fn keys() {
    assert_eq!(parse_key("Alt+X"), Some(KeyEvent::new(Key::Char('X'), Mods::ALT)));
    assert_eq!(parse_key("F10"), Some(KeyEvent::new(Key::F(10), Mods::NONE)));
    assert_eq!(parse_key("Ctrl-F5"), Some(KeyEvent::new(Key::F(5), Mods::CTRL)));
    assert_eq!(parse_key("Shift+Del"), Some(KeyEvent::new(Key::Del, Mods::SHIFT)));
    assert_eq!(parse_key("Hyper+Q"), None);
    assert_eq!(key_expr(&parse_key("alt+x").unwrap()), "KeyEvent::new(Key::Char('X'), Mods::ALT)");
}

/// Проект на Rust: только Cargo.toml, main.rs и файл проекта; тип
/// сохраняется, а старые файлы проекта без поля `kind` — tvision-rs.
#[test]
fn rust_project() {
    let base = std::env::temp_dir().join(format!("tvs_rustproj_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let p = Project::create_rust(&base, "plain").unwrap();
    assert!(!p.has_forms());
    let cargo = std::fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"plain\"") && !cargo.contains("tvision"), "{}", cargo);
    assert!(std::fs::read_to_string(p.dir.join("src/main.rs")).unwrap().contains("Hello, world!"));
    assert!(!p.dir.join("forms").exists() && !p.dir.join("src/forms").exists());
    let q = Project::load(&p.file()).unwrap();
    assert_eq!((q.kind, q.forms.len()), (ProjectKind::Rust, 0));
    assert!(Project::create_rust(&base, "plain").is_err(), "второй раз в тот же каталог");
    // Старый файл проекта (без kind) — проект tvision-rs.
    let text = std::fs::read_to_string(p.file()).unwrap();
    assert!(text.starts_with("format: tvstudio-project\nversion: "), "файл проекта — YAML:\n{}", text);
    let mut j = crate::yaml::parse_document(&text).unwrap();
    if let Json::Obj(v) = &mut j {
        v.retain(|(k, _)| k != "kind");
    }
    // …и записан в старых форматах — TOML и JSON — тоже читается.
    std::fs::write(p.file(), crate::toml::to_string(&j).unwrap()).unwrap();
    assert_eq!(Project::load(&p.file()).unwrap().kind, ProjectKind::Tvision);
    std::fs::write(p.file(), j.to_pretty()).unwrap();
    assert_eq!(Project::load(&p.file()).unwrap().kind, ProjectKind::Tvision);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn build_options_args() {
    let d = BuildOptions::default();
    assert!(d.cargo_args().is_empty() && d.is_default());
    assert_eq!(d.profile_title(), "Debug");
    let o = BuildOptions {
        profile: "release".into(),
        features: "serde, gui  log".into(),
        all_features: false,
        no_default_features: true,
        target: "x86_64-unknown-linux-musl".into(),
        package: "app".into(),
        extra: "--locked -j 4".into(),
        run_args: "--file \"мой файл.txt\" -v".into(),
    };
    assert_eq!(
        o.cargo_args(),
        [
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
            "-p",
            "app",
            "--features",
            "serde,gui,log",
            "--no-default-features",
            "--locked",
            "-j",
            "4"
        ]
    );
    assert_eq!(o.profile_args(), ["--release", "--target", "x86_64-unknown-linux-musl", "-p", "app"]);
    assert_eq!(o.program_args(), ["--file", "мой файл.txt", "-v"]);
    assert_eq!(o.profile_title(), "Release");
    let c = BuildOptions { profile: "bench".into(), ..Default::default() };
    assert_eq!((c.cargo_args(), c.profile_title()), (vec!["--profile".to_string(), "bench".into()], "bench".to_string()));
    assert_eq!(BuildOptions::from_json(&o.to_json()), o);
    assert_eq!(split_args(r#"a 'b c' "d \"e\"" "" f"#), ["a", "b c", "d \"e\"", "", "f"]);
}

/// Параметры сборки сохраняются в файле проекта (раздел build — только если задан).
#[test]
fn build_options_saved() {
    let base = std::env::temp_dir().join(format!("tvs_buildopts_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mut p = Project::create_rust(&base, "opts").unwrap();
    assert!(!std::fs::read_to_string(p.file()).unwrap().contains("build:"));
    p.build.profile = "release".into();
    p.build.run_args = "-x".into();
    p.save().unwrap();
    let text = std::fs::read_to_string(p.file()).unwrap();
    assert!(text.contains("build:\n  profile: release\n  run_args: '-x'\n"), "{}", text);
    assert_eq!(Project::load(&p.file()).unwrap().build, p.build);
    let _ = std::fs::remove_dir_all(&base);
}

/// Пути в Cargo.toml: без приставки `\\?\` и с прямыми косыми.
#[test]
fn cargo_paths() {
    assert_eq!(cargo_path(r"\\?\D:\a\tvision-rs"), "D:/a/tvision-rs");
    assert_eq!(cargo_path(r"\\?\UNC\srv\share\tv"), "//srv/share/tv");
    assert_eq!(cargo_path(r"D:\develop\tvision-rs"), "D:/develop/tvision-rs");
    assert_eq!(cargo_path("../tvision-rs"), "../tvision-rs");
    assert_eq!(Dependency::Path(r"\\?\C:\x".into()).cargo_line(), "tvision-rs = { path = \"C:/x\" }");
}

/// Workspace: участники по списку и шаблонам, без exclude; имена — из их
/// Cargo.toml; свой пакет в корне — по [package].
#[test]
fn workspace_layout() {
    let base = std::env::temp_dir().join(format!("tvs_ws_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mk = |rel: &str, name: &str| {
        let d = base.join(rel);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("Cargo.toml"), format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n", name)).unwrap();
        std::fs::write(d.join("src/lib.rs"), "").unwrap();
    };
    mk("crates/core", "my-core");
    mk("crates/ui", "my-ui");
    mk("crates/old", "my-old");
    mk("tools/gen", "gen");
    std::fs::create_dir_all(base.join("crates/not-a-crate")).unwrap();
    std::fs::write(
            base.join("Cargo.toml"),
            "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\", \"tools/gen\", \"tools/gen\", \"нет-такого\"]\nexclude = [\"crates/old\"]\n\n[workspace.dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\n",
        )
        .unwrap();
    let l = cargo_layout(&base);
    assert!(!l.package);
    let names: Vec<&str> = l.members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["my-core", "my-ui", "gen"]);
    assert_eq!(l.members[0].dir, normalize(&base.join("crates/core")));
    // Проект на Rust из workspace: открыт lib.rs первого участника.
    let p = Project::adopt_rust(&base).unwrap();
    assert_eq!(p.open_files, vec!["crates/core/src/lib.rs".to_string()]);
    // Пакет с workspace в корне (members = ["."] и подкаталог).
    std::fs::write(base.join("Cargo.toml"), "[package]\nname = \"root\"\n\n[workspace]\nmembers = [\".\", \"tools/*\"]\n").unwrap();
    let l = cargo_layout(&base);
    assert!(l.package);
    assert_eq!(l.members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["gen"]);
    assert!(wildcard("x-*", "x-gen") && wildcard("a?c", "abc") && !wildcard("x-*", "y-gen") && wildcard("*", "any"));
    // Ctrl+F9: участник открытого файла, если у него есть программа.
    std::fs::write(base.join("tools/gen/src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(base.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\", \"tools/*\"]\n").unwrap();
    let l = cargo_layout(&base);
    assert_eq!(l.run_package(Some(&base.join("tools/gen/src/main.rs"))).as_deref(), Some("gen"));
    assert_eq!(l.run_package(Some(&base.join("crates/core/src/lib.rs"))).as_deref(), Some("gen"), "у core нет программы — первая с программой");
    assert_eq!(l.run_package(None).as_deref(), Some("gen"));
    assert_eq!(CargoLayout::default().run_package(None), None);
    let with_root = CargoLayout { package: true, ..l };
    assert_eq!(with_root.run_package(Some(&base.join("crates/core/src/lib.rs"))), None, "корневой пакет — cargo сам");
    let _ = std::fs::remove_dir_all(&base);
}

/// Каталог с Cargo.toml берётся в проект на Rust; имя — из [package].
#[test]
fn adopt_cargo_dir() {
    assert_eq!(cargo_package_name("[package]\nname = \"my-tool\" # имя\n[dependencies]\nname = \"x\"\n").as_deref(), Some("my-tool"));
    assert_eq!(cargo_package_name("[dependencies]\nname = \"x\"\n[package]\nversion = \"1\"\nname='q'\n").as_deref(), Some("q"));
    assert_eq!(cargo_package_name("[workspace]\nmembers = [\"a\"]\n"), None);
    assert_eq!(cargo_package_name("[package]\nname.workspace = true\n"), None);

    let base = std::env::temp_dir().join(format!("tvs_adopt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("tool dir");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    assert!(Project::adopt_rust(&dir).unwrap_err().contains("нет Cargo.toml"));
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"my-tool\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
    let p = Project::adopt_rust(&dir).unwrap();
    assert_eq!((p.name.as_str(), p.kind), ("my-tool", ProjectKind::Rust));
    assert_eq!(p.file(), dir.join("my-tool.tvsp"));
    assert_eq!(p.open_files, vec!["src/lib.rs".to_string()]);
    assert_eq!(std::fs::read_to_string(dir.join("src/lib.rs")).unwrap(), "pub fn f() {}\n", "исходники не тронуты");
    assert!(Project::adopt_rust(&dir).is_err(), "файл проекта уже есть");
    // Workspace без [package] — имя каталога.
    let ws = base.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    let p = Project::adopt_rust(&ws).unwrap();
    assert_eq!((p.name.as_str(), p.open_files.len()), ("ws", 0));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn form_roundtrip() {
    let mut f = about_form("Тест");
    let mut l = f.new_control(Kind::Label, 2, 2);
    l.link = "info".into();
    f.controls.push(l);
    let mut ln = f.new_control(Kind::Line, 1, 6);
    (ln.vertical, ln.double, ln.to_frame) = (true, true, true);
    f.controls.push(ln);
    let j = crate::yaml::to_string(&f.to_json());
    let g = Form::from_json(&crate::yaml::parse(&j).unwrap()).unwrap();
    assert_eq!(f, g);
    assert_eq!(f.unique_name("label"), "label2");
}

#[test]
fn app_roundtrip() {
    let a = AppModel::starter("x");
    let doc = Json::obj(vec![("app", a.to_json())]);
    let b = AppModel::from_json(crate::yaml::parse(&crate::yaml::to_string(&doc)).unwrap().get("app").unwrap());
    assert_eq!(a, b);
    assert_eq!(a.command_value("ABOUT"), Some(100));
    assert_eq!(a.command_value("cm::QUIT"), Some(1));
    assert_eq!(a.command_expr("ABOUT"), "cmd::ABOUT");
    assert_eq!(a.next_command_value(), 101);
}

#[test]
fn dependency_detect() {
    assert_eq!(Dependency::detect(" https://github.com/cat386/tvision-rs "), Some(Dependency::Git("https://github.com/cat386/tvision-rs".into())));
    assert_eq!(Dependency::detect("git@github.com:cat386/tvision-rs.git"), Some(Dependency::Git("git@github.com:cat386/tvision-rs.git".into())));
    assert_eq!(Dependency::detect("D:\\develop\\tvision-rs"), Some(Dependency::Path("D:\\develop\\tvision-rs".into())));
    assert_eq!(Dependency::detect(""), None);
    // Несуществующий путь — понятная ошибка, а не сломанный Cargo.toml.
    let base = std::env::temp_dir().join(format!("tvs_dep_{}", std::process::id()));
    let e = Project::create(&base, "x", &Dependency::Path("../нет-такой".into())).unwrap_err();
    assert!(e.contains("нет tvision-rs"), "{}", e);
    assert!(!base.join("x").exists());
}

#[test]
fn names() {
    assert!(is_ident("ok_button"));
    assert!(!is_ident("1x"));
    assert!(!is_ident("fn"));
    assert_eq!(camel("about_dialog"), "AboutDialog");
}
