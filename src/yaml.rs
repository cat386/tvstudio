//! Небольшой YAML без внешних зависимостей: запись и разбор.
//!
//! Формат файлов проекта (`*.tvsp`) и форм (`*.tvf`). Работает с тем же
//! деревом значений [`Json`], что и модули [`crate::json`] и
//! [`crate::toml`]: модель переводит себя в дерево (`to_json`) и обратно
//! (`from_json`), а здесь — только текст. Порядок ключей сохраняется —
//! файлы стабильны и удобно сравниваются в системе контроля версий.
//!
//! Запись — блочный стиль с отступом 2: объекты — `ключ: значение`,
//! вложенные объекты и массивы объектов — с новой строки (`- ключ: …`),
//! массивы простых значений — в строку `[a, b]`, многострочные строки —
//! `|`-блоки. Строки, которые читались бы иначе (числа, `true`, `null`,
//! начинаются со спецсимвола, содержат `: ` или ` #`), берутся в кавычки;
//! управляющие символы — в двойных кавычках с `\xNN`.
//!
//! Разбор понимает обычный YAML 1.2 «для людей»: комментарии, `---` и
//! `...`, блочные объекты и списки (в том числе `- ключ: …` и `- - …`),
//! списки на одном уровне с ключом, строковые форматы — простые, `'…'`,
//! `"…"` (с escape-последовательностями), блоки `|` и `>` с индикаторами
//! `-`, `+` и отступа; встроенные `[…]` и `{…}` (и на нескольких
//! строках); `null`/`~`, логические, целые (`0x`, `0o`), дробные,
//! `.inf`/`.nan`. Якоря, ссылки, теги и сложные ключи (`? `) не
//! поддерживаются — разбор сообщает об этом с номером строки.

use std::fmt::Write;

use crate::json::Json;

// ======================================================================
// Запись
// ======================================================================

/// Документ YAML из значения (обычно — объекта).
pub fn to_string(v: &Json) -> String {
    let mut out = String::new();
    match v {
        Json::Obj(p) if !p.is_empty() => write_map(&mut out, p, 0),
        Json::Arr(a) if !a.is_empty() => write_seq(&mut out, a, 0),
        _ => {
            write_inline(&mut out, v, false);
            out.push('\n');
        }
    }
    out
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

/// Значение пишется в той же строке, что и ключ (или `- `)?
fn is_inline(v: &Json) -> bool {
    match v {
        Json::Obj(p) => p.is_empty(),
        Json::Arr(a) => a.is_empty() || a.iter().all(is_flow_scalar),
        _ => true,
    }
}

/// Значение, которое можно записать внутри `[…]`: не объект, не массив, без переводов строк.
fn is_flow_scalar(v: &Json) -> bool {
    match v {
        Json::Str(s) => !s.contains('\n'),
        Json::Obj(_) | Json::Arr(_) => false,
        _ => true,
    }
}

fn write_map(out: &mut String, pairs: &[(String, Json)], ind: usize) {
    for (k, v) in pairs {
        pad(out, ind);
        out.push_str(&key(k));
        out.push(':');
        write_after_indicator(out, v, ind);
    }
}

fn write_seq(out: &mut String, items: &[Json], ind: usize) {
    for v in items {
        pad(out, ind);
        out.push('-');
        match v {
            // `- ключ: …` — объект начинается в той же строке.
            Json::Obj(p) if !p.is_empty() => {
                out.push(' ');
                let mut first = String::new();
                write_map(&mut first, p, ind + 2);
                out.push_str(&first[ind + 2..]);
            }
            Json::Arr(a) if !is_inline(v) => {
                out.push(' ');
                let mut first = String::new();
                write_seq(&mut first, a, ind + 2);
                out.push_str(&first[ind + 2..]);
            }
            _ => write_after_indicator(out, v, ind),
        }
    }
}

/// После `ключ:` или `-`: значение в строке или вложенный блок.
fn write_after_indicator(out: &mut String, v: &Json, ind: usize) {
    match v {
        Json::Obj(p) if !p.is_empty() => {
            out.push('\n');
            write_map(out, p, ind + 2);
        }
        Json::Arr(a) if !is_inline(v) => {
            out.push('\n');
            write_seq(out, a, ind + 2);
        }
        Json::Str(s) if s.contains('\n') && block_ok(s) => {
            out.push(' ');
            write_block(out, s, ind + 2);
        }
        _ => {
            out.push(' ');
            write_inline(out, v, false);
            out.push('\n');
        }
    }
}

fn write_inline(out: &mut String, v: &Json, flow: bool) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) => out.push_str(&number(*n)),
        Json::Str(s) => out.push_str(&scalar(s, flow)),
        Json::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_inline(out, x, true);
            }
            out.push(']');
        }
        Json::Obj(p) => {
            out.push('{');
            for (i, (k, x)) in p.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&key(k));
                out.push_str(": ");
                write_inline(out, x, true);
            }
            out.push('}');
        }
    }
}

