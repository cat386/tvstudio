use super::*;
use crate::json::parse_json;

#[test]
fn array_of_objects() {
    let j =
        parse_json(r#"[{"id": 1, "имя": "Аня", "адрес": {"город": "Тула"}}, {"id": 2, "имя": "Борис", "тэги": ["a", "b"], "ok": true}]"#).unwrap();
    let t = to_table(&j);
    assert_eq!(t.headers, vec!["id", "имя", "адрес.город", "тэги", "ok"]);
    assert_eq!(t.rows[0], vec!["1", "Аня", "Тула", "", ""]);
    assert_eq!(t.rows[1], vec!["2", "Борис", "", "a, b", "true"]);
    assert_eq!(t.kinds, vec![ColKind::Number, ColKind::Text, ColKind::Text, ColKind::Text, ColKind::Bool]);
}

#[test]
fn other_shapes() {
    // Объект с массивом внутри.
    let t = to_table(&parse_json(r#"{"total": 2, "items": [{"a": 1}, {"a": 2}]}"#).unwrap());
    assert_eq!((t.source.as_str(), t.rows.len()), ("items", 2));
    // Объект объектов: столбец «ключ».
    let t = to_table(&parse_json(r#"{"x": {"v": 1}, "y": {"v": 2}}"#).unwrap());
    assert_eq!(t.headers, vec!["ключ", "v"]);
    assert_eq!(t.rows[1], vec!["y", "2"]);
    // Массив массивов.
    let t = to_table(&parse_json("[[1, 2], [3, 4, 5]]").unwrap());
    assert_eq!(t.headers, vec!["A", "B", "C"]);
    assert_eq!(t.rows[0], vec!["1", "2", ""]);
    // Массив значений и смешанный тип столбца.
    let t = to_table(&parse_json(r#"[1, "два", null]"#).unwrap());
    assert_eq!(t.headers, vec!["значение"]);
    assert_eq!(t.kinds, vec![ColKind::Text]);
    assert_eq!(t.rows[2], vec!["null"]);
}
