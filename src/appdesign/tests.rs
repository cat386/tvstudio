use super::*;

#[test]
fn flatten_roundtrip() {
    let app = AppModel::starter("x");
    let mut rows = Vec::new();
    flatten(&app.menu, 0, &mut rows);
    assert_eq!(rows.len(), 2 + 3 + 3);
    assert_eq!(unflatten(&rows), app.menu);
    // Лишняя глубина у пункта без подменю выравнивается.
    rows[2].depth = 5;
    let m = unflatten(&rows);
    assert_eq!(m.len(), 2);
}

#[test]
fn apply_creates_commands() {
    let app = AppModel::starter("x");
    let mut e = ListEditor::new(LeKind::Menu, &app, &["about".into()]);
    let data = Value::List(vec![Value::Str("~Н~овый".into()), Value::Str("new_doc".into()), Value::Str("Ctrl+N".into()), Value::Str("0".into())]);
    e.apply(Pending { index: 0, new: true, sub: false }, &data).unwrap();
    assert!(e.app.commands.iter().any(|c| c.name == "NEW_DOC"));
    assert_eq!(e.rows[1].command, "NEW_DOC");
    assert_eq!(e.rows[1].depth, 1);
    let bad = Value::List(vec![Value::Str("x".into()), Value::Str("cm::QUIT".into()), Value::Str("Hyper+Q".into())]);
    assert!(e.apply(Pending { index: 0, new: true, sub: false }, &bad).is_err());
}

#[test]
fn invalid_item_keeps_all_fields_and_cancel_does_not_change_model() {
    for kind in [LeKind::Menu, LeKind::Status, LeKind::Commands] {
        let editor = ListEditor::new(kind, &AppModel::starter("audit"), &[]);
        let before = editor.app.clone();
        let mut dialog = editor.edit_dialog(Pending { index: 0, new: true, sub: false });
        let mut data = dialog.get_data();
        if let Value::List(values) = &mut data {
            values[0] = Value::Str("My entered title".into());
            // Имя команды с пробелами / пустая команда у пункта и статуса.
        }
        dialog.set_data(&data);
        let mut app = Program::new(Box::new(tvision::platform::HeadlessBackend::new(80, 25)), None, None);
        app.put_event(Event::command_event(cm::OK));
        app.put_event(Event::command_event(cm::CANCEL));
        let (result, dialog) = crate::dialogs::exec(&mut app, dialog);
        assert_eq!(result, cm::CANCEL, "ошибка не должна закрывать диалог");
        assert_eq!(dialog.get_data(), data);
        assert_eq!(editor.app, before);
    }
}

#[test]
fn invalid_menu_item_can_be_corrected_without_retyping_title() {
    let mut editor = ListEditor::new(LeKind::Menu, &AppModel::starter("audit"), &[]);
    let pending = Pending { index: 0, new: true, sub: false };
    let mut dialog = editor.edit_dialog(pending);
    dialog.set_data(&Value::List(vec![
        Value::Str("Keep this title".into()),
        Value::Str(String::new()),
        Value::Str("Ctrl+H".into()),
        Value::Str("0".into()),
    ]));
    let mut app = Program::new(Box::new(tvision::platform::HeadlessBackend::new(80, 25)), None, None);
    app.put_event(Event::command_event(cm::OK));
    app.put_event(Event::KeyDown(kb::TAB));
    for ch in "hello".chars() {
        app.put_event(Event::KeyDown(KeyEvent::new(Key::Char(ch), Mods::NONE)));
    }
    app.put_event(Event::command_event(cm::OK));
    let (result, dialog) = crate::dialogs::exec(&mut app, dialog);
    assert_eq!(result, cm::OK);
    editor.apply(pending, &dialog.get_data()).unwrap();
    assert_eq!(editor.rows[1].title, "Keep this title");
    assert_eq!(editor.rows[1].command, "HELLO");
    assert_eq!(editor.app.commands.iter().filter(|c| c.name == "HELLO").count(), 1);
}

#[test]
fn invalid_shortcut_does_not_create_command() {
    let mut editor = ListEditor::new(LeKind::Menu, &AppModel::starter("audit"), &[]);
    let before = editor.app.clone();
    let data = Value::List(vec![Value::Str("Title".into()), Value::Str("new_command".into()), Value::Str("Hyper+X".into()), Value::Str("0".into())]);
    assert!(editor.apply(Pending { index: 0, new: true, sub: false }, &data).is_err());
    assert_eq!(editor.app, before);
}

#[test]
fn command_action_fields_hide_without_losing_values() {
    let editor = ListEditor::new(LeKind::Commands, &AppModel::starter("audit"), &["about".into()]);
    let mut dialog = editor.edit_dialog(Pending { index: 0, new: true, sub: false });
    for action in [0, 1, 2, 1] {
        let data = Value::List(vec![
            Value::Str("TEST".into()),
            Value::Str("150".into()),
            Value::U32(action),
            Value::Str("about".into()),
            Value::Str("Hello".into()),
            Value::Str("Comment".into()),
        ]);
        dialog.set_data(&data);
        assert_eq!(dialog.get_data(), data);
        for child in &dialog.dialog.win.group.children {
            if let Some(input) = child.as_any().downcast_ref::<InputLine>() {
                match input.text().as_str() {
                    "about" => assert_eq!(input.vs.state & sf::VISIBLE != 0, action == 1),
                    "Hello" => assert_eq!(input.vs.state & sf::VISIBLE != 0, action == 2),
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn list_editor_buttons_have_unique_mnemonics() {
    for kind in [LeKind::Menu, LeKind::Status, LeKind::Commands] {
        let editor = ListEditor::new(kind, &AppModel::starter("audit"), &[]);
        let mut used = std::collections::HashSet::new();
        for child in &editor.dlg.win.group.children {
            if let Some(button) = child.as_any().downcast_ref::<Button>() {
                if let Some(key) = tvision::text::hotkey(&button.title) {
                    assert!(used.insert(key.to_lowercase().collect::<String>()), "{}", button.title);
                }
            }
        }
    }
}
