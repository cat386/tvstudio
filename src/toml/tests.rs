use super::*;
use crate::json;

fn rt(j: &Json) -> Json {
    let t = to_string(j).unwrap();
    parse(&t).unwrap_or_else(|e| panic!("{}\n---\n{}", e, t))
}

#[test]
fn writes_tables_and_arrays_of_tables() {
    let j = json::parse(
        r#"{"format":"x","n":1,"f":0.5,"ok":true,"list":["a","b"],"empty":[],
                "app":{"title":"t","menu":[{"title":"F","items":[{"title":"Open","key":"F3"},{"separator":true},
                {"title":"Sub","items":[{"title":"deep"}]}]}]},
                "rows":[["a","b"],["c","d"]],"mixed":[1,{"k":"v"}],"text":"line1\nline2 \"q\" \\ \u0003end",
                "key with space":"v","after":{"k":1}}"#,
    )
    .unwrap();
    let t = to_string(&j).unwrap();
    assert!(t.starts_with("format = \"x\"\nn = 1\nf = 0.5\nok = true\nlist = [\"a\", \"b\"]\nempty = []\n"), "{}", t);
    assert!(t.contains("\n[app]\ntitle = \"t\"\n\n[[app.menu]]\ntitle = \"F\"\n\n[[app.menu.items]]\ntitle = \"Open\""), "{}", t);
    assert!(t.contains("[[app.menu.items.items]]\ntitle = \"deep\""), "{}", t);
    assert!(t.contains("rows = [\n  [\"a\", \"b\"],\n  [\"c\", \"d\"],\n]"), "{}", t);
    assert!(t.contains("mixed = [\n  1,\n  { k = \"v\" },\n]"), "{}", t);
    assert!(t.contains("text = \"\"\"\nline1\nline2 \\\"q\\\" \\\\ \\u0003end\"\"\""), "{}", t);
    assert!(t.contains("\"key with space\" = \"v\""), "{}", t);
    assert_eq!(rt(&j), j_sorted_like_output(&j));
}

/// Порядок после записи: простые ключи, затем таблицы (как пишет `to_string`).
fn j_sorted_like_output(j: &Json) -> Json {
    match j {
        Json::Obj(p) => {
            let mut simple = Vec::new();
            let mut tables = Vec::new();
            for (k, v) in p {
                if matches!(v, Json::Obj(_)) || is_table_array(v) {
                    tables.push((k.clone(), j_sorted_like_output(v)));
                } else {
                    simple.push((k.clone(), j_sorted_like_output(v)));
                }
            }
            simple.extend(tables);
            Json::Obj(simple)
        }
        Json::Arr(a) => Json::Arr(a.iter().map(j_sorted_like_output).collect()),
        v => v.clone(),
    }
}

#[test]
fn parses_toml_features() {
    let t = r#"
# комментарий
title = 'буквально \n'   # после значения
"кл юч".x = 0x1F
num = [ 1_000, -2, +3, 0o17, 0b101 ]
fl = [1.5, -0.0, 1e3, 2.5E-1, inf, -inf]
s = """
первая\
    вторая
третья ""кавычки"" """
lit = '''
C:\путь'''
inl = { a = 1, b.c = "x", d = [], e = {} }
arr = [
  "a", # комментарий
  "b",
]

[t.u]
v = true

[[aot]]
x = 1
[aot.sub]
y = 2
[[aot]]
x = 2
[aot.sub]
y = 3
"#;
    let j = parse(t).unwrap();
    assert_eq!(j.str("title"), "буквально \\n");
    assert_eq!(j.get("кл юч").unwrap().int("x", 0), 31);
    let nums: Vec<i64> = j.arr("num").iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(nums, vec![1000, -2, 3, 15, 5]);
    assert_eq!(j.arr("fl")[2], Json::Num(1000.0));
    assert_eq!(j.arr("fl")[3], Json::Num(0.25));
    assert_eq!(j.str("s"), "перваявторая\nтретья \"\"кавычки\"\" ");
    assert_eq!(j.str("lit"), "C:\\путь");
    let inl = j.get("inl").unwrap();
    assert_eq!((inl.int("a", 0), inl.get("b").unwrap().str("c")), (1, "x".to_string()));
    assert_eq!(j.strings("arr"), vec!["a", "b"]);
    assert!(j.get("t").unwrap().get("u").unwrap().bool("v", false));
    let aot = j.arr("aot");
    assert_eq!(aot.len(), 2);
    assert_eq!(aot[1].get("sub").unwrap().int("y", 0), 3);
}

#[test]
fn reports_errors_with_line() {
    for (src, what) in [
        ("a = 1\na = 2\n", "строка 2"),
        ("[t]\n[t]\n", "дважды"),
        ("a = \"не закрыта\n", "не закрыта"),
        ("a = 1 2\n", "лишнее"),
        ("a = 07\n", "неверное число"),
        ("d = 1979-05-27\n", "даты"),
        ("a = [1 2]\n", "«,» или «]»"),
        ("= 1\n", "ключ"),
    ] {
        let e = parse(src).unwrap_err();
        assert!(e.contains(what), "{:?}: {}", src, e);
    }
}

#[test]
fn document_accepts_old_json() {
    let j = parse_document("  {\"format\": \"tvstudio-form\", \"x\": 1}").unwrap();
    assert_eq!(j.int("x", 0), 1);
    let t = parse_document("format = \"tvstudio-form\"\nx = 2\n").unwrap();
    assert_eq!(t.int("x", 0), 2);
}
