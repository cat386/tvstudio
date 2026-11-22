use super::*;
use crate::json;

fn rt(j: &Json) -> Json {
    let t = to_string(j);
    parse(&t).unwrap_or_else(|e| panic!("{}\n---\n{}", e, t))
}

#[test]
fn writes_block_style() {
    let j = json::parse(
        r#"{"format":"tvstudio-form","n":1,"f":0.5,"ok":true,"list":["a","b c"],"empty":[],"eo":{},
                "app":{"title":"t","menu":[{"title":"~Ф~айл","items":[{"title":"Open","key":"F3"},{"separator":true}]}]},
                "rows":[["a","b"],["c","d"]],"text":"line1\nline2","esc":"\u0003По центру"}"#,
    )
    .unwrap();
    let t = to_string(&j);
    let want = "format: tvstudio-form\nn: 1\nf: 0.5\nok: true\nlist: [a, b c]\nempty: []\neo: {}\napp:\n  title: t\n  menu:\n    - title: ~Ф~айл\n      items:\n        - title: Open\n          key: F3\n        - separator: true\nrows:\n  - [a, b]\n  - [c, d]\ntext: |-\n  line1\n  line2\nesc: \"\\x03По центру\"\n";
    assert_eq!(t, want, "\n{}", t);
    assert_eq!(rt(&j), j);
}

#[test]
fn quotes_only_when_needed() {
    for (s, want) in [
        ("hello world", "hello world"),
        ("Наименование:", "'Наименование:'"),
        ("a: b", "'a: b'"),
        ("x #y", "'x #y'"),
        ("42", "'42'"),
        ("3.5", "'3.5'"),
        ("true", "'true'"),
        ("yes", "'yes'"),
        ("null", "'null'"),
        ("~", "'~'"),
        ("", "''"),
        (" lead", "' lead'"),
        ("- item", "'- item'"),
        ("*star", "'*star'"),
        ("it's", "it's"),
        ("'q'", "'''q'''"),
        ("cm::OK", "cm::OK"),
        ("Да | Нет", "Да | Нет"),
        ("picture:###-####", "picture:###-####"),
        ("tab\there", "\"tab\\there\""),
    ] {
        assert_eq!(scalar(s, false), want, "{:?}", s);
        assert_eq!(parse(&format!("k: {}\n", scalar(s, false))).unwrap(), Json::obj(vec![("k", Json::s(s))]), "{:?}", s);
    }
    // Во встроенном списке — ещё запятые, скобки и двоеточия.
    assert_eq!(scalar("a,b", true), "'a,b'");
    assert_eq!(scalar("cm::OK", true), "'cm::OK'");
}

#[test]
fn roundtrips_tricky_values() {
    let j = Json::obj(vec![
        ("multi", Json::s("a\n  b\n\nc\n")),
        ("lead", Json::s("  indented\nnext")),
        ("tail2", Json::s("x\n\n")),
        ("trail_space", Json::s("a \nb")),
        ("ctrl", Json::s("\x03a\nb")),
        ("quote\"key", Json::i(-7)),
        ("big", Json::Num(1e20)),
        ("neg", Json::Num(-0.25)),
        ("nul", Json::Null),
        ("nested", Json::Arr(vec![Json::Arr(vec![Json::obj(vec![("k", Json::s("v"))])]), Json::Arr(vec![])])),
        ("objs_in_flowless", Json::Arr(vec![Json::obj(vec![]), Json::obj(vec![("a", Json::Arr(vec![Json::i(1)]))])])),
    ]);
    assert_eq!(rt(&j), j, "\n{}", to_string(&j));
}

