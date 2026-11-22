use super::language_label;

#[test]
fn language_labels_fit_indicator() {
    assert_eq!(language_label("en-US").as_deref(), Some("ENG"));
    assert_eq!(language_label("ru_RU").as_deref(), Some("РУС"));
    assert_eq!(language_label("uk").as_deref(), Some("УКР"));
    assert_eq!(language_label("ja").as_deref(), Some("JA"));
    assert_eq!(language_label("und"), None);
}
