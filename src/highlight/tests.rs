use super::*;

fn classes(s: &str) -> Vec<Class> {
    let mut st = State::Normal;
    highlight_line(&s.chars().collect::<Vec<_>>(), &mut st, Lang::Rust)
}

#[test]
fn rust_tokens() {
    let c = classes("let x = \"a\"; // ок");
    assert_eq!(c[0], Class::Keyword);
    assert_eq!(c[8], Class::Str);
    assert_eq!(*c.last().unwrap(), Class::Comment);
    let c = classes("println!(\"{}\", 'a', 42u8);");
    assert_eq!(c[0], Class::Macro);
    assert_eq!(c[7], Class::Macro);
    let c = classes("fn f<'a>(x: &'a Vec<u8>) {}");
    assert_eq!(c[5], Class::Lifetime);
    assert_eq!(c[17], Class::Type);
}

#[test]
fn block_comment_spans_lines() {
    let mut st = State::Normal;
    let l1: Vec<char> = "a /* b".chars().collect();
    let l2: Vec<char> = "c */ d".chars().collect();
    highlight_line(&l1, &mut st, Lang::Rust);
    assert_eq!(st, State::Block(1));
    let c = highlight_line(&l2, &mut st, Lang::Rust);
    assert_eq!(c[0], Class::Comment);
    assert_eq!(c[5], Class::Normal);
    assert_eq!(st, State::Normal);
}

#[test]
fn yaml() {
    let hl = |s: &str, st: &mut State| highlight_line(&s.chars().collect::<Vec<_>>(), st, Lang::Yaml);
    let mut st = State::Normal;
    let c = hl("name: about  # c", &mut st);
    assert_eq!((c[0], c[4], c[6], c[13]), (Class::Type, Class::Punct, Class::Str, Class::Comment));
    let c = hl("  - w: 40", &mut st);
    assert_eq!((c[2], c[4], c[7]), (Class::Punct, Class::Type, Class::Number));
    let c = hl("ok: [true, 'x', 1]", &mut st);
    assert_eq!((c[4], c[5], c[11], c[16]), (Class::Punct, Class::Keyword, Class::Str, Class::Number));
    let c = hl("text: |-", &mut st);
    assert_eq!(c[6], Class::Punct);
    assert_eq!(st, State::YamlBlock(0));
    let c = hl("  # не комментарий: это текст", &mut st);
    assert_eq!(c[2], Class::Str);
    hl("", &mut st);
    assert_eq!(st, State::YamlBlock(0));
    let c = hl("next: 1", &mut st);
    assert_eq!((c[0], st), (Class::Type, State::Normal));
    assert_eq!(Lang::from_path(std::path::Path::new("forms/a.tvf")), Lang::Yaml);
}

#[test]
fn toml() {
    let mut st = State::Normal;
    let c = highlight_line(&"name = \"x\" # c".chars().collect::<Vec<_>>(), &mut st, Lang::Toml);
    assert_eq!(c[0], Class::Type);
    assert_eq!(c[7], Class::Str);
    assert_eq!(c[13], Class::Comment);
}