fn number(n: f64) -> String {
    if n.is_nan() {
        ".nan".into()
    } else if n.is_infinite() {
        if n > 0.0 { ".inf" } else { "-.inf" }.into()
    } else if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// Строку можно записать блоком `|`: без управляющих символов (кроме
/// перевода строки), без пробелов/табуляций в конце строк (их редакторы
/// теряют), первая строка не пустая и без отступа (по ней читатель
/// узнаёт отступ блока). Остальное — в двойных кавычках.
fn block_ok(s: &str) -> bool {
    let first = s.split('\n').next().unwrap_or("");
    !first.is_empty()
        && !first.starts_with(' ')
        && !s.chars().any(|c| c.is_control() && c != '\n')
        && s.split('\n').all(|l| !l.ends_with(' ') && !l.ends_with('\t'))
}

/// Блок `|`; индикатор «chomping» — по числу переводов строк в конце.
fn write_block(out: &mut String, s: &str, ind: usize) {
    let body = s.trim_end_matches('\n');
    let tail = s.len() - body.len();
    out.push('|');
    out.push_str(match tail {
        0 => "-",
        1 => "",
        _ => "+",
    });
    out.push('\n');
    for line in s.split('\n').take(body.split('\n').count() + tail.saturating_sub(1)) {
        if !line.is_empty() {
            pad(out, ind);
            out.push_str(line);
        }
        out.push('\n');
    }
}

/// Ключ: как есть, если читается обратно той же строкой, иначе — в кавычках.
fn key(k: &str) -> String {
    scalar(k, true)
}

/// Строка — простым скаляром, если это безопасно, иначе в кавычках.
fn scalar(s: &str, flow: bool) -> String {
    if plain_ok(s, flow) {
        s.to_string()
    } else if s.chars().any(|c| c.is_control()) {
        double_quoted(s)
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

fn plain_ok(s: &str, flow: bool) -> bool {
    let Some(first) = s.chars().next() else { return false };
    if s.starts_with(' ') || s.ends_with(' ') || s.chars().any(|c| c.is_control()) {
        return false;
    }
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return false;
    }
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    if flow && s.chars().any(|c| ",[]{}:".contains(c)) {
        return false;
    }
    // Читалось бы как число, null или логическое (в том числе в YAML 1.1).
    if !matches!(resolve_plain(s), Json::Str(_)) {
        return false;
    }
    let low = s.to_ascii_lowercase();
    !matches!(low.as_str(), "yes" | "no" | "on" | "off" | "=" | "<<")
}

fn double_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ======================================================================
// Разбор
// ======================================================================

/// Разобрать документ YAML.
pub fn parse(src: &str) -> Result<Json, String> {
    let mut lines: Vec<String> = Vec::new();
    // Начался ли документ (была ли «---» или значащая строка).
    let mut started = false;
    for (i, raw) in src.split('\n').enumerate() {
        let l = raw.strip_suffix('\r').unwrap_or(raw);
        let l = if i == 0 { l.trim_start_matches('\u{feff}') } else { l };
        let marker = l.trim_end() == "---" || l.starts_with("--- ");
        if !started && (l.starts_with('%') || marker) {
            if marker {
                let tail = l[3..].trim();
                if !tail.is_empty() && !tail.starts_with('#') {
                    return Err(format!("YAML, строка {}: значение после «---» не поддерживается", i + 1));
                }
                started = true;
            }
            lines.push(String::new());
            continue;
        }
        // Второй документ не нужен; «...» — конец документа.
        if (started && marker) || l.trim_end() == "..." {
            break;
        }
        if !l.trim().is_empty() && !l.trim_start().starts_with('#') {
            started = true;
        }
        lines.push(l.to_string());
    }
    let mut p = Parser { lines, i: 0 };
    p.skip_blank();
    if p.i >= p.lines.len() {
        return Ok(Json::Null);
    }
    let ind = p.indent(p.i);
    let v = p.node(ind, ind)?;
    p.skip_blank();
    if p.i < p.lines.len() {
        return Err(p.err_at(p.i, "лишний текст (неверный отступ?)"));
    }
    Ok(v)
}

struct Parser {
    lines: Vec<String>,
    i: usize,
}

impl Parser {
    fn err_at(&self, line: usize, msg: &str) -> String {
        format!("YAML, строка {}: {}", line + 1, msg)
    }

    fn indent(&self, i: usize) -> usize {
        self.lines[i].chars().take_while(|&c| c == ' ').count()
    }

    fn blank(&self, i: usize) -> bool {
        let t = self.lines[i].trim();
        t.is_empty() || t.starts_with('#')
    }

    fn skip_blank(&mut self) {
        while self.i < self.lines.len() && self.blank(self.i) {
            self.i += 1;
        }
    }

    /// Узел, первая строка которого — `self.i` с отступом `ind`; `parent` —
    /// отступ владельца (для `- значение` — отступ «-»): от него считаются
    /// отступы блоков `|` и продолжений простых строк.
    fn node(&mut self, ind: usize, parent: usize) -> Result<Json, String> {
        let line = self.lines[self.i].clone();
        if line[..ind].contains('\t') || line[ind..].starts_with('\t') {
            return Err(self.err_at(self.i, "табуляция в отступе"));
        }
        let rest = &line[ind..];
        if rest == "-" || rest.starts_with("- ") || rest.starts_with("-\t") {
            self.seq(ind)
        } else if find_colon(rest).is_some() {
            self.map(ind)
        } else {
            // Одиночное значение (корень документа или после `- `).
            let i = self.i;
            self.value_after(rest.to_string(), parent, i)
        }
    }

    fn seq(&mut self, ind: usize) -> Result<Json, String> {
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() || self.indent(self.i) != ind {
                break;
            }
            let rest = self.lines[self.i][ind..].to_string();
            if !(rest == "-" || rest.starts_with("- ") || rest.starts_with("-\t")) {
                break;
            }
            let after = rest[1..].trim_start_matches([' ', '\t']).to_string();
            let content = strip_comment(&after);
            if content.is_empty() {
                // Значение — блоком ниже (или пусто).
                self.i += 1;
                self.skip_blank();
                if self.i < self.lines.len() && self.indent(self.i) > ind {
                    let ci = self.indent(self.i);
                    items.push(self.node(ci, ind)?);
                } else {
                    items.push(Json::Null);
                }
            } else {
                // `- что-то`: заменяем «-» пробелом — получается узел с отступом содержимого.
                let col = ind + (rest.len() - after.len());
                let mut l = self.lines[self.i].clone();
                l.replace_range(ind..ind + 1, " ");
                self.lines[self.i] = l;
                items.push(self.node(col, ind)?);
            }
        }
        Ok(Json::Arr(items))
    }

    fn map(&mut self, ind: usize) -> Result<Json, String> {
        let mut pairs: Vec<(String, Json)> = Vec::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() {
                break;
            }
            let li = self.i;
            let cur = self.indent(li);
            if self.lines[li][cur..].starts_with('\t') {
                return Err(self.err_at(li, "табуляция в отступе"));
            }
            if cur != ind {
                if cur > ind {
                    return Err(self.err_at(li, "неверный отступ"));
                }
                break;
            }
            let rest = self.lines[li][ind..].to_string();
            if rest.starts_with("- ") || rest == "-" {
                break;
            }
            if rest.starts_with("? ") {
                return Err(self.err_at(li, "сложные ключи («? ») не поддерживаются"));
            }
            let Some(c) = find_colon(&rest) else { return Err(self.err_at(li, "ожидалось «ключ: значение»")) };
            let k = parse_key(rest[..c].trim_end()).map_err(|e| self.err_at(li, &e))?;
            if pairs.iter().any(|(x, _)| *x == k) {
                return Err(self.err_at(li, &format!("ключ «{}» повторяется", k)));
            }
            let after = rest[c + 1..].to_string();
            let content = strip_comment(after.trim_start_matches([' ', '\t'])).to_string();
            let v = if content.is_empty() {
                self.i += 1;
                self.skip_blank();
                if self.i < self.lines.len() {
                    let ci = self.indent(self.i);
                    let r = &self.lines[self.i][ci..];
                    let is_seq = r == "-" || r.starts_with("- ");
                    if ci > ind || (ci == ind && is_seq) {
                        self.node(ci, ind)?
                    } else {
                        Json::Null
                    }
                } else {
                    Json::Null
                }
            } else {
                self.value_after(after.trim_start_matches([' ', '\t']).to_string(), ind, li)?
            };
            pairs.push((k, v));
        }
        Ok(Json::Obj(pairs))
    }

    /// Значение, записанное после `ключ:`/`- ` в строке `li` (сама строка
    /// узла с отступом `ind`); после разбора `self.i` — на следующей строке.
    fn value_after(&mut self, text: String, ind: usize, li: usize) -> Result<Json, String> {
        let t = text.trim_start();
        if t.starts_with('&') || t.starts_with('*') || t.starts_with('!') {
            return Err(self.err_at(li, "якоря, ссылки и теги не поддерживаются"));
        }
        if t.starts_with('|') || t.starts_with('>') {
            return self.block_scalar(t, ind, li);
        }
        if t.starts_with('[') || t.starts_with('{') {
            // Встроенный список/объект — возможно, на нескольких строках.
            let mut buf = t.to_string();
            let mut j = li;
            loop {
                let mut f = Flow { s: buf.chars().collect(), i: 0 };
                match f.value() {
                    Ok(v) => {
                        f.ws();
                        if f.i < f.s.len() {
                            return Err(self.err_at(j, "лишний текст после «]» или «}»"));
                        }
                        self.i = j + 1;
                        return Ok(v);
                    }
                    Err(FlowErr::Eof) if j + 1 < self.lines.len() => {
                        j += 1;
                        buf.push('\n');
                        buf.push_str(&self.lines[j]);
                    }
                    Err(FlowErr::Eof) => return Err(self.err_at(li, "не закрыта «]» или «}»")),
                    Err(FlowErr::Msg(m)) => return Err(self.err_at(j, &m)),
                }
            }
        }
        if t.starts_with('"') || t.starts_with('\'') {
            // Строка в кавычках — возможно, продолжается на следующих строках.
            let mut buf = t.to_string();
            let mut j = li;
            loop {
                let mut f = Flow { s: buf.chars().collect(), i: 0 };
                match f.quoted() {
                    Ok(s) => {
                        f.ws();
                        let tail: String = f.s[f.i..].iter().collect();
                        let tail = tail.trim();
                        if !tail.is_empty() && !tail.starts_with('#') {
                            return Err(self.err_at(j, "лишний текст после строки в кавычках"));
                        }
                        self.i = j + 1;
                        return Ok(Json::Str(s));
                    }
                    Err(FlowErr::Eof) if j + 1 < self.lines.len() => {
                        j += 1;
                        buf.push('\n');
                        buf.push_str(&self.lines[j]);
                    }
                    Err(FlowErr::Eof) => return Err(self.err_at(li, "не закрыта кавычка")),
                    Err(FlowErr::Msg(m)) => return Err(self.err_at(j, &m)),
                }
            }
        }
        // Простой скаляр; продолжение — строки глубже (свёртываются в пробел).
        let mut s = strip_comment(t).trim_end().to_string();
        self.i = li + 1;
        loop {
            let mut j = self.i;
            while j < self.lines.len() && self.lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= self.lines.len() || self.indent(j) <= ind || self.lines[j].trim_start().starts_with('#') {
                break;
            }
            let more = self.lines[j].trim();
            if find_colon(more).is_some() {
                return Err(self.err_at(j, "неверный отступ"));
            }
            let blanks = j - self.i;
            if blanks > 0 {
                s.push_str(&"\n".repeat(blanks));
            } else {
                s.push(' ');
            }
            s.push_str(strip_comment(more).trim_end());
            self.i = j + 1;
        }
        Ok(resolve_plain(&s))
    }

    /// Блок `|` или `>` с необязательными индикаторами.
    fn block_scalar(&mut self, head: &str, ind: usize, li: usize) -> Result<Json, String> {
        let head = strip_comment(head).trim_end();
        let folded = head.starts_with('>');
        let mut chomp = ' ';
        let mut explicit: Option<usize> = None;
        for c in head[1..].chars() {
            match c {
                '-' | '+' => chomp = c,
                '1'..='9' => explicit = Some(c as usize - '0' as usize),
                _ => return Err(self.err_at(li, "неверный заголовок блока «|» / «>»")),
            }
        }
        let mut j = li + 1;
        // Отступ блока — по первой непустой строке (или из индикатора).
        let bi = match explicit {
            Some(n) => ind + n,
            None => {
                let mut k = j;
                while k < self.lines.len() && self.lines[k].trim().is_empty() {
                    k += 1;
                }
                if k < self.lines.len() {
                    self.indent(k).max(ind + 1)
                } else {
                    ind + 1
                }
            }
        };
        let mut raw: Vec<String> = Vec::new();
        while j < self.lines.len() {
            let l = &self.lines[j];
            if l.trim().is_empty() {
                raw.push(String::new());
                j += 1;
                continue;
            }
            if self.indent(j) < bi {
                break;
            }
            raw.push(l[bi..].to_string());
            j += 1;
        }
        // Пустые строки в конце — для «chomping».
        let mut trailing = 0;
        while raw.last().map(|l| l.is_empty()).unwrap_or(false) {
            raw.pop();
            trailing += 1;
        }
        self.i = j;
        let mut s = if folded {
            // Соседние строки — через пробел; пустая строка — перевод строки;
            // строки с отступом («более глубокие») не сворачиваются.
            let mut out = String::new();
            let mut prev: Option<&str> = None;
            for l in &raw {
                if let Some(p) = prev {
                    if l.is_empty() {
                        out.push('\n');
                    } else if p.is_empty() {
                    } else if p.starts_with(' ') || l.starts_with(' ') {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                }
                out.push_str(l);
                prev = Some(l);
            }
            out
        } else {
            raw.join("\n")
        };
        if !raw.is_empty() {
            match chomp {
                '-' => {}
                '+' => s.push_str(&"\n".repeat(trailing + 1)),
                _ => s.push('\n'),
            }
        }
        Ok(Json::Str(s))
    }
}

/// Позиция «:» ключа в строке: двоеточие вне кавычек и скобок, за
/// которым пробел или конец строки.
pub fn find_colon(s: &str) -> Option<usize> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let first = b.first()?.1;
    if first == '[' || first == '{' || first == '#' {
        return None;
    }
    let mut k = 0;
    // Ключ в кавычках.
    if first == '"' || first == '\'' {
        k = 1;
        while k < b.len() {
            let c = b[k].1;
            if first == '"' && c == '\\' {
                k += 2;
                continue;
            }
            if c == first {
                if first == '\'' && k + 1 < b.len() && b[k + 1].1 == '\'' {
                    k += 2;
                    continue;
                }
                break;
            }
            k += 1;
        }
        k += 1;
        while k < b.len() && b[k].1 == ' ' {
            k += 1;
        }
        return (k < b.len() && b[k].1 == ':' && (k + 1 == b.len() || b[k + 1].1 == ' ' || b[k + 1].1 == '\t')).then(|| b[k].0);
    }
    while k < b.len() {
        let c = b[k].1;
        if c == '#' && k > 0 && b[k - 1].1 == ' ' {
            return None;
        }
        if c == ':' && (k + 1 == b.len() || b[k + 1].1 == ' ' || b[k + 1].1 == '\t') {
            return Some(b[k].0);
        }
        k += 1;
    }
    None
}

