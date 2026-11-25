use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn clone_dialog_keeps_url_while_browsing() {
    let base = std::env::temp_dir();
    let url = "https://example.test/team/project.git";
    let mut backend = HeadlessBackend::new(100, 30);
    for c in url.chars() {
        backend.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
    }
    backend.push_key(kb::key(Key::Tab));
    backend.push_key(kb::key(Key::Tab));
    backend.push_key(kb::key(Key::Char(' '))); // кнопка «…»
    backend.push_key(kb::ENTER); // Выбрать этот каталог
    backend.push_key(kb::ENTER); // Клонировать
    let mut app = Program::new(Box::new(backend), None, None);
    assert_eq!(clone_dialog(&mut app, &base), Some(CloneRequest { url: url.into(), parent: base, branch: CloneBranch::Direct(None) }));
}

#[test]
fn git_actions_fit_above_messages_and_empty_status_is_explicit() {
    for (width, height) in [(80, 25), (100, 30), (120, 40)] {
        let mut app = Program::new(Box::new(HeadlessBackend::new(width, height)), Some(crate::ide::menu_bar()), Some(crate::ide::status_line()));
        let desktop = app.desktop().group.vs.size;
        let top = desktop.y - ((desktop.y / 4).clamp(5, 9) - 2);
        let messages = crate::dialogs::insert(&mut app, crate::toolwins::MessagesWindow::new(Rect::new(0, top, width, desktop.y)));
        let mut panel = Panel::open(&mut app);
        let window = app.find::<Window>(panel.window).unwrap();
        assert!(window.group.vs.origin.y + window.group.vs.size.y <= top);
        panel.set(&mut app, Snapshot { root: PathBuf::new(), branch: "main".into(), upstream: String::new(), changes: vec![], initial: false });
        app.focus(messages); // Даже поверх активной панели окно сообщений не закрывает кнопки.
        app.redraw();
        let text = app.screen_text();
        for label in ["Нет изменений", "В индекс", "Из индекса", "Коммит", "Ветка"] {
            assert!(text.contains(label), "{label}: {text}");
        }
        assert!(panel.selected(&app).is_none());
    }
}

#[test]
fn git_status_labels_distinguish_staged_worktree_and_conflicts() {
    let mut change = Change { path: "src/main.rs".into(), index: '.', worktree: 'M', unmerged: false };
    assert_eq!(change_label(&change), "Изменён");
    change.index = 'M';
    assert_eq!(change_label(&change), "Индекс + правки");
    change.worktree = '.';
    assert_eq!(change_label(&change), "В индексе");
    change.unmerged = true;
    assert_eq!(change_label(&change), "Конфликт");
}
