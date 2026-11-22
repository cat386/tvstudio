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