fn parse_key(s: &str) -> Result<String, String> {
    if s.starts_with('"') || s.starts_with('\'') {
        let mut f = Flow { s: s.chars().collect(), i: 0 };
        return f.quoted().map_err(|_| "неверный ключ в кавычках".to_string());
    }
    if s.starts_with('&') || s.starts_with('*') || s.starts_with('!') {
        return Err("якоря, ссылки и теги не поддерживаются".into());
    }
    Ok(s.to_string())
}

/// Убрать комментарий ` # …` (вне кавычек).
fn strip_comment(s: &str) -> &str {
    let mut q: Option<char> = None;
    let mut prev = ' ';
    let mut esc = false;
    for (i, c) in s.char_indices() {
        match q {
            Some('"') if esc => esc = false,
            Some('"') if c == '\\' => esc = true,
            Some(x) if c == x => q = None,
            Some(_) => {}
            None => {
                if c == '#' && (prev == ' ' || prev == '\t' || i == 0) {
                    return s[..i].trim_end();
                }
                if (c == '"' || c == '\'') && (i == 0 || " [{,:".contains(prev)) {
                    q = Some(c);
                }
            }
        }
        prev = c;
    }
    s
}

/// Значение простого скаляра: null, логическое, число или строка.
pub fn resolve_plain(s: &str) -> Json {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Json::Null,
        "true" | "True" | "TRUE" => return Json::Bool(true),
        "false" | "False" | "FALSE" => return Json::Bool(false),
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => return Json::Num(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => return Json::Num(f64::NEG_INFINITY),
        ".nan" | ".NaN" | ".NAN" => return Json::Num(f64::NAN),
        _ => {}
    }
    let (sign, body) = match s.strip_prefix('-') {
        Some(b) => (-1.0, b),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    if let Some(h) = body.strip_prefix("0x") {
        if !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(n) = i64::from_str_radix(h, 16) {
                return Json::Num(sign * n as f64);
            }
        }
    }
    if let Some(o) = body.strip_prefix("0o") {
        if !o.is_empty() && o.chars().all(|c| ('0'..='7').contains(&c)) {
            if let Ok(n) = i64::from_str_radix(o, 8) {
                return Json::Num(sign * n as f64);
            }
        }
    }
    if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = body.parse::<f64>() {
            return Json::Num(sign * n);
        }
    }
    // Дробное: цифры, не более одной точки, необязательная экспонента.
    let looks_float = {
        let (mant, exp) = match body.find(['e', 'E']) {
            Some(p) => (&body[..p], Some(&body[p + 1..])),
            None => (body, None),
        };
        let digits = mant.chars().filter(|c| c.is_ascii_digit()).count();
        let dots = mant.chars().filter(|&c| c == '.').count();
        let mant_ok = digits > 0 && dots <= 1 && mant.chars().all(|c| c.is_ascii_digit() || c == '.') && (dots == 1 || exp.is_some());
        let exp_ok = exp.map(|e| {
            let e = e.strip_prefix(['+', '-']).unwrap_or(e);
            !e.is_empty() && e.chars().all(|c| c.is_ascii_digit())
        });
        mant_ok && exp_ok.unwrap_or(true)
    };
    if looks_float {
        if let Ok(n) = body.parse::<f64>() {
            return Json::Num(sign * n);
        }
    }
    Json::Str(s.to_string())
}

