//! Небольшой TOML без внешних зависимостей: запись и разбор.
//!
//! Нужен для `Cargo.toml` (состав workspace, имя пакета) и для файлов
//! проекта (`*.tvsp`) и форм (`*.tvf`) прежних версий — теперь они в YAML
//! (см. [`crate::yaml`]). Работает с тем же
//! деревом значений [`Json`], что и модуль [`crate::json`]: модель
//! переводит себя в дерево (`to_json`) и обратно (`from_json`), а здесь —
//! только текст. Порядок ключей сохраняется — файлы стабильны и удобно
//! сравниваются в системе контроля версий.
//!
//! Запись: простые значения и массивы — `ключ = значение`, вложенные
//! объекты — таблицы `[a.b]`, массивы объектов — `[[a.b]]`; строки с
//! переводами строк — многострочные `"""…"""`. В TOML нет `null`: такие
//! ключи пропускаются (модель их и не пишет).
//!
//! Разбор понимает TOML 1.0 без дат и времени: комментарии, простые,
//! в кавычках и составные (`a.b`) ключи, все четыре вида строк, целые
//! (в том числе `0x`, `0o`, `0b`, с `_`), дробные, `inf`/`nan`, логические,
//! массивы (многострочные, с запятой в конце), встроенные таблицы,
//! заголовки `[таблица]` и `[[массив таблиц]]`.

use std::fmt::Write;

use crate::json::Json;

// ======================================================================
// Запись
// ======================================================================