#[test]
fn parses_hand_written_yaml() {
    let src = "\u{feff}%YAML 1.2\n---\n# комментарий\nformat: tvstudio-project   # хвост\nname: \"demo\"\nlist:\n- 1\n- two\n-   - nested\n    - x\nmap: {a: 1, b: [x, 'y z'], c: \"q\\u0041\"}\nflow: [\n  a,\n  b,\n]\ntext: >\n  folded\n  line\n\n  para\nlit: |+\n  keep\n\nplain: multi\n  line plain\nsingle: 'it''s'\nhex: 0x1F\noct: 0o17\nexp: 1e3\nnot_num: 1.2.3\nempty:\n...\nignored: yes\n";
    let j = parse(src).unwrap();
    assert_eq!(j.get("format").unwrap().as_str(), Some("tvstudio-project"));
    assert_eq!(j.get("name").unwrap().as_str(), Some("demo"));
    assert_eq!(json::Json::to_compact(j.get("list").unwrap()), r#"[1,"two",["nested","x"]]"#);
    assert_eq!(json::Json::to_compact(j.get("map").unwrap()), r#"{"a":1,"b":["x","y z"],"c":"qA"}"#);
    assert_eq!(json::Json::to_compact(j.get("flow").unwrap()), r#"["a","b"]"#);
    assert_eq!(j.get("text").unwrap().as_str(), Some("folded line\npara\n"));
    assert_eq!(j.get("lit").unwrap().as_str(), Some("keep\n\n"));
    assert_eq!(j.get("plain").unwrap().as_str(), Some("multi line plain"));
    assert_eq!(j.get("single").unwrap().as_str(), Some("it's"));
    assert_eq!(j.get("hex").unwrap().as_i64(), Some(31));
    assert_eq!(j.get("oct").unwrap().as_i64(), Some(15));
    assert_eq!(j.get("exp").unwrap().as_i64(), Some(1000));
    assert_eq!(j.get("not_num").unwrap().as_str(), Some("1.2.3"));
    assert_eq!(j.get("empty"), Some(&Json::Null));
    assert!(j.get("ignored").is_none(), "после «...» документ закончен");
}

#[test]
fn reports_errors_with_line() {
    for (src, line, what) in [
        ("a: 1\n  b: 2\n", 2, "отступ"),
        ("a: 1\na: 2\n", 2, "повторяется"),
        ("a: &x 1\n", 1, "якоря"),
        ("a: [1, 2\n", 1, "не закрыта"),
        ("a: \"x\n", 1, "кавычка"),
        ("a:\n\t- 1\n", 2, "табуляция"),
        ("a: \"\\q\"\n", 1, "escape"),
    ] {
        let e = parse(src).unwrap_err();
        assert!(e.contains(&format!("строка {}", line)) && e.contains(what), "{:?} → {}", src, e);
    }
}

/// Случайные деревья с «неудобными» строками: запись → разбор даёт то же.
#[test]
fn random_roundtrip() {
    fn rnd(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }
    const PARTS: [&str; 34] = [
        "a", " ", ":", "#", "-", "'", "\"", "\n", "~", "Ф", "1", ".", ",", "[", "]", "{", "}", "|", ">", "\x03", "\t", "true", "null", "0x1", "*",
        "&", "!", "%", "@", "?", "yes", "\\", "  ", "e5",
    ];
    fn rstr(seed: &mut u64) -> String {
        (0..rnd(seed) % 6).map(|_| PARTS[(rnd(seed) % PARTS.len() as u64) as usize]).collect()
    }
    fn rval(seed: &mut u64, d: u32) -> Json {
        match rnd(seed) % if d > 3 { 4 } else { 7 } {
            0 => Json::Str(rstr(seed)),
            1 => Json::Num((rnd(seed) % 2000) as f64 - 1000.0),
            2 => Json::Bool(rnd(seed).is_multiple_of(2)),
            3 => Json::Num((rnd(seed) % 1000) as f64 / 8.0),
            4 | 5 => Json::Obj((0..rnd(seed) % 4).map(|i| (format!("{}{}", rstr(seed), i), rval(seed, d + 1))).collect()),
            _ => Json::Arr((0..rnd(seed) % 4).map(|_| rval(seed, d + 1)).collect()),
        }
    }
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..2000 {
        let v = Json::Obj(vec![("root".into(), rval(&mut seed, 0))]);
        assert_eq!(rt(&v), v, "\n{}", to_string(&v));
    }
}

#[test]
fn detects_formats() {
    assert_eq!(detect("{\"format\": 1}"), DocFormat::Json);
    assert_eq!(detect("format = \"tvstudio-project\"\n"), DocFormat::Toml);
    assert_eq!(detect("# c\n[app]\ntitle = \"x\"\n"), DocFormat::Toml);
    assert_eq!(detect("format: tvstudio-project\n"), DocFormat::Yaml);
    assert_eq!(detect("---\nformat: x\n"), DocFormat::Yaml);
    assert_eq!(detect("title: \"a = b\"\n"), DocFormat::Yaml);
    let toml = "format = \"tvstudio-form\"\nname = \"x\"\n";
    assert_eq!(parse_document(toml).unwrap().get("name").unwrap().as_str(), Some("x"));
}