// ----------------------------------------------------------------------
// Встроенный стиль: […], {…}, строки в кавычках
// ----------------------------------------------------------------------

enum FlowErr {
    /// Текст кончился раньше, чем значение, — возможно, оно продолжается на следующей строке.
    Eof,
    Msg(String),
}

struct Flow {
    s: Vec<char>,
    i: usize,
}

impl Flow {
    /// Пробелы, переводы строк и комментарии ` # …` до конца строки.
    fn ws(&mut self) {
        loop {
            while self.i < self.s.len() && matches!(self.s[self.i], ' ' | '\t' | '\n') {
                self.i += 1;
            }
            if self.peek() == Some('#') && (self.i == 0 || matches!(self.s[self.i - 1], ' ' | '\t' | '\n')) {
                while self.i < self.s.len() && self.s[self.i] != '\n' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn value(&mut self) -> Result<Json, FlowErr> {
        self.ws();
        match self.peek() {
            None => Err(FlowErr::Eof),
            Some('[') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    match self.peek() {
                        None => return Err(FlowErr::Eof),
                        Some(']') => {
                            self.i += 1;
                            return Ok(Json::Arr(v));
                        }
                        _ => {}
                    }
                    v.push(self.value()?);
                    self.ws();
                    match self.peek() {
                        None => return Err(FlowErr::Eof),
                        Some(',') => self.i += 1,
                        Some(']') => {}
                        Some(c) => return Err(FlowErr::Msg(format!("в списке ожидалась «,» или «]», а не «{}»", c))),
                    }
                }
            }
            Some('{') => {
                self.i += 1;
                let mut v: Vec<(String, Json)> = Vec::new();
                loop {
                    self.ws();
                    match self.peek() {
                        None => return Err(FlowErr::Eof),
                        Some('}') => {
                            self.i += 1;
                            return Ok(Json::Obj(v));
                        }
                        _ => {}
                    }
                    let k = match self.peek() {
                        Some('"') | Some('\'') => self.quoted()?,
                        _ => match self.plain(true)? {
                            Json::Str(s) => s,
                            Json::Null => String::new(),
                            other => other.to_compact(),
                        },
                    };
                    self.ws();
                    let val = if self.peek() == Some(':') {
                        self.i += 1;
                        self.ws();
                        if matches!(self.peek(), Some(',') | Some('}')) {
                            Json::Null
                        } else {
                            self.value()?
                        }
                    } else {
                        Json::Null
                    };
                    v.push((k, val));
                    self.ws();
                    match self.peek() {
                        None => return Err(FlowErr::Eof),
                        Some(',') => self.i += 1,
                        Some('}') => {}
                        Some(c) => return Err(FlowErr::Msg(format!("в объекте ожидалась «,» или «}}», а не «{}»", c))),
                    }
                }
            }
            Some('"') | Some('\'') => Ok(Json::Str(self.quoted()?)),
            Some('&') | Some('*') | Some('!') => Err(FlowErr::Msg("якоря, ссылки и теги не поддерживаются".into())),
            _ => self.plain(false),
        }
    }

    /// Простой скаляр во встроенном стиле: до «,», «]», «}» (и «: » в ключе).
    fn plain(&mut self, is_key: bool) -> Result<Json, FlowErr> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == ',' || c == ']' || c == '}' {
                break;
            }
            if c == ':' && (is_key || matches!(self.s.get(self.i + 1), Some(' ') | Some(',') | Some(']') | Some('}') | None)) {
                break;
            }
            if c == '#' && self.i > start && matches!(self.s[self.i - 1], ' ' | '\t' | '\n') {
                break;
            }
            self.i += 1;
        }
        // Перенос строки внутри простого значения — пробел.
        let s: String = self.s[start..self.i].iter().collect();
        let s = s.split('\n').map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        Ok(resolve_plain(&s))
    }

    fn quoted(&mut self) -> Result<String, FlowErr> {
        let q = self.s[self.i];
        self.i += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else { return Err(FlowErr::Eof) };
            self.i += 1;
            if c == '\n' {
                // Перенос внутри кавычек: пробелы по краям строк отбрасываются,
                // один перевод строки — пробел, пустые строки — переводы строк.
                while out.ends_with([' ', '\t']) {
                    out.pop();
                }
                let mut n = 1;
                loop {
                    while matches!(self.peek(), Some(' ') | Some('\t')) {
                        self.i += 1;
                    }
                    if self.peek() == Some('\n') {
                        self.i += 1;
                        n += 1;
                    } else {
                        break;
                    }
                }
                if n == 1 {
                    out.push(' ');
                } else {
                    out.push_str(&"\n".repeat(n - 1));
                }
                continue;
            }
            if q == '\'' {
                if c == '\'' {
                    if self.peek() == Some('\'') {
                        self.i += 1;
                        out.push('\'');
                        continue;
                    }
                    return Ok(out);
                }
                out.push(c);
                continue;
            }
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let Some(e) = self.peek() else { return Err(FlowErr::Eof) };
                    self.i += 1;
                    match e {
                        'n' => out.push('\n'),
                        't' | '\t' => out.push('\t'),
                        'r' => out.push('\r'),
                        '0' => out.push('\0'),
                        'a' => out.push('\x07'),
                        'b' => out.push('\x08'),
                        'e' => out.push('\x1b'),
                        'f' => out.push('\x0c'),
                        'v' => out.push('\x0b'),
                        ' ' => out.push(' '),
                        '/' => out.push('/'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        'N' => out.push('\u{85}'),
                        '_' => out.push('\u{a0}'),
                        'L' => out.push('\u{2028}'),
                        'P' => out.push('\u{2029}'),
                        'x' | 'u' | 'U' => {
                            let n = match e {
                                'x' => 2,
                                'u' => 4,
                                _ => 8,
                            };
                            if self.i + n > self.s.len() {
                                return Err(FlowErr::Msg("неполная escape-последовательность".into()));
                            }
                            let h: String = self.s[self.i..self.i + n].iter().collect();
                            self.i += n;
                            let code =
                                u32::from_str_radix(&h, 16).map_err(|_| FlowErr::Msg(format!("неверная escape-последовательность \\{}{}", e, h)))?;
                            out.push(char::from_u32(code).ok_or_else(|| FlowErr::Msg("неверный код символа".into()))?);
                        }
                        // Перевод строки после «\» — продолжение без пробела.
                        '\n' => {
                            while matches!(self.peek(), Some(' ') | Some('\t')) {
                                self.i += 1;
                            }
                        }
                        x => return Err(FlowErr::Msg(format!("неизвестная escape-последовательность \\{}", x))),
                    }
                }
                c => out.push(c),
            }
        }
    }
}

