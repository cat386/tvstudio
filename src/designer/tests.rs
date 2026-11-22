use super::*;
use crate::model::about_form;

#[test]
fn doc_edit_undo_paste() {
    let d = FormDoc::new(about_form("x"), PathBuf::from("/tmp/x.tvf"));
    let mut doc = d.borrow_mut();
    let i = doc.insert(Kind::InputLine);
    assert_eq!(doc.sel, vec![i]);
    assert!(doc.modified);
    let t = doc.copy_text().unwrap();
    assert!(doc.paste_text(&t));
    assert_eq!(doc.form.controls.len(), 4);
    assert_eq!(doc.form.controls[3].name, "input2");
    doc.undo();
    assert_eq!(doc.form.controls.len(), 3);
    doc.redo();
    assert_eq!(doc.form.controls.len(), 4);
    doc.select(vec![0, 1]);
    doc.align(Align::Left);
    assert_eq!(doc.form.controls[1].x, doc.form.controls[0].x);
    doc.select(vec![0]);
    doc.reorder(Reorder::Front);
    assert_eq!(doc.form.controls[3].name, "info");
    doc.select(vec![3]);
    doc.delete_selected();
    assert!(doc.form.find("info").is_none());
}
