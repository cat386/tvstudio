use std::{fs, path::PathBuf, process::Command, time::Duration};
use tvision::{platform::HeadlessBackend, *};
use tvstudio::{
    cmds::*,
    dialogs::Config,
    git::{Git, Switch},
    ide::{menu_bar, status_line, Ide},
    model::Project,
};

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("tvstudio-git-{name}-{}", std::process::id()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_git_first_commit_diff_push_and_clone() {
    let temp = Temp::new("remote");
    let project = temp.0.join("project");
    let remote = temp.0.join("origin.git");
    let clone = temp.0.join("clone");
    fs::create_dir(&project).unwrap();
    assert!(Command::new("git").arg("init").arg("--bare").arg(&remote).output().unwrap().status.success());
    let git = Git { executable: "git", cwd: &project };
    let initial = git.init("main").unwrap();
    assert_eq!(initial.branch, "main");
    assert!(initial.initial);
    assert!(git.init("other").is_err());
    git.identity(true, "TVStudio Test", "tvstudio@example.invalid").unwrap();
    for (key, value) in [("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        assert!(Command::new("git").current_dir(&project).args(["config", "--local", key, value]).status().unwrap().success());
    }
    let name = PathBuf::from("с пробелом.txt");
    fs::write(project.join(&name), "first\n").unwrap();
    let status = git.snapshot().unwrap();
    let change = &status.changes[0];
    assert_eq!(change.path, name);
    assert_eq!(change.index, '?');
    assert!(git.diff(change, false).unwrap().contains("first"));
    git.stage(&name).unwrap();
    assert_eq!(git.snapshot().unwrap().changes[0].index, 'A');
    git.unstage(&name, true).unwrap();
    assert_eq!(git.snapshot().unwrap().changes[0].index, '?');
    git.stage(&name).unwrap();
    git.commit("Первый коммит").unwrap();
    fs::write(project.join(&name), "second\n").unwrap();
    git.stage(&name).unwrap();
    fs::write(project.join(&name), "third\n").unwrap();
    let status = git.snapshot().unwrap();
    let change = &status.changes[0];
    assert_eq!((change.index, change.worktree), ('M', 'M'));
    assert!(git.diff(change, true).unwrap().contains("+second"));
    assert!(git.diff(change, false).unwrap().contains("+third"));
    git.unstage(&name, false).unwrap();
    assert_eq!(git.snapshot().unwrap().changes[0].index, '.');
    assert_eq!(fs::read_to_string(project.join(&name)).unwrap(), "third\n");

    // In restricted Windows environments, local Git transport may be unable
    // to spawn its shell. A test daemon can serve the same temporary repos.
    let url = match std::env::var("TVSTUDIO_TEST_GIT_BASE_URL") {
        Ok(base) => format!("{}/{}/origin.git", base.trim_end_matches('/'), temp.0.file_name().unwrap().to_string_lossy()),
        Err(_) => remote.to_string_lossy().into_owned(),
    };
    git.add_origin(&url).unwrap();
    git.push().unwrap();
    git.fetch().unwrap();
    assert_eq!(git.snapshot().unwrap().upstream, "origin/main");
    assert!(Command::new("git").current_dir(&remote).args(["symbolic-ref", "HEAD", "refs/heads/main"]).status().unwrap().success());
    let cloned = git.clone_into(&url, &clone, None).unwrap();
    assert!(cloned.changes.is_empty());
    assert!(fs::read_to_string(clone.join(&name)).unwrap().lines().eq(["first"]));

    let sub = clone.join("sub");
    fs::create_dir(&sub).unwrap();
    assert_eq!(Git { executable: "git", cwd: &sub }.snapshot().unwrap().root, cloned.root);

    git.switch_branch(Switch::Create("feature/next")).unwrap();
    let new_path = PathBuf::from("feature.txt");
    fs::write(project.join(&new_path), "feature\n").unwrap();
    git.stage(&new_path).unwrap();
    git.commit("Feature branch").unwrap();
    git.push().unwrap();
    let (default, branches) = git.remote_branches(&url).unwrap();
    assert_eq!(default.as_deref(), Some("main"));
    assert!(branches.iter().any(|branch| branch == "feature/next"));

    let selected_clone = temp.0.join("selected");
    let selected = git.clone_into(&url, &selected_clone, Some("feature/next")).unwrap();
    assert_eq!(selected.branch, "feature/next");
    assert!(selected_clone.join(&new_path).is_file());

    let receiver = Git { executable: "git", cwd: &clone };
    receiver.fetch().unwrap();
    let branches = receiver.branches().unwrap();
    assert!(branches.iter().any(|b| b.current && !b.remote && b.name == "main"));
    assert!(branches.iter().any(|b| b.remote && b.name == "origin/feature/next"));
    assert!(!branches.iter().any(|b| b.name == "origin/HEAD"));
    assert_eq!(receiver.switch_branch(Switch::Track("origin/feature/next")).unwrap().snapshot.branch, "feature/next");
    assert!(clone.join(&new_path).is_file());
    receiver.switch_branch(Switch::Local("main")).unwrap();
    assert!(!clone.join(&new_path).exists());

    git.switch_branch(Switch::Local("main")).unwrap();
    git.stage(&name).unwrap();
    git.commit("Advance main").unwrap();
    git.push().unwrap();
    receiver.fetch_upstream().unwrap();
    let updated = receiver.fast_forward().unwrap();
    assert_eq!(updated.changed.as_deref().unwrap(), std::slice::from_ref(&name));
    assert!(fs::read_to_string(clone.join(&name)).unwrap().lines().eq(["third"]));
    let history = receiver.history().unwrap();
    assert!(history[0].label.contains("Advance main"));
    assert!(receiver.show_commit(&history[0].oid).unwrap().contains("+third"));

    receiver.identity(true, "TVStudio Test", "tvstudio@example.invalid").unwrap();
    assert!(Command::new("git").current_dir(&clone).args(["config", "--local", "commit.gpgsign", "false"]).status().unwrap().success());
    let local_only = PathBuf::from("local.txt");
    fs::write(clone.join(&local_only), "local").unwrap();
    receiver.stage(&local_only).unwrap();
    receiver.commit("Local side").unwrap();

    let upstream_only = PathBuf::from("upstream.txt");
    fs::write(project.join(&upstream_only), "upstream").unwrap();
    git.stage(&upstream_only).unwrap();
    git.commit("Upstream side").unwrap();
    git.push().unwrap();
    receiver.fetch_upstream().unwrap();
    assert!(receiver.fast_forward().is_err());
    assert!(clone.join(&local_only).exists());
    assert!(!clone.join(&upstream_only).exists());
}

#[test]
fn git_panel_stages_a_file_through_the_ide() {
    let temp = Temp::new("ide");
    let project = Project::create_rust(&temp.0, "demo").unwrap();
    let git = Git { executable: "git", cwd: &project.dir };
    git.init("main").unwrap();
    let backend = HeadlessBackend::new(115, 35);
    let mut app = Program::new(Box::new(backend), Some(menu_bar()), Some(status_line()));
    let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
    ide.startup(&mut app, vec![project.file().display().to_string()]);
    app.put_event(Event::command_event(CM_GIT_SHOW));
    wait_screen(&mut app, &mut ide, "??   .gitignore");
    assert!(app.screen_text().contains("Git — main"));
    app.put_event(Event::command_event(CM_GIT_STAGE));
    wait_screen(&mut app, &mut ide, "A.   .gitignore");
    assert!(git.snapshot().unwrap().changes.iter().any(|entry| entry.index == 'A'));
    app.put_event(Event::command_event(CM_GIT_UNSTAGE));
    wait_screen(&mut app, &mut ide, "??   .gitignore");
    assert!(!git.snapshot().unwrap().changes.iter().any(|entry| entry.index == 'A'));
}

fn wait_screen(app: &mut Program, ide: &mut Ide, text: &str) {
    let start = std::time::Instant::now();
    loop {
        app.pump(ide);
        let screen = app.screen_text();
        if screen.contains(text) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(8), "Не найдено {text:?}, ошибка {:?}\n{screen}", ide.last_error);
        std::thread::sleep(Duration::from_millis(10));
    }
}