/// Формат файла TV Studio по тексту: JSON (самые старые версии — `{`),
/// TOML (`ключ = значение` или `[таблица]` в первой значащей строке)
/// или YAML (текущий).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocFormat {
    Json,
    Toml,
    Yaml,
}

pub fn detect(src: &str) -> DocFormat {
    let first = src.lines().map(|l| l.trim_start_matches('\u{feff}').trim()).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
    if first.starts_with('{') {
        return DocFormat::Json;
    }
    if first.starts_with('[') && first.ends_with(']') && !first.contains(", ") {
        return DocFormat::Toml;
    }
    // `ключ = …` до любого «: ».
    let eq = first.find(" = ").or_else(|| first.find('='));
    let colon = find_colon(first);
    match (eq, colon) {
        (Some(e), Some(c)) if e < c => DocFormat::Toml,
        (Some(_), None) => DocFormat::Toml,
        _ => DocFormat::Yaml,
    }
}

/// Документ файла TV Studio: YAML, а также файлы прежних версий — TOML и JSON.
pub fn parse_document(src: &str) -> Result<Json, String> {
    match detect(src) {
        DocFormat::Json => crate::json::parse(src),
        DocFormat::Toml => crate::toml::parse(src),
        DocFormat::Yaml => parse(src),
    }
}

#[cfg(test)]
mod tests {
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
            "a", " ", ":", "#", "-", "'", "\"", "\n", "~", "Ф", "1", ".", ",", "[", "]", "{", "}", "|", ">", "\x03", "\t", "true", "null", "0x1",
            "*", "&", "!", "%", "@", "?", "yes", "\\", "  ", "e5",
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
}
