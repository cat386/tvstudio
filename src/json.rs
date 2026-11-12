//! Небольшой JSON без внешних зависимостей: разбор и красивая запись.
//!
//! Дерево значений [`Json`] — общее для файлов проекта и форм (они
//! пишутся в YAML, см. [`crate::yaml`]; файлы старых версий — TOML и
//! JSON — читаются модулем [`crate::toml`] и отсюда), настроек среды, буфера обмена дизайнера и протокола
//! LSP. Объект хранит пары в порядке добавления — так файлы получаются
//! стабильными и их удобно сравнивать в системе контроля версий.

use std::fmt::Write;

/// Значение JSON.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Json {
    #[default]
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Объект: пары «ключ — значение» в порядке добавления.
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Объект из пар (удобно при записи).
    pub fn obj(pairs: Vec<(&str, Json)>) -> Json {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// Строка.
    pub fn s(v: &str) -> Json {
        Json::Str(v.to_string())
    }

    /// Целое число.
    pub fn i(v: i64) -> Json {
        Json::Num(v as f64)
    }

    /// Поле объекта.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(p) => p.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(n) => Some(*n as i64),
            Json::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            _ => &[],
        }
    }

    /// Строковое поле (пустая строка, если нет).
    pub fn str(&self, key: &str) -> String {
        self.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }

    /// Числовое поле со значением по умолчанию.
    pub fn int(&self, key: &str, def: i64) -> i64 {
        self.get(key).and_then(|v| v.as_i64()).unwrap_or(def)
    }

    /// Логическое поле со значением по умолчанию.
    pub fn bool(&self, key: &str, def: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(def)
    }

    /// Поле-массив (пустой срез, если нет).
    pub fn arr(&self, key: &str) -> &[Json] {
        self.get(key).map(|v| v.as_arr()).unwrap_or(&[])
    }

    /// Массив строк.
    pub fn strings(&self, key: &str) -> Vec<String> {
        self.arr(key).iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    }

    /// Массив строк → значение.
    pub fn from_strings(v: &[String]) -> Json {
        Json::Arr(v.iter().map(|s| Json::s(s)).collect())
    }

    /// Запись с отступами в два пробела.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    /// Запись в одну строку без пробелов (для протоколов вроде LSP).
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write_compact(&mut out);
        out
    }

    fn write_compact(&self, out: &mut String) {
        match self {
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write_compact(out);
                }
                out.push(']');
            }
            Json::Obj(p) => {
                out.push('{');
                for (i, (k, v)) in p.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(out, k);
                    out.push(':');
                    v.write_compact(out);
                }
                out.push('}');
            }
            other => other.write(out, 0),
        }
    }

    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    let _ = write!(out, "{}", *n as i64);
                } else {
                    let _ = write!(out, "{}", n);
                }
            }
            Json::Str(s) => write_str(out, s),
            Json::Arr(a) => {
                // Короткие массивы простых значений — в одну строку.
                let simple = a.iter().all(|v| !matches!(v, Json::Arr(_) | Json::Obj(_)));
                if a.is_empty() {
                    out.push_str("[]");
                } else if simple && a.len() <= 16 {
                    out.push('[');
                    for (i, v) in a.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        v.write(out, indent);
                    }
                    out.push(']');
                } else {
                    out.push_str("[\n");
                    for (i, v) in a.iter().enumerate() {
                        pad(out, indent + 1);
                        v.write(out, indent + 1);
                        if i + 1 < a.len() {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    pad(out, indent);
                    out.push(']');
                }
            }
            Json::Obj(p) => {
                if p.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for (i, (k, v)) in p.iter().enumerate() {
                    pad(out, indent + 1);
                    write_str(out, k);
                    out.push_str(": ");
                    v.write(out, indent + 1);
                    if i + 1 < p.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Разбор текста JSON. Ошибка содержит номер строки.
pub fn parse(src: &str) -> Result<Json, String> {
    let mut p = Parser { s: src.chars().collect(), i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i < p.s.len() {
        return Err(p.err("лишние символы после значения"));
    }
    Ok(v)
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn err(&self, msg: &str) -> String {
        let line = self.s[..self.i.min(self.s.len())].iter().filter(|&&c| c == '\n').count() + 1;
        format!("JSON, строка {}: {}", line, msg)
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("ожидался символ «{}»", c)))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.word("true", Json::Bool(true)),
            Some('f') => self.word("false", Json::Bool(false)),
            Some('n') => self.word("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.err("ожидалось значение")),
        }
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        for c in w.chars() {
            self.expect(c)?;
        }
        Ok(v)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E') {
                self.i += 1;
            } else {
                break;
            }
        }
        let t: String = self.s[start..self.i].iter().collect();
        t.parse::<f64>().map(Json::Num).map_err(|_| self.err("неверное число"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or_else(|| self.err("незакрытая строка"))?;
            self.i += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.peek().ok_or_else(|| self.err("незакрытая строка"))?;
                    self.i += 1;
                    match e {
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'u' => {
                            let h: String = self.s.get(self.i..self.i + 4).map(|s| s.iter().collect()).unwrap_or_default();
                            self.i += 4;
                            let code = u32::from_str_radix(&h, 16).map_err(|_| self.err("неверный \\u"))?;
                            out.push(char::from_u32(code).unwrap_or('?'));
                        }
                        c => out.push(c),
                    }
                }
                c => out.push(c),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut v = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(Json::Arr(v));
        }
        loop {
            self.ws();
            v.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Json::Arr(v));
                }
                _ => return Err(self.err("ожидалась «,» или «]»")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut p = Vec::new();
        self.ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(Json::Obj(p));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            self.expect(':')?;
            self.ws();
            let v = self.value()?;
            p.push((k, v));
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(Json::Obj(p));
                }
                _ => return Err(self.err("ожидалась «,» или «}»")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
