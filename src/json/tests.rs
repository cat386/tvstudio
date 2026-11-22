use super::*;

#[test]
fn roundtrip() {
    let v = Json::obj(vec![
        ("имя", Json::s("Окно \"1\"\nвторая")),
        ("n", Json::i(-42)),
        ("ok", Json::Bool(true)),
        ("a", Json::Arr(vec![Json::i(1), Json::s("x")])),
        ("o", Json::obj(vec![("z", Json::Null)])),
    ]);
    let t = v.to_pretty();
    assert_eq!(parse(&t).unwrap(), v);
    assert_eq!(v.str("имя"), "Окно \"1\"\nвторая");
    assert_eq!(v.int("n", 0), -42);
}

#[test]
fn errors_have_line() {
    let e = parse("{\n\"a\": }").unwrap_err();
    assert!(e.contains("строка 2"), "{}", e);
}