/// Документ TOML из объекта. Корень должен быть объектом.
pub fn to_string(v: &Json) -> Result<String, String> {
    let Json::Obj(pairs) = v else { return Err("TOML: корень документа должен быть таблицей".into()) };
    let mut out = String::new();
    write_table(&mut out, &[], pairs, None);
    // Ровно один перевод строки в конце.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Массив, который пишется заголовками `[[…]]`: непустой, только объекты.
fn is_table_array(v: &Json) -> bool {
    matches!(v, Json::Arr(a) if !a.is_empty() && a.iter().all(|x| matches!(x, Json::Obj(_))))
}

/// Таблица `path`: сначала свои простые ключи, потом вложенные таблицы
/// и массивы таблиц (после заголовка таблицы ключи уже не её).
/// `header`: `None` — корень, `Some(false)` — `[path]`, `Some(true)` — `[[path]]`.
fn write_table(out: &mut String, path: &[String], pairs: &[(String, Json)], header: Option<bool>) {
    if let Some(aot) = header {
        if !out.is_empty() {
            out.push('\n');
        }
        let p = path.iter().map(|k| key(k)).collect::<Vec<_>>().join(".");
        let _ = writeln!(out, "{}", if aot { format!("[[{}]]", p) } else { format!("[{}]", p) });
    }
    for (k, v) in pairs {
        if matches!(v, Json::Null | Json::Obj(_)) || is_table_array(v) {
            continue;
        }
        let _ = write!(out, "{} = ", key(k));
        write_value(out, v, 0, true);
        out.push('\n');
    }
    for (k, v) in pairs {
        let mut sub = path.to_vec();
        sub.push(k.clone());
        match v {
            Json::Obj(p) => write_table(out, &sub, p, Some(false)),
            Json::Arr(a) if is_table_array(v) => {
                for item in a {
                    if let Json::Obj(p) = item {
                        write_table(out, &sub, p, Some(true));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Ключ: голый (`[A-Za-z0-9_-]+`) или в кавычках.
fn key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        k.to_string()
    } else {
        quote(k)
    }
}

/// Однострочная строка в двойных кавычках.
fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(o, "\\u{:04X}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Многострочная строка `"""…"""`: переводы строк — как есть, кавычки и
/// `\` экранированы (так `"""` внутри не встретится), прочие управляющие —
/// `\uXXXX`. Перевод строки сразу после `"""` разбор отбрасывает.
fn quote_multi(s: &str) -> String {
    let mut o = String::from("\"\"\"\n");
    for c in s.chars() {
        match c {
            '\n' => o.push('\n'),
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(o, "\\u{:04X}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push_str("\"\"\"");
    o
}

fn number(n: f64) -> String {
    if n.is_nan() {
        "nan".into()
    } else if n.is_infinite() {
        if n > 0.0 { "inf" } else { "-inf" }.into()
    } else if n.fract() == 0.0 && n.abs() < 9e15 {
        format!("{}", n as i64)
    } else {
        // `{:?}` всегда даёт точку или экспоненту: 0.5, 1e-7.
        format!("{:?}", n)
    }
}

/// Значение справа от `=` или внутри массива. `top` — прямо у ключа
/// (там допустима многострочная строка).
fn write_value(out: &mut String, v: &Json, depth: usize, top: bool) {
    match v {
        Json::Null => out.push_str("\"\""),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) => out.push_str(&number(*n)),
        Json::Str(s) if top && s.contains('\n') => out.push_str(&quote_multi(s)),
        Json::Str(s) => out.push_str(&quote(s)),
        Json::Obj(p) => {
            // Встроенная таблица (объект внутри массива).
            if p.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{ ");
            let mut first = true;
            for (k, v) in p.iter().filter(|(_, v)| !matches!(v, Json::Null)) {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                let _ = write!(out, "{} = ", key(k));
                write_value(out, v, depth, false);
            }
            out.push_str(" }");
        }
        Json::Arr(a) => {
            let items: Vec<&Json> = a.iter().filter(|x| !matches!(x, Json::Null)).collect();
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            // В одну строку — если коротко и без вложенных массивов и таблиц.
            let mut line = String::new();
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    line.push_str(", ");
                }
                write_value(&mut line, x, depth + 1, false);
            }
            let nested = items.iter().any(|x| matches!(x, Json::Arr(_) | Json::Obj(_)));
            if !nested && line.chars().count() <= 100 && !line.contains('\n') {
                let _ = write!(out, "[{}]", line);
                return;
            }
            let pad = "  ".repeat(depth + 1);
            out.push_str("[\n");
            for x in items {
                out.push_str(&pad);
                write_value(out, x, depth + 1, false);
                out.push_str(",\n");
            }
            out.push_str(&"  ".repeat(depth));
            out.push(']');
        }
    }
}

// ======================================================================
// Разбор
// ======================================================================

/// Разобрать документ TOML в дерево значений (корень — объект).
pub fn parse(src: &str) -> Result<Json, String> {
    let mut p = Parser { s: src.chars().collect(), i: 0 };
    let mut root = Json::Obj(Vec::new());
    // Текущая таблица — путь от корня; для массивов таблиц берётся последний элемент.
    let mut current: Vec<String> = Vec::new();
    // Таблицы, явно объявленные заголовком `[…]` (повтор — ошибка).
    let mut declared: Vec<Vec<String>> = Vec::new();
    loop {
        p.skip_blank_lines();
        let Some(c) = p.peek() else { break };
        if c == '[' {
            let aot = p.peek_at(1) == Some('[');
            p.i += if aot { 2 } else { 1 };
            p.spaces();
            let path = p.key_path()?;
            p.spaces();
            if aot {
                p.expect(']')?;
                p.expect(']')?;
            } else {
                p.expect(']')?;
            }
            // Ошибки ниже — о строке заголовка, поэтому её конец — после них.
            let (last, parent) = path.split_last().ok_or_else(|| p.err("пустой заголовок таблицы"))?;
            let tbl = table_mut(&mut root, parent, &p)?;
            if aot {
                match find_mut(tbl, last) {
                    Some(Json::Arr(a)) => a.push(Json::Obj(Vec::new())),
                    Some(_) => return Err(p.err(&format!("«{}» уже задан и это не массив таблиц", last))),
                    None => tbl.push((last.clone(), Json::Arr(vec![Json::Obj(Vec::new())]))),
                }
                // Новый элемент — новые вложенные таблицы.
                declared.retain(|d| !d.starts_with(&path));
            } else {
                if declared.contains(&path) {
                    return Err(p.err(&format!("таблица [{}] объявлена дважды", path.join("."))));
                }
                declared.push(path.clone());
                match find_mut(tbl, last) {
                    Some(Json::Obj(_)) => {}
                    Some(_) => return Err(p.err(&format!("«{}» уже задан и это не таблица", last))),
                    None => tbl.push((last.clone(), Json::Obj(Vec::new()))),
                }
            }
            current = path;
            p.end_of_line()?;
        } else {
            let path = p.key_path()?;
            p.spaces();
            p.expect('=')?;
            p.spaces();
            let v = p.value()?;
            let tbl = table_mut(&mut root, &current, &p)?;
            insert(tbl, &path, v, &p)?;
            p.end_of_line()?;
        }
    }
    Ok(root)
}

fn find_mut<'a>(tbl: &'a mut [(String, Json)], k: &str) -> Option<&'a mut Json> {
    tbl.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v)
}

/// Таблица по пути (недостающие создаются; у массива таблиц — последний элемент).
fn table_mut<'a>(root: &'a mut Json, path: &[String], p: &Parser) -> Result<&'a mut Vec<(String, Json)>, String> {
    let mut cur = root;
    for k in path {
        let Json::Obj(tbl) = cur else { return Err(p.err("ожидалась таблица")) };
        if find_mut(tbl, k).is_none() {
            tbl.push((k.clone(), Json::Obj(Vec::new())));
        }
        let next = find_mut(tbl, k).expect("только что добавлен");
        cur = match next {
            Json::Arr(a) => a.last_mut().filter(|x| matches!(x, Json::Obj(_))).ok_or_else(|| p.err(&format!("«{}» — не таблица", k)))?,
            Json::Obj(_) => next,
            _ => return Err(p.err(&format!("«{}» уже задан и это не таблица", k))),
        };
    }
    match cur {
        Json::Obj(t) => Ok(t),
        _ => Err(p.err("ожидалась таблица")),
    }
}

/// Записать значение по составному ключу `a.b.c` (промежуточные таблицы создаются).
fn insert(tbl: &mut Vec<(String, Json)>, path: &[String], v: Json, p: &Parser) -> Result<(), String> {
    let (last, inner) = path.split_last().ok_or_else(|| p.err("пустой ключ"))?;
    let mut t = tbl;
    for k in inner {
        if find_mut(t, k).is_none() {
            t.push((k.clone(), Json::Obj(Vec::new())));
        }
        match find_mut(t, k) {
            Some(Json::Obj(next)) => t = next,
            _ => return Err(p.err(&format!("«{}» уже задан и это не таблица", k))),
        }
    }
    if find_mut(t, last).is_some() {
        return Err(p.err(&format!("ключ «{}» задан дважды", last)));
    }
    t.push((last.clone(), v));
    Ok(())
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn err(&self, msg: &str) -> String {
        let line = self.s[..self.i.min(self.s.len())].iter().filter(|&&c| c == '\n').count() + 1;
        format!("TOML, строка {}: {}", line, msg)
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }

    fn starts(&self, t: &str) -> bool {
        t.chars().enumerate().all(|(k, c)| self.peek_at(k) == Some(c))
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("ожидалось «{}»", c)))
        }
    }

    /// Пробелы и табуляции.
    fn spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.i += 1;
        }
    }

    fn comment(&mut self) {
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.i += 1;
            }
        }
    }

    /// Пустые строки, пробелы и комментарии.
    fn skip_blank_lines(&mut self) {
        loop {
            self.spaces();
            self.comment();
            match self.peek() {
                Some('\n') => self.i += 1,
                Some('\r') if self.peek_at(1) == Some('\n') => self.i += 2,
                _ => return,
            }
        }
    }

    /// После значения или заголовка — только комментарий и конец строки.
    fn end_of_line(&mut self) -> Result<(), String> {
        self.spaces();
        self.comment();
        match self.peek() {
            None => Ok(()),
            Some('\n') => {
                self.i += 1;
                Ok(())
            }
            Some('\r') if self.peek_at(1) == Some('\n') => {
                self.i += 2;
                Ok(())
            }
            Some(c) => Err(self.err(&format!("лишнее «{}» в конце строки", c))),
        }
    }

    /// Пробелы, переводы строк и комментарии (внутри массивов).
    fn ws_nl(&mut self) {
        loop {
            self.spaces();
            self.comment();
            match self.peek() {
                Some('\n' | '\r') => self.i += 1,
                _ => return,
            }
        }
    }

    /// Ключ: `a`, `"a b"`, `'a'`, составной `a."b".c`.
    fn key_path(&mut self) -> Result<Vec<String>, String> {
        let mut path = Vec::new();
        loop {
            self.spaces();
            let k = match self.peek() {
                Some('"') => {
                    if self.starts("\"\"\"") {
                        return Err(self.err("многострочная строка не может быть ключом"));
                    }
                    self.basic_string()?
                }
                Some('\'') => {
                    if self.starts("'''") {
                        return Err(self.err("многострочная строка не может быть ключом"));
                    }
                    self.literal_string()?
                }
                _ => {
                    let st = self.i;
                    while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                        self.i += 1;
                    }
                    if st == self.i {
                        return Err(self.err("ожидался ключ"));
                    }
                    self.s[st..self.i].iter().collect()
                }
            };
            path.push(k);
            self.spaces();
            if self.peek() == Some('.') {
                self.i += 1;
            } else {
                return Ok(path);
            }
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('"') if self.starts("\"\"\"") => self.multi_basic().map(Json::Str),
            Some('"') => self.basic_string().map(Json::Str),
            Some('\'') if self.starts("'''") => self.multi_literal().map(Json::Str),
            Some('\'') => self.literal_string().map(Json::Str),
            Some('[') => self.array(),
            Some('{') => self.inline_table(),
            Some('t') if self.starts("true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            Some('f') if self.starts("false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            Some(c) if c.is_ascii_digit() || c == '+' || c == '-' || c == 'i' || c == 'n' => self.number(),
            Some(c) => Err(self.err(&format!("неожиданное «{}» — ожидалось значение", c))),
            None => Err(self.err("ожидалось значение, а файл кончился")),
        }
    }

    /// `\…` в строке в двойных кавычках.
    fn escape(&mut self, out: &mut String) -> Result<(), String> {
        let c = self.peek().ok_or_else(|| self.err("строка не закрыта"))?;
        self.i += 1;
        match c {
            'b' => out.push('\u{8}'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'f' => out.push('\u{c}'),
            'r' => out.push('\r'),
            'e' => out.push('\u{1b}'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'u' | 'U' => {
                let n = if c == 'u' { 4 } else { 8 };
                let hex: String = self.s.get(self.i..self.i + n).map(|h| h.iter().collect()).unwrap_or_default();
                let code = u32::from_str_radix(&hex, 16).ok().filter(|_| hex.len() == n).ok_or_else(|| self.err("неверный \\u в строке"))?;
                out.push(char::from_u32(code).ok_or_else(|| self.err("неверный код символа"))?);
                self.i += n;
            }
            c => return Err(self.err(&format!("неизвестная последовательность «\\{}»", c))),
        }
        Ok(())
    }

    fn basic_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err("строка не закрыта")),
                Some('"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some('\\') => {
                    self.i += 1;
                    self.escape(&mut out)?;
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn literal_string(&mut self) -> Result<String, String> {
        self.expect('\'')?;
        let st = self.i;
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err("строка не закрыта")),
                Some('\'') => {
                    let s = self.s[st..self.i].iter().collect();
                    self.i += 1;
                    return Ok(s);
                }
                Some(_) => self.i += 1,
            }
        }
    }

    /// Перевод строки сразу после открывающих кавычек не входит в строку.
    fn skip_first_newline(&mut self) {
        if self.peek() == Some('\n') {
            self.i += 1;
        } else if self.starts("\r\n") {
            self.i += 2;
        }
    }

    fn multi_basic(&mut self) -> Result<String, String> {
        self.i += 3;
        self.skip_first_newline();
        let mut out = String::new();
        loop {
            if self.starts("\"\"\"") {
                // До двух кавычек перед закрывающими входят в строку: `""""" `.
                let mut n = 3;
                while self.peek_at(n) == Some('"') && n < 5 {
                    n += 1;
                }
                for _ in 3..n {
                    out.push('"');
                }
                self.i += n;
                return Ok(out);
            }
            match self.peek() {
                None => return Err(self.err("многострочная строка не закрыта")),
                Some('\\') => {
                    // `\` в конце строки — продолжение: пробелы и переводы строк пропускаются.
                    let mut k = 1;
                    while matches!(self.peek_at(k), Some(' ' | '\t')) {
                        k += 1;
                    }
                    if matches!(self.peek_at(k), Some('\n' | '\r')) {
                        self.i += k;
                        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                            self.i += 1;
                        }
                    } else {
                        self.i += 1;
                        self.escape(&mut out)?;
                    }
                }
                Some('\r') if self.peek_at(1) == Some('\n') => {
                    out.push('\n');
                    self.i += 2;
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn multi_literal(&mut self) -> Result<String, String> {
        self.i += 3;
        self.skip_first_newline();
        let mut out = String::new();
        loop {
            if self.starts("'''") {
                let mut n = 3;
                while self.peek_at(n) == Some('\'') && n < 5 {
                    n += 1;
                }
                for _ in 3..n {
                    out.push('\'');
                }
                self.i += n;
                return Ok(out);
            }
            match self.peek() {
                None => return Err(self.err("многострочная строка не закрыта")),
                Some('\r') if self.peek_at(1) == Some('\n') => {
                    out.push('\n');
                    self.i += 2;
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let st = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_')) {
            // Знак допустим только в начале и после экспоненты.
            if matches!(self.peek(), Some('+' | '-')) && self.i > st && !matches!(self.s[self.i - 1], 'e' | 'E') {
                break;
            }
            self.i += 1;
        }
        let raw: String = self.s[st..self.i].iter().collect();
        // 1979-05-27, 07:32:00 — дата или время.
        if matches!(self.peek(), Some('-' | ':')) && !raw.is_empty() && raw.chars().all(|c| c.is_ascii_digit()) {
            return Err(self.err("даты и время в файлах TV Studio не поддерживаются"));
        }
        let bad = || self.err(&format!("неверное число «{}»", raw));
        let (sign, body) = match raw.strip_prefix('-') {
            Some(b) => (-1.0, b),
            None => (1.0, raw.strip_prefix('+').unwrap_or(&raw)),
        };
        match body {
            "inf" => return Ok(Json::Num(sign * f64::INFINITY)),
            "nan" => return Ok(Json::Num(f64::NAN)),
            _ => {}
        }
        let b = body.as_bytes();
        if body.contains(':') || (b.len() >= 5 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-') {
            return Err(self.err("даты и время в файлах TV Studio не поддерживаются"));
        }
        if body.starts_with('_') || body.ends_with('_') || body.contains("__") {
            return Err(bad());
        }
        let clean = body.replace('_', "");
        for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
            if let Some(d) = clean.strip_prefix(prefix) {
                if sign < 0.0 || raw.starts_with('+') {
                    return Err(bad());
                }
                return i64::from_str_radix(d, radix).map(|n| Json::Num(n as f64)).map_err(|_| bad());
            }
        }
        // Ведущие нули запрещены (кроме самого 0 и дробной части).
        let int_part = clean.split(['.', 'e', 'E']).next().unwrap_or("");
        if int_part.len() > 1 && int_part.starts_with('0') {
            return Err(bad());
        }
        if !clean.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return Err(bad());
        }
        if clean.contains(['.', 'e', 'E']) {
            if clean.contains(".e") || clean.contains(".E") || clean.ends_with('.') {
                return Err(bad());
            }
            clean.parse::<f64>().map(|n| Json::Num(sign * n)).map_err(|_| bad())
        } else {
            clean.parse::<i64>().map(|n| Json::Num(sign * n as f64)).map_err(|_| bad())
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut v = Vec::new();
        loop {
            self.ws_nl();
            if self.peek() == Some(']') {
                self.i += 1;
                return Ok(Json::Arr(v));
            }
            v.push(self.value()?);
            self.ws_nl();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Json::Arr(v));
                }
                _ => return Err(self.err("в массиве ожидалась «,» или «]»")),
            }
        }
    }

    fn inline_table(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut t: Vec<(String, Json)> = Vec::new();
        self.spaces();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(Json::Obj(t));
        }
        loop {
            let path = self.key_path()?;
            self.spaces();
            self.expect('=')?;
            self.spaces();
            let v = self.value()?;
            insert(&mut t, &path, v, self)?;
            self.spaces();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                    self.spaces();
                }
                Some('}') => {
                    self.i += 1;
                    return Ok(Json::Obj(t));
                }
                _ => return Err(self.err("во встроенной таблице ожидалась «,» или «}»")),
            }
        }
    }
}

/// Документ файла TV Studio: TOML или (файлы старых версий) JSON —
/// JSON начинается с `{`.
pub fn parse_document(src: &str) -> Result<Json, String> {
    if src.trim_start().starts_with('{') {
        crate::json::parse(src)
    } else {
        parse(src)
    }
}

#[cfg(test)]
mod tests {
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
}
