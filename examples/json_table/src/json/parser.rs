//! Разбор JSON — общий для примеров `json_parse`, `json_table`, `json_load`.
//!
//! Внешних зависимостей у библиотеки нет, поэтому разбор здесь свой —
//! маленький, но полный: строки с `\uXXXX` и суррогатными парами, числа,
//! вложенность (не глубже 200 уровней), ошибки с номером строки и столбца,
//! сообщения о ходе разбора и прерывание (для полосы выполнения).
//! В своей программе можно взять `serde_json`.

/// Значение JSON. Порядок ключей объекта сохраняется (как в файле) —
/// от него зависит порядок столбцов.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// Число хранится текстом, как в файле: `1e3`, `0.10` и большие целые
    /// показываются без искажений.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// Ошибка разбора с позицией (строка и столбец — с 1).
#[derive(Clone, Debug, PartialEq)]
pub struct JsonError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "строка {}, столбец {}: {}", self.line, self.col, self.msg)
    }
}

/// Функция хода разбора: получает (разобрано символов, всего символов),
/// возвращает `false`, чтобы прервать разбор.
pub type ParseHook<'a> = &'a mut dyn FnMut(usize, usize) -> bool;

/// Разборщик: позиция в массиве символов. Время жизни текста (`'a`) и
/// функции хода (`'h`) разные: текст создаётся внутри `parse_json_with`.
struct Parser<'a, 'h> {
    s: &'a [char],
    i: usize,
    /// Сообщать о ходе разбора (для полосы выполнения).
    hook: Option<&'h mut dyn FnMut(usize, usize) -> bool>,
    /// Позиция, на которой в следующий раз позвать `hook`.
    next_report: usize,
    /// Разбор прерван через `hook`.
    cancelled: bool,
}

/// Как часто сообщать о ходе разбора (в символах).
const REPORT_STEP: usize = 64 * 1024;

/// Разобрать текст JSON. Допускается метка порядка байт (BOM) в начале.
pub fn parse_json(text: &str) -> Result<Json, JsonError> {
    parse_json_with(text, None).map_err(|e| e.unwrap_or_else(|| unreachable!("без hook разбор не прерывается")))
}

/// Разбор с сообщениями о ходе работы. `Err(None)` — разбор прерван.
pub fn parse_json_with(text: &str, hook: Option<ParseHook<'_>>) -> Result<Json, Option<JsonError>> {
    let chars: Vec<char> = text.trim_start_matches('\u{feff}').chars().collect();
    let mut p = Parser { s: &chars, i: 0, hook, next_report: 0, cancelled: false };
    p.ws();
    let v = p.value(0);
    if p.cancelled {
        return Err(None);
    }
    let v = v.map_err(Some)?;
    p.ws();
    if p.i < p.s.len() {
        return Err(Some(p.err("лишние символы после конца данных")));
    }
    if let Some(h) = p.hook.as_mut() {
        h(p.s.len(), p.s.len());
    }
    Ok(v)
}

impl<'a, 'h> Parser<'a, 'h> {
    /// Ошибка в текущей позиции.
    fn err(&self, msg: &str) -> JsonError {
        let mut line = 1;
        let mut col = 1;
        for &c in &self.s[..self.i.min(self.s.len())] {
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        JsonError { line, col, msg: msg.into() }
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    /// Пропустить пробелы, табуляции и переводы строк.
    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.i += 1;
        }
    }

