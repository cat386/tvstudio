//! Подсветка синтаксиса: Rust, TOML и JSON.
//!
//! Разбор идёт построчно; между строками переносится только состояние
//! «внутри многострочного комментария / строки». Этого достаточно для
//! редактора: подсветка не обязана быть компилятором.

/// Ключевые слова Rust.
pub const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let",
    "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Встроенные типы Rust (подсвечиваются как типы).
const RUST_PRIMITIVES: &[&str] =
    &["bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64"];

/// Язык файла.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Toml,
    Json,
    Plain,
}

impl Lang {
    /// Язык по расширению файла.
    pub fn from_path(p: &std::path::Path) -> Lang {
        match p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
            Some("rs") => Lang::Rust,
            Some("toml") => Lang::Toml,
            Some("json") | Some("tvf") | Some("tvsp") => Lang::Json,
            _ => Lang::Plain,
        }
    }
}

/// Класс символа.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Normal,
    Keyword,
    Type,
    Number,
    Str,
    Comment,
    Macro,
    Attr,
    Lifetime,
    Punct,
}

/// Состояние на границе строк.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Normal,
    /// Внутри `/* … */` (глубина вложенности).
    Block(u8),
    /// Внутри обычной строки `"…"`.
    Str,
    /// Внутри сырой строки `r#"…"#` (число `#`).
    Raw(u8),
    /// TOML: внутри многострочной строки `"""…"""`.
    TomlMulti,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Подсветить строку. `state` — состояние в начале строки, после вызова —
/// в её конце. Результат — класс каждого символа.
pub fn highlight_line(line: &[char], state: &mut State, lang: Lang) -> Vec<Class> {
    match lang {
        Lang::Rust => rust_line(line, state),
        Lang::Toml => toml_line(line, state),
        Lang::Json => json_line(line),
        Lang::Plain => vec![Class::Normal; line.len()],
    }
}

