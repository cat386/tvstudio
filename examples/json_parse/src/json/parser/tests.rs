use super::*;

fn s(x: &str) -> Json {
    Json::Str(x.into())
}
fn n(x: &str) -> Json {
    Json::Num(x.into())
}

#[test]
fn parse_values() {
    assert_eq!(parse_json(" null ").unwrap(), Json::Null);
    assert_eq!(parse_json("[true, false, -1.5e3, 0]").unwrap(), Json::Arr(vec![Json::Bool(true), Json::Bool(false), n("-1.5e3"), n("0")]));
    assert_eq!(
        parse_json(r#"{"b": 1, "a": {"x": []}}"#).unwrap(),
        Json::Obj(vec![("b".into(), n("1")), ("a".into(), Json::Obj(vec![("x".into(), Json::Arr(vec![]))]))])
    );
    // Экранирование и суррогатные пары.
    assert_eq!(parse_json(r#""\"Привет\"\nЖ 😀""#).unwrap(), s("\"Привет\"\nЖ 😀"));
    assert_eq!(parse_json("\u{feff}[1]").unwrap(), Json::Arr(vec![n("1")]));
}

#[test]
fn parse_errors_with_position() {
    let e = parse_json("[1, 2,\n  ]").unwrap_err();
    assert_eq!((e.line, e.col), (2, 3), "{}", e);
    let e = parse_json(r#"{"a" 1}"#).unwrap_err();
    assert!(e.msg.contains("«:»"), "{}", e);
    assert!(parse_json("[1] x").is_err());
    assert!(parse_json(r#""abc"#).is_err());
    assert!(parse_json("01x").is_err());
    assert!(parse_json(&"[".repeat(1000)).unwrap_err().msg.contains("вложенность"));
}

#[test]
fn roundtrip_compact() {
    let src = r#"{"a": [1, {"b": "x\ty"}], "c": null}"#;
    let j = parse_json(src).unwrap();
    assert_eq!(j.compact(), r#"{"a": [1, {"b": "x\ty"}], "c": null}"#);
    assert_eq!(parse_json(&j.pretty()).unwrap(), j);
}