    /// Ожидаемое слово (`true`, `false`, `null`).
    fn word(&mut self, w: &str, v: Json) -> Result<Json, JsonError> {
        for c in w.chars() {
            if self.peek() != Some(c) {
                return Err(self.err(&format!("ожидалось «{}»", w)));
            }
            self.i += 1;
        }
        Ok(v)
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        // Защита от переполнения стека на «бесконечной» вложенности.
        if depth > 200 {
            return Err(self.err("слишком глубокая вложенность"));
        }
        // Сообщить о ходе разбора; `false` — прервать.
        if self.i >= self.next_report {
            self.next_report = self.i + REPORT_STEP;
            let (i, n) = (self.i, self.s.len());
            if let Some(h) = self.hook.as_mut() {
                if !h(i, n) {
                    self.cancelled = true;
                    return Err(self.err("прервано"));
                }
            }
        }
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.word("true", Json::Bool(true)),
            Some('f') => self.word("false", Json::Bool(false)),
            Some('n') => self.word("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(self.err(&format!("неожиданный символ «{}»", c))),
            None => Err(self.err("неожиданный конец файла")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.i += 1; // '{'
        let mut fields = Vec::new();
        self.ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek() != Some('"') {
                return Err(self.err("ожидалось имя поля в кавычках"));
            }
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(':') {
                return Err(self.err("ожидалось «:»"));
            }
            self.i += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            fields.push((key, v));
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                _ => return Err(self.err("ожидалось «,» или «}»")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.i += 1; // '['
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(self.err("ожидалось «,» или «]»")),
            }
        }
    }

    /// Четыре шестнадцатеричные цифры после `\u`.
    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut v = 0;
        for _ in 0..4 {
            let d = self.peek().and_then(|c| c.to_digit(16)).ok_or_else(|| self.err("ожидались 4 шестнадцатеричные цифры после \\u"))?;
            v = v * 16 + d;
            self.i += 1;
        }
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1; // '"'
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("незакрытая строка")),
                Some('"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some('\\') => {
                    self.i += 1;
                    let c = self.peek().ok_or_else(|| self.err("незакрытая строка"))?;
                    self.i += 1;
                    match c {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hi = self.hex4()?;
                            // Суррогатная пара: 😀 → 😀
                            let code = if (0xD800..0xDC00).contains(&hi) && self.peek() == Some('\\') && self.s.get(self.i + 1) == Some(&'u') {
                                self.i += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF)
                            } else {
                                hi
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => {
                            self.i -= 1;
                            return Err(self.err(&format!("неизвестная escape-последовательность «\\{}»", c)));
                        }
                    }
                }
                Some(c) if (c as u32) < 0x20 => return Err(self.err("управляющий символ внутри строки")),
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.i;
        if self.peek() == Some('-') {
            self.i += 1;
        }
        let digits = |p: &mut Parser<'_, '_>| {
            let s = p.i;
            while matches!(p.peek(), Some(c) if c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i > s
        };
        if !digits(self) {
            return Err(self.err("ожидалась цифра"));
        }
        if self.peek() == Some('.') {
            self.i += 1;
            if !digits(self) {
                return Err(self.err("ожидалась цифра после точки"));
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.i += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.err("ожидалась цифра в показателе степени"));
            }
        }
        Ok(Json::Num(self.s[start..self.i].iter().collect()))
    }
}

impl Json {
    /// Компактная запись JSON (для вложенных значений в ячейке).
    pub fn compact(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, None, 0);
        s
    }

    /// Запись JSON с отступами (для окна «Запись»).
    pub fn pretty(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, Some(2), 0);
        s
    }

    fn write(&self, out: &mut String, indent: Option<usize>, level: usize) {
        let nl = |out: &mut String, l: usize| {
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * l));
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(n),
            Json::Str(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Arr(a) => {
                if a.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        if indent.is_none() {
                            out.push(' ');
                        }
                    }
                    nl(out, level + 1);
                    v.write(out, indent, level + 1);
                }
                nl(out, level);
                out.push(']');
            }
            Json::Obj(o) => {
                if o.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        if indent.is_none() {
                            out.push(' ');
                        }
                    }
                    nl(out, level + 1);
                    Json::Str(k.clone()).write(out, None, 0);
                    out.push_str(": ");
                    v.write(out, indent, level + 1);
                }
                nl(out, level);
                out.push('}');
            }
        }
    }

    /// Текст для ячейки таблицы: строки — без кавычек, массивы значений —
    /// через запятую, остальное вложенное — компактным JSON.
    pub fn cell_text(&self) -> String {
        match self {
            Json::Null => "null".into(),
            Json::Bool(b) => b.to_string(),
            Json::Num(n) => n.clone(),
            // Переводы строк в ячейке заменяем, чтобы строка таблицы не «ломалась».
            Json::Str(s) => s.replace(['\n', '\r', '\t'], " "),
            Json::Arr(a) if a.iter().all(|v| !matches!(v, Json::Arr(_) | Json::Obj(_))) => {
                a.iter().map(|v| v.cell_text()).collect::<Vec<_>>().join(", ")
            }
            other => other.compact(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