fn rust_line(l: &[char], state: &mut State) -> Vec<Class> {
    let n = l.len();
    let mut out = vec![Class::Normal; n];
    let mut i = 0;
    while i < n {
        match *state {
            State::Block(depth) => {
                // Ищем конец комментария (с учётом вложенности).
                while i < n {
                    out[i] = Class::Comment;
                    if l[i] == '*' && i + 1 < n && l[i + 1] == '/' {
                        out[i + 1] = Class::Comment;
                        i += 2;
                        *state = if depth <= 1 { State::Normal } else { State::Block(depth - 1) };
                        break;
                    }
                    if l[i] == '/' && i + 1 < n && l[i + 1] == '*' {
                        out[i + 1] = Class::Comment;
                        i += 2;
                        *state = State::Block(depth.saturating_add(1));
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            State::Str => {
                while i < n {
                    out[i] = Class::Str;
                    if l[i] == '\\' {
                        if i + 1 < n {
                            out[i + 1] = Class::Str;
                        }
                        i += 2;
                        continue;
                    }
                    if l[i] == '"' {
                        i += 1;
                        *state = State::Normal;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            State::Raw(h) => {
                while i < n {
                    out[i] = Class::Str;
                    if l[i] == '"' && (0..h as usize).all(|k| i + 1 + k < n && l[i + 1 + k] == '#') {
                        for k in 0..=h as usize {
                            out[i + k] = Class::Str;
                        }
                        i += 1 + h as usize;
                        *state = State::Normal;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            State::TomlMulti => *state = State::Normal,
            State::Normal => {}
        }
        if i >= n {
            break;
        }
        let c = l[i];
        // Комментарии.
        if c == '/' && i + 1 < n && l[i + 1] == '/' {
            for o in out.iter_mut().skip(i) {
                *o = Class::Comment;
            }
            break;
        }
        if c == '/' && i + 1 < n && l[i + 1] == '*' {
            out[i] = Class::Comment;
            out[i + 1] = Class::Comment;
            i += 2;
            *state = State::Block(1);
            continue;
        }
        // Сырые строки r"…", r#"…"#, br"…".
        if (c == 'r' || (c == 'b' && i + 1 < n && l[i + 1] == 'r')) && (i == 0 || !is_ident_char(l[i - 1])) {
            let mut j = i + if c == 'b' { 2 } else { 1 };
            let mut h = 0u8;
            while j < n && l[j] == '#' {
                h += 1;
                j += 1;
            }
            if j < n && l[j] == '"' {
                for o in out.iter_mut().take(j + 1).skip(i) {
                    *o = Class::Str;
                }
                i = j + 1;
                *state = State::Raw(h);
                continue;
            }
        }
        // Строки (и байтовые строки b"…").
        if c == '"' || (c == 'b' && i + 1 < n && l[i + 1] == '"' && (i == 0 || !is_ident_char(l[i - 1]))) {
            let start = i;
            i += if c == 'b' { 2 } else { 1 };
            for o in out.iter_mut().take(i).skip(start) {
                *o = Class::Str;
            }
            *state = State::Str;
            continue;
        }
        // Символ 'x' или время жизни 'a.
        if c == '\'' {
            // Символьный литерал: 'x', '\n', '\u{1F600}'.
            let mut j = i + 1;
            if j < n && l[j] == '\\' {
                j += 2;
                while j < n && l[j] != '\'' {
                    j += 1;
                }
                if j < n {
                    for o in out.iter_mut().take(j + 1).skip(i) {
                        *o = Class::Str;
                    }
                    i = j + 1;
                    continue;
                }
            } else if j + 1 < n && l[j + 1] == '\'' {
                for o in out.iter_mut().take(j + 2).skip(i) {
                    *o = Class::Str;
                }
                i = j + 2;
                continue;
            }
            // Время жизни.
            j = i + 1;
            while j < n && is_ident_char(l[j]) {
                j += 1;
            }
            for o in out.iter_mut().take(j).skip(i) {
                *o = Class::Lifetime;
            }
            i = j.max(i + 1);
            continue;
        }
        // Атрибуты #[…] и #![…] — до конца строки или до закрывающей скобки.
        if c == '#' && i + 1 < n && (l[i + 1] == '[' || (l[i + 1] == '!' && i + 2 < n && l[i + 2] == '[')) {
            let mut depth = 0;
            let mut j = i;
            while j < n {
                if l[j] == '[' {
                    depth += 1;
                } else if l[j] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                j += 1;
            }
            for o in out.iter_mut().take(j).skip(i) {
                *o = Class::Attr;
            }
            i = j;
            continue;
        }
        // Числа.
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (l[j].is_ascii_alphanumeric() || l[j] == '_' || (l[j] == '.' && j + 1 < n && l[j + 1].is_ascii_digit())) {
                j += 1;
            }
            for o in out.iter_mut().take(j).skip(i) {
                *o = Class::Number;
            }
            i = j;
            continue;
        }
        // Идентификаторы, ключевые слова, макросы, типы.
        if is_ident_start(c) {
            let mut j = i;
            while j < n && is_ident_char(l[j]) {
                j += 1;
            }
            let word: String = l[i..j].iter().collect();
            let class = if j < n && l[j] == '!' && !(j + 1 < n && l[j + 1] == '=') {
                Class::Macro
            } else if RUST_KEYWORDS.contains(&word.as_str()) {
                Class::Keyword
            } else if RUST_PRIMITIVES.contains(&word.as_str()) || c.is_uppercase() {
                Class::Type
            } else {
                Class::Normal
            };
            let end = if class == Class::Macro { j + 1 } else { j };
            for o in out.iter_mut().take(end).skip(i) {
                *o = class;
            }
            i = end;
            continue;
        }
        if "{}()[];,.:&|<>=+-*/%!?^@".contains(c) {
            out[i] = Class::Punct;
        }
        i += 1;
    }
    // Незакрытая обычная строка продолжается на следующей строке (как в Rust).
    out
}

fn toml_line(l: &[char], state: &mut State) -> Vec<Class> {
    let n = l.len();
    let mut out = vec![Class::Normal; n];
    let mut i = 0;
    if *state == State::TomlMulti {
        while i < n {
            out[i] = Class::Str;
            if i + 2 < n && l[i] == '"' && l[i + 1] == '"' && l[i + 2] == '"' {
                out[i + 1] = Class::Str;
                out[i + 2] = Class::Str;
                i += 3;
                *state = State::Normal;
                break;
            }
            i += 1;
        }
    }
    // Заголовок секции.
    let first = l.iter().position(|c| !c.is_whitespace());
    if *state == State::Normal && first.map(|f| l[f] == '[').unwrap_or(false) && i == 0 {
        let f = first.unwrap();
        let end = l.iter().rposition(|&c| c == ']').map(|e| e + 1).unwrap_or(n);
        for o in out.iter_mut().take(end).skip(f) {
            *o = Class::Keyword;
        }
        i = end;
    }
    let mut key_part = !l[..i.min(n)].contains(&'=');
    while i < n {
        let c = l[i];
        if c == '#' {
            for o in out.iter_mut().skip(i) {
                *o = Class::Comment;
            }
            break;
        }
        if c == '"' || c == '\'' {
            if c == '"' && i + 2 < n && l[i + 1] == '"' && l[i + 2] == '"' {
                // Многострочная строка.
                let mut j = i + 3;
                let mut closed = false;
                while j + 2 < n {
                    if l[j] == '"' && l[j + 1] == '"' && l[j + 2] == '"' {
                        j += 3;
                        closed = true;
                        break;
                    }
                    j += 1;
                }
                let end = if closed { j } else { n };
                for o in out.iter_mut().take(end).skip(i) {
                    *o = Class::Str;
                }
                if !closed {
                    *state = State::TomlMulti;
                }
                i = end;
                continue;
            }
            let mut j = i + 1;
            while j < n && l[j] != c {
                if l[j] == '\\' && c == '"' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(n);
            for o in out.iter_mut().take(end).skip(i) {
                *o = Class::Str;
            }
            i = end;
            continue;
        }
        if c == '=' {
            out[i] = Class::Punct;
            key_part = false;
            i += 1;
            continue;
        }
        if key_part && (is_ident_char(c) || c == '-' || c == '.') {
            out[i] = Class::Type;
        } else if c.is_ascii_digit() {
            out[i] = Class::Number;
        } else if is_ident_start(c) {
            let mut j = i;
            while j < n && is_ident_char(l[j]) {
                j += 1;
            }
            let w: String = l[i..j].iter().collect();
            if w == "true" || w == "false" {
                for o in out.iter_mut().take(j).skip(i) {
                    *o = Class::Keyword;
                }
            }
            i = j;
            continue;
        } else if "{}[],".contains(c) {
            out[i] = Class::Punct;
        }
        i += 1;
    }
    out
}

fn json_line(l: &[char]) -> Vec<Class> {
    let n = l.len();
    let mut out = vec![Class::Normal; n];
    let mut i = 0;
    while i < n {
        let c = l[i];
        if c == '"' {
            let mut j = i + 1;
            while j < n && l[j] != '"' {
                if l[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(n);
            // Ключ — строка, за которой идёт двоеточие.
            let mut k = end;
            while k < n && l[k] == ' ' {
                k += 1;
            }
            let class = if k < n && l[k] == ':' { Class::Type } else { Class::Str };
            for o in out.iter_mut().take(end).skip(i) {
                *o = class;
            }
            i = end;
            continue;
        }
        if c.is_ascii_digit() || c == '-' {
            out[i] = Class::Number;
        } else if is_ident_start(c) {
            let mut j = i;
            while j < n && is_ident_char(l[j]) {
                j += 1;
            }
            for o in out.iter_mut().take(j).skip(i) {
                *o = Class::Keyword;
            }
            i = j;
            continue;
        } else if "{}[],:".contains(c) {
            out[i] = Class::Punct;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
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
    fn toml() {
        let mut st = State::Normal;
        let c = highlight_line(&"name = \"x\" # c".chars().collect::<Vec<_>>(), &mut st, Lang::Toml);
        assert_eq!(c[0], Class::Type);
        assert_eq!(c[7], Class::Str);
        assert_eq!(c[13], Class::Comment);
    }
}
