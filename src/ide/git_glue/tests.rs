use super::*;
use std::process::Command;
use tvision::platform::HeadlessBackend;

#[test]
fn clone_lookup_cancel_and_failure_keep_welcome_navigation() {
    for fail in [false, true] {
        let mut app = Program::new(Box::new(HeadlessBackend::new(80, 25)), Some(menu_bar()), Some(status_line()));
        let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
        ide.startup(&mut app, vec![]);
        let welcome = ide.start_page.unwrap();
        let dir = std::env::temp_dir();
        ide.start_git(&mut app, dir.clone(), "Список веток", move |_| {
            if fail {
                Err("Тестовая ошибка сети".into())
            } else {
                Ok(Outcome::CloneBranches {
                    destination: dir.join("test-repo"),
                    dir,
                    url: "https://example.invalid/repo.git".into(),
                    default: Some("main".into()),
                    branches: vec!["main".into()],
                })
            }
        });
        app.put_event(Event::KeyDown(kb::key(Key::Esc)));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while ide.git.pending.is_some() {
            ide.poll_git(&mut app);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(ide.start_page, Some(welcome));
        assert!(app.desktop().group.view(welcome).is_some());
        app.redraw();
        assert!(app.screen_text().contains("Новый проект"));
        assert!(app.screen_text().contains("Недавние проекты"));
    }
}

#[test]
fn branch_picker_reloads_editor_and_protects_unsaved_buffer() {
    let dir = std::env::temp_dir().join(format!("tvstudio-git-branch-ui-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let project = Project::create_rust(&dir, "demo").unwrap();
    let source = project.dir.join("src/main.rs");
    let git = Git { executable: "git", cwd: &project.dir };
    git.init("main").unwrap();
    git.identity(true, "TVStudio Test", "tvstudio@example.invalid").unwrap();
    assert!(Command::new("git").current_dir(&project.dir).args(["config", "--local", "commit.gpgsign", "false"]).status().unwrap().success());
    git.stage(Path::new("src/main.rs")).unwrap();
    git.commit("main").unwrap();
    git.switch_branch(Switch::Create("topic")).unwrap();
    std::fs::write(&source, "fn main() { println!(\"On topic\"); }\n").unwrap();
    git.stage(Path::new("src/main.rs")).unwrap();
    git.commit("topic").unwrap();
    git.switch_branch(Switch::Local("main")).unwrap();

    let mut app = Program::new(Box::new(HeadlessBackend::new(100, 30)), Some(menu_bar()), Some(status_line()));
    let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
    ide.startup(&mut app, vec![project.file().display().to_string()]);
    let id = ide.open_file(&mut app, &source, None).unwrap();
    app.find_mut::<CodeWindow>(id).unwrap().editor_mut().fe.editor.insert_text("unsaved", false);

    let choose_topic = |app: &mut Program| {
        app.put_event(Event::KeyDown(kb::key(Key::Down)));
        app.put_event(Event::KeyDown(kb::ENTER));
    };
    choose_topic(&mut app);
    ide.git_choose_branch(&mut app, project.dir.clone(), git.branches().unwrap());
    assert_eq!(git.snapshot().unwrap().branch, "main");
    let editor = &mut app.find_mut::<CodeWindow>(id).unwrap().editor_mut().fe;
    assert!(editor.editor.text().contains("unsaved"));
    assert!(editor.load_file());

    choose_topic(&mut app);
    ide.git_choose_branch(&mut app, project.dir.clone(), git.branches().unwrap());
    assert_eq!(git.snapshot().unwrap().branch, "topic");
    assert!(app.find::<CodeWindow>(id).is_none());
    let open = Ide::windows::<CodeWindow>(&app);
    let editor = open.into_iter().map(|id| app.find::<CodeWindow>(id).unwrap()).find(|w| same_file(&w.path(), &source)).unwrap();
    assert!(editor.editor().fe.editor.text().contains("On topic"));
    let _ = std::fs::remove_dir_all(dir);
}
