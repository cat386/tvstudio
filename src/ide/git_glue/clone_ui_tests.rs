use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn clone_command_opens_real_cargo_project_and_editor() {
    use std::process::Command;
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(format!("tvstudio-clone-open-{}", std::process::id()));
    let fixture = root.join("fixture");
    let destination = root.join("projects");
    let bundle = root.join("cloned_app.bundle");
    std::fs::create_dir_all(fixture.join("src")).unwrap();
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(fixture.join("Cargo.toml"), "[package]\nname = \"cloned_app\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(fixture.join("src/main.rs"), "fn main() {}\n").unwrap();
    let git = Git { executable: "git", cwd: &fixture };
    git.init("main").unwrap();
    git.identity(true, "TVStudio Test", "tvstudio@example.invalid").unwrap();
    assert!(Command::new("git").current_dir(&fixture).args(["config", "--local", "commit.gpgsign", "false"]).status().unwrap().success());
    git.stage(Path::new("Cargo.toml")).unwrap();
    git.stage(Path::new("src/main.rs")).unwrap();
    git.commit("Fixture").unwrap();
    let output =
        Command::new("git").current_dir(&fixture).args(["bundle", "create"]).arg(&bundle).args(["HEAD", "refs/heads/main"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let mut app = Program::new(Box::new(HeadlessBackend::new(100, 30)), Some(menu_bar()), Some(status_line()));
    let cfg = Config { lsp_enabled: false, recent: vec![destination.join("previous/demo.tvsp").to_string_lossy().into_owned()], ..Config::default() };
    let mut ide = Ide::new(cfg, true);
    for ch in bundle.to_str().unwrap().chars() {
        app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(ch), Mods::NONE)));
    }
    app.put_event(Event::KeyDown(kb::ENTER));
    assert!(ide.command(&mut app, CM_GIT_CLONE, &Info::None));
    assert!(ide.git.pending.is_some());

    let start = Instant::now();
    while ide.git.pending.is_some() {
        ide.poll_git(&mut app);
        assert!(start.elapsed() < Duration::from_secs(8), "{:?}", ide.last_error);
        std::thread::sleep(Duration::from_millis(10));
    }
    let target = destination.join("cloned_app");
    let source = target.join("src/main.rs");
    let project = ide.project.as_ref().unwrap();
    assert!(same_file(&project.dir, &target));
    assert_eq!(project.name, "cloned_app");
    assert!(project.file().is_file());
    assert!(Ide::windows::<CodeWindow>(&app).iter().any(|&id| same_file(&app.find::<CodeWindow>(id).unwrap().path(), &source)));
    let _ = std::fs::remove_dir_all(root);
}
