//! Импорт готовой программы на tvision-rs в проект TV Studio.
//!
//! Что делается:
//! * исходники копируются в новый проект (вместе с модулями из
//!   `#[path = …]`, файлами из `include_str!` и данными `examples/data`);
//! * строка меню (цепочка `Menu::new().submenu(…)…`), строка состояния
//!   (`vec![StatusDef::all(vec![StatusItem::new(…), …])]`) и константы
//!   команд (`const CM_X: u16 = 100;`) переносятся в модель приложения;
//! * в коде они заменяются на `ui::menu_bar()`, `ui::status_line()` и
//!   `use ui::cmd::*` — после этого меню и команды правятся в редакторах
//!   модели, а код генерируется заново.
//!
//! Разбор — не компилятор: понимаются «прямые» записи меню и строки
//! состояния. Меню, которое собирается вспомогательными функциями или в
//! цикле, и строка состояния из нескольких `StatusDef::range` остаются в
//! коде как есть (об этом сообщает отчёт).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tvision::{Key, KeyEvent, Mods};

use crate::model::{parse_key, Action, AppModel, CommandDef, Dependency, MenuNode, Project, StatusEntry, BUILTIN_COMMANDS};

// ======================================================================
// Лексер
// ======================================================================

#[derive(Clone, Debug, PartialEq)]
enum Tk {
    Id(String),
    Str(String),
    Chr(char),
    Num(String),
    P(char),
}

#[derive(Clone, Debug)]
struct Tok {
    t: Tk,
    /// Байтовые смещения в исходнике.
    s: usize,
    e: usize,
}

fn lex(src: &str) -> Vec<Tok> {
    let b: Vec<(usize, char)> = src.char_indices().collect();
    let n = b.len();
    let end_of = |i: usize| if i < n { b[i].0 } else { src.len() };
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let (pos, c) = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // Комментарии.
        if c == '/' && i + 1 < n && b[i + 1].1 == '/' {
            while i < n && b[i].1 != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < n && b[i + 1].1 == '*' {
            let mut depth = 0;
            while i < n {
                if b[i].1 == '/' && i + 1 < n && b[i + 1].1 == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i].1 == '*' && i + 1 < n && b[i + 1].1 == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Сырые и байтовые строки: r"…", r#"…"#, b"…", br"…".
        if (c == 'r' || c == 'b') && i + 1 < n {
            let mut j = i + 1;
            if c == 'b' && j < n && b[j].1 == 'r' {
                j += 1;
            }
            let raw = c == 'r' || (j > i + 1);
            let mut hashes = 0;
            while raw && j < n && b[j].1 == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && b[j].1 == '"' && (raw || c == 'b') {
                if raw {
                    let start = j + 1;
                    let mut k = start;
                    loop {
                        if k >= n {
                            break;
                        }
                        if b[k].1 == '"' && (0..hashes).all(|h| k + 1 + h < n && b[k + 1 + h].1 == '#') {
                            break;
                        }
                        k += 1;
                    }
                    let text: String = b[start..k.min(n)].iter().map(|x| x.1).collect();
                    let e = (k + 1 + hashes).min(n);
                    out.push(Tok { t: Tk::Str(text), s: pos, e: end_of(e) });
                    i = e;
                    continue;
                }
                // b"…" — как обычная строка, со смещением на «b».
                i = j;
                let (text, e) = lex_str(&b, i);
                out.push(Tok { t: Tk::Str(text), s: pos, e: end_of(e) });
                i = e;
                continue;
            }
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < n && (b[j].1.is_alphanumeric() || b[j].1 == '_') {
                j += 1;
            }
            let w: String = b[i..j].iter().map(|x| x.1).collect();
            out.push(Tok { t: Tk::Id(w), s: pos, e: end_of(j) });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (b[j].1.is_ascii_alphanumeric() || b[j].1 == '_' || (b[j].1 == '.' && j + 1 < n && b[j + 1].1.is_ascii_digit())) {
                j += 1;
            }
            let w: String = b[i..j].iter().map(|x| x.1).collect();
            out.push(Tok { t: Tk::Num(w), s: pos, e: end_of(j) });
            i = j;
            continue;
        }
        if c == '"' {
            let (text, e) = lex_str(&b, i);
            out.push(Tok { t: Tk::Str(text), s: pos, e: end_of(e) });
            i = e;
            continue;
        }
        if c == '\'' {
            // Символ 'x', '\n', '\u{..}' — или время жизни 'a.
            if i + 1 < n && b[i + 1].1 == '\\' {
                let mut k = i + 2;
                while k < n && b[k].1 != '\'' {
                    k += 1;
                }
                let esc: String = b[i + 1..k.min(n)].iter().map(|x| x.1).collect();
                out.push(Tok { t: Tk::Chr(unescape_char(&esc)), s: pos, e: end_of(k + 1) });
                i = k + 1;
                continue;
            }
            if i + 2 < n && b[i + 2].1 == '\'' {
                out.push(Tok { t: Tk::Chr(b[i + 1].1), s: pos, e: end_of(i + 3) });
                i += 3;
                continue;
            }
            out.push(Tok { t: Tk::P('\''), s: pos, e: end_of(i + 1) });
            i += 1;
            continue;
        }
        out.push(Tok { t: Tk::P(c), s: pos, e: end_of(i + 1) });
        i += 1;
    }
    out
}

/// Обычная строка с `"` в позиции `i`. Возвращает текст и индекс за ней.
fn lex_str(b: &[(usize, char)], i: usize) -> (String, usize) {
    let n = b.len();
    let mut k = i + 1;
    let mut s = String::new();
    while k < n && b[k].1 != '"' {
        if b[k].1 == '\\' && k + 1 < n {
            let e = b[k + 1].1;
            k += 2;
            match e {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                '0' => s.push('\0'),
                '\\' => s.push('\\'),
                '"' => s.push('"'),
                '\'' => s.push('\''),
                'x' => {
                    let h: String = b[k..(k + 2).min(n)].iter().map(|x| x.1).collect();
                    k += 2;
                    s.push(u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).unwrap_or('?'));
                }
                'u' => {
                    // \u{XXXX}
                    let mut h = String::new();
                    if k < n && b[k].1 == '{' {
                        k += 1;
                        while k < n && b[k].1 != '}' {
                            h.push(b[k].1);
                            k += 1;
                        }
                        k += 1;
                    }
                    s.push(u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).unwrap_or('?'));
                }
                '\n' => {
                    // Перенос строки внутри литерала: пропускаем отступ.
                    while k < n && b[k].1.is_whitespace() {
                        k += 1;
                    }
                }
                other => s.push(other),
            }
            continue;
        }
        s.push(b[k].1);
        k += 1;
    }
    (s, (k + 1).min(n))
}

fn unescape_char(esc: &str) -> char {
    let mut it = esc.chars();
    it.next(); // '\\'
    match it.next() {
        Some('n') => '\n',
        Some('t') => '\t',
        Some('r') => '\r',
        Some('0') => '\0',
        Some('x') => u32::from_str_radix(&it.collect::<String>(), 16).ok().and_then(char::from_u32).unwrap_or('?'),
        Some('u') => {
            let h: String = it.filter(|c| c.is_ascii_hexdigit()).collect();
            u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).unwrap_or('?')
        }
        Some(c) => c,
        None => '\\',
    }
}

fn parse_int(s: &str) -> Option<i64> {
    let mut t: String = s.chars().filter(|&c| c != '_').collect();
    // Суффиксы типа: 100u16, 5i64.
    for suf in ["usize", "isize", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        if let Some(x) = t.strip_suffix(suf) {
            t = x.to_string();
            break;
        }
    }
    if let Some(h) = t.strip_prefix("0x") {
        i64::from_str_radix(h, 16).ok()
    } else {
        t.parse().ok()
    }
}

// ======================================================================
// Разбор выражений меню
// ======================================================================

#[derive(Clone, Debug)]
enum Arg {
    Str(String),
    Num(i64),
    Chr(char),
    /// Путь: `cm::OK`, `CM_ABOUT`, `hc::S_ABOUT`.
    Path(Vec<String>),
    Key(KeyEvent),
    KeyCode(Key),
    Mods(Mods),
    /// Меню и имена переменных-меню, которые в нём использованы.
    Menu(Vec<MenuNode>, Vec<String>),
}

/// Константа `const ИМЯ: тип = число;`.
#[derive(Clone, Debug)]
struct ConstDef {
    value: i64,
    /// На верхнем уровне файла (не в модуле).
    top: bool,
    /// Байтовый диапазон строк объявления (вместе с документацией).
    span: (usize, usize),
    doc: String,
}

struct Parser<'a> {
    t: &'a [Tok],
    src: &'a str,
    consts: HashMap<String, ConstDef>,
    menus: HashMap<String, (Vec<MenuNode>, (usize, usize))>,
    /// Команды, на которые ссылаются разобранные меню и строка состояния.
    used: HashSet<String>,
}

impl<'a> Parser<'a> {
    fn is_p(&self, i: usize, c: char) -> bool {
        matches!(self.t.get(i).map(|t| &t.t), Some(Tk::P(x)) if *x == c)
    }

    fn id(&self, i: usize) -> Option<&str> {
        match self.t.get(i).map(|t| &t.t) {
            Some(Tk::Id(s)) => Some(s),
            _ => None,
        }
    }

    /// `a::b::c` с позиции `i`.
    fn path(&self, mut i: usize) -> (Vec<String>, usize) {
        let mut v = Vec::new();
        while let Some(s) = self.id(i) {
            v.push(s.to_string());
            i += 1;
            if self.is_p(i, ':') && self.is_p(i + 1, ':') {
                i += 2;
            } else {
                break;
            }
        }
        (v, i)
    }

    /// Аргументы в скобках с позиции `(`.
    fn args(&self, mut i: usize) -> Option<(Vec<Arg>, usize)> {
        if !self.is_p(i, '(') {
            return None;
        }
        i += 1;
        let mut v = Vec::new();
        if self.is_p(i, ')') {
            return Some((v, i + 1));
        }
        loop {
            let (a, j) = self.expr(i)?;
            v.push(a);
            i = j;
            if self.is_p(i, ',') {
                i += 1;
                if self.is_p(i, ')') {
                    return Some((v, i + 1));
                }
            } else if self.is_p(i, ')') {
                return Some((v, i + 1));
            } else {
                return None;
            }
        }
    }

    fn expr(&self, i: usize) -> Option<(Arg, usize)> {
        let tok = self.t.get(i)?;
        match &tok.t {
            Tk::Str(s) => Some((Arg::Str(s.clone()), i + 1)),
            Tk::Num(s) => Some((Arg::Num(parse_int(s)?), i + 1)),
            Tk::Chr(c) => Some((Arg::Chr(*c), i + 1)),
            Tk::P('&') => self.expr(i + 1),
            Tk::Id(_) => {
                let (p, j) = self.path(i);
                let ps: Vec<&str> = p.iter().map(|s| s.as_str()).collect();
                match ps.as_slice() {
                    ["Menu", "new"] => {
                        let (_, j) = self.args(j).filter(|(a, _)| a.is_empty())?;
                        self.chain(Vec::new(), Vec::new(), j)
                    }
                    [var] if self.menus.contains_key(*var) && !self.is_p(j, '(') => {
                        let m = self.menus[*var].0.clone();
                        self.chain(m, vec![var.to_string()], j)
                    }
                    ["kb", name] if !self.is_p(j, '(') => Some((Arg::Key(kb_const(name)?), j)),
                    ["kb", f] => {
                        let (a, j) = self.args(j)?;
                        Some((Arg::Key(kb_fn(f, &a)?), j))
                    }
                    ["KeyEvent", "new"] => {
                        let (a, j) = self.args(j)?;
                        match a.as_slice() {
                            [Arg::KeyCode(k), Arg::Mods(m)] => Some((Arg::Key(KeyEvent::new(*k, *m)), j)),
                            _ => None,
                        }
                    }
                    ["Key", "Char"] | ["Key", "F"] => {
                        let (a, j) = self.args(j)?;
                        match (ps[1], a.as_slice()) {
                            ("Char", [Arg::Chr(c)]) => Some((Arg::KeyCode(Key::Char(*c)), j)),
                            ("F", [Arg::Num(n)]) => Some((Arg::KeyCode(Key::F(*n as u8)), j)),
                            _ => None,
                        }
                    }
                    ["Key", k] => Some((Arg::KeyCode(key_by_name(k)?), j)),
                    ["Mods", m] => Some((Arg::Mods(mods_by_name(m)?), j)),
                    _ if self.is_p(j, '(') || self.is_p(j, '!') => None,
                    _ => Some((Arg::Path(p), j)),
                }
            }
            _ => None,
        }
    }

    /// Номер команды по пути; запоминает использованные свои команды.
    fn command(&self, a: &Arg, used: &mut Vec<String>) -> Option<String> {
        match a {
            Arg::Num(n) => Some(n.to_string()),
            Arg::Path(p) if p.len() == 2 && p[0] == "cm" => {
                let name = format!("cm::{}", p[1]);
                BUILTIN_COMMANDS.iter().any(|(b, _, _)| *b == name).then_some(name)
            }
            Arg::Path(p) if p.len() == 1 => {
                let c = self.consts.get(&p[0])?;
                if !c.top || !(0..=u16::MAX as i64).contains(&c.value) {
                    return None;
                }
                used.push(p[0].clone());
                Some(p[0].clone())
            }
            _ => None,
        }
    }

    fn help(&self, a: &Arg) -> Option<i64> {
        match a {
            Arg::Num(n) => Some(*n),
            Arg::Path(p) => self.consts.get(&p.join("::")).map(|c| c.value),
            _ => None,
        }
    }

    /// Методы цепочки `.item(…).submenu(…)…` после начального меню.
    fn chain(&self, mut m: Vec<MenuNode>, mut vars: Vec<String>, mut i: usize) -> Option<(Arg, usize)> {
        let mut used = Vec::new();
        while self.is_p(i, '.') {
            let name = self.id(i + 1)?.to_string();
            let (a, j) = self.args(i + 2)?;
            match (name.as_str(), a.as_slice()) {
                ("item", [Arg::Str(t), c]) => {
                    let command = self.command(c, &mut used)?;
                    m.push(MenuNode::Item { title: t.clone(), command, key: String::new(), help: 0 });
                }
                ("item_key", [Arg::Str(t), c, Arg::Key(k), Arg::Str(param)]) => {
                    let command = self.command(c, &mut used)?;
                    m.push(MenuNode::Item { title: t.clone(), command, key: key_text(k, param), help: 0 });
                }
                ("separator", []) => m.push(MenuNode::Separator),
                ("submenu", [Arg::Str(t), Arg::Menu(children, v)]) => {
                    vars.extend(v.iter().cloned());
                    m.push(MenuNode::Sub { title: t.clone(), key: String::new(), help: 0, children: children.clone() });
                }
                ("submenu_key", [Arg::Str(t), Arg::Key(k), Arg::Menu(children, v)]) => {
                    vars.extend(v.iter().cloned());
                    m.push(MenuNode::Sub { title: t.clone(), key: key_text(k, ""), help: 0, children: children.clone() });
                }
                ("help", [h]) => {
                    let h = self.help(h)?;
                    match m.last_mut()? {
                        MenuNode::Item { help, .. } | MenuNode::Sub { help, .. } => *help = h,
                        MenuNode::Separator => {}
                    }
                }
                _ => return None,
            }
            i = j;
        }
        // Команды вложенных меню уже учтены при их разборе (`used` общий не нужен:
        // все команды берутся из итогового дерева).
        let _ = used;
        Some((Arg::Menu(m, vars), i))
    }
}

/// Текст клавиши для модели: подпись из программы, если она означает ту
/// же клавишу (сохраняем «Alt-X» как было), иначе — каноническая запись.
fn key_text(k: &KeyEvent, param: &str) -> String {
    if let Some(p) = parse_key(param) {
        if p == *k || (matches!((p.key, k.key), (Key::Char(a), Key::Char(b)) if a.eq_ignore_ascii_case(&b)) && p.mods == k.mods) {
            return param.trim().to_string();
        }
    }
    key_name(k)
}

/// Каноническое имя клавиши, понятное `parse_key`.
pub fn key_name(k: &KeyEvent) -> String {
    let mut s = String::new();
    let mut shift = k.mods.shift;
    if k.mods.ctrl {
        s.push_str("Ctrl+");
    }
    if k.mods.alt {
        s.push_str("Alt+");
    }
    let name = match k.key {
        Key::Char(' ') => "Space".to_string(),
        Key::Char(c) => c.to_uppercase().to_string(),
        Key::F(n) => format!("F{}", n),
        Key::BackTab => {
            shift = true;
            "Tab".into()
        }
        Key::PgUp => "PgUp".into(),
        Key::PgDn => "PgDn".into(),
        other => format!("{:?}", other),
    };
    if shift {
        s.push_str("Shift+");
    }
    s.push_str(&name);
    s
}

fn key_by_name(n: &str) -> Option<Key> {
    Some(match n {
        "Enter" => Key::Enter,
        "Esc" => Key::Esc,
        "Tab" => Key::Tab,
        "BackTab" => Key::BackTab,
        "Backspace" => Key::Backspace,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Home" => Key::Home,
        "End" => Key::End,
        "PgUp" => Key::PgUp,
        "PgDn" => Key::PgDn,
        "Ins" => Key::Ins,
        "Del" => Key::Del,
        _ => return None,
    })
}

fn mods_by_name(n: &str) -> Option<Mods> {
    Some(match n {
        "NONE" => Mods::NONE,
        "SHIFT" => Mods::SHIFT,
        "CTRL" => Mods::CTRL,
        "ALT" => Mods::ALT,
        "CTRL_SHIFT" => Mods::CTRL_SHIFT,
        _ => return None,
    })
}

/// Константы `kb::…`.
fn kb_const(n: &str) -> Option<KeyEvent> {
    use tvision::kb;
    Some(match n {
        "ENTER" => kb::ENTER,
        "ESC" => kb::ESC,
        "TAB" => kb::TAB,
        "SHIFT_TAB" => kb::SHIFT_TAB,
        "F1" => kb::F1,
        "F2" => kb::F2,
        "F3" => kb::F3,
        "F5" => kb::F5,
        "F6" => kb::F6,
        "F10" => kb::F10,
        "ALT_X" => kb::ALT_X,
        "ALT_F3" => kb::ALT_F3,
        "CTRL_F5" => kb::CTRL_F5,
        "SHIFT_F6" => kb::SHIFT_F6,
        _ => return None,
    })
}

/// Функции `kb::f(3)`, `kb::alt('X')`…
fn kb_fn(f: &str, a: &[Arg]) -> Option<KeyEvent> {
    use tvision::kb;
    Some(match (f, a) {
        ("f", [Arg::Num(n)]) => kb::f(*n as u8),
        ("alt_f", [Arg::Num(n)]) => kb::alt_f(*n as u8),
        ("ctrl_f", [Arg::Num(n)]) => kb::ctrl_f(*n as u8),
        ("shift_f", [Arg::Num(n)]) => kb::shift_f(*n as u8),
        ("alt", [Arg::Chr(c)]) => kb::alt(*c),
        ("ctrl", [Arg::Chr(c)]) => kb::ctrl(*c),
        ("key", [Arg::KeyCode(k)]) => kb::key(*k),
        _ => return None,
    })
}

/// Команды, на которые ссылается дерево меню.
fn menu_commands(m: &[MenuNode], out: &mut Vec<String>) {
    for it in m {
        match it {
            MenuNode::Item { command, .. } => out.push(command.clone()),
            MenuNode::Sub { children, .. } => menu_commands(children, out),
            MenuNode::Separator => {}
        }
    }
}

fn count_nodes(m: &[MenuNode]) -> usize {
    m.iter()
        .map(|n| match n {
            MenuNode::Sub { children, .. } => 1 + count_nodes(children),
            _ => 1,
        })
        .sum()
}

// ======================================================================
// Преобразование исходника
// ======================================================================

/// Отчёт об импорте.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub menu: bool,
    pub status: bool,
    pub commands: Vec<String>,
    pub notes: Vec<String>,
    pub files: Vec<String>,
    /// Были ли в программе строка меню и строка состояния.
    pub had_menu: bool,
    pub had_status: bool,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "меню: {}; строка состояния: {}; команд в модели: {}",
            if self.menu {
                "в модели"
            } else if self.had_menu {
                "осталось в коде"
            } else {
                "нет"
            },
            if self.status {
                "в модели"
            } else if self.had_status {
                "осталась в коде"
            } else {
                "нет"
            },
            self.commands.len()
        );
        for n in &self.notes {
            s.push_str("\n  ");
            s.push_str(n);
        }
        s
    }
}

/// Номер строки начала, начало строки и конец строки (с переводом строки)
/// для байтового смещения.
fn line_bounds(src: &str, pos: usize) -> (usize, usize) {
    let start = src[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = src[pos..].find('\n').map(|i| pos + i + 1).unwrap_or(src.len());
    (start, end)
}

/// Перенести меню, строку состояния и команды программы в модель.
/// Возвращает новый текст `main.rs`, модель и отчёт.
pub fn convert_source(src: &str, title: &str) -> (String, AppModel, Report) {
    let toks = lex(src);
    let mut report = Report { had_menu: src.contains(".submenu"), had_status: src.contains("StatusDef::"), ..Default::default() };
    // --- Модули: глубина фигурных скобок и стек `mod имя {` ---
    let mut in_mod: Vec<Vec<String>> = Vec::with_capacity(toks.len());
    {
        let mut stack: Vec<(String, i32)> = Vec::new();
        let mut depth = 0;
        let mut pending: Option<String> = None;
        for (i, t) in toks.iter().enumerate() {
            match &t.t {
                Tk::Id(s) if s == "mod" => {
                    if let Some(Tk::Id(name)) = toks.get(i + 1).map(|x| &x.t) {
                        if matches!(toks.get(i + 2).map(|x| &x.t), Some(Tk::P('{'))) {
                            pending = Some(name.clone());
                        }
                    }
                }
                Tk::P('{') => {
                    depth += 1;
                    if let Some(n) = pending.take() {
                        stack.push((n, depth));
                    }
                }
                Tk::P('}') => {
                    if stack.last().map(|(_, d)| *d == depth).unwrap_or(false) {
                        stack.pop();
                    }
                    depth -= 1;
                }
                _ => {}
            }
            in_mod.push(stack.iter().map(|(n, _)| n.clone()).collect());
        }
    }
    let in_tests = |i: usize| in_mod.get(i).map(|m| m.iter().any(|n| n == "tests")).unwrap_or(false);
    // Глубина фигурных скобок у каждой лексемы (для «верхнего уровня»).
    let mut brace = Vec::with_capacity(toks.len());
    {
        let mut d = 0;
        for t in &toks {
            if t.t == Tk::P('}') {
                d -= 1;
            }
            brace.push(d);
            if t.t == Tk::P('{') {
                d += 1;
            }
        }
    }

    // --- Константы ---
    let mut consts: HashMap<String, ConstDef> = HashMap::new();
    for i in 0..toks.len() {
        if toks[i].t != Tk::Id("const".into()) {
            continue;
        }
        let (Some(Tk::Id(name)), Some(Tk::P(':')), Some(Tk::Id(_ty)), Some(Tk::P('=')), Some(Tk::Num(v)), Some(Tk::P(';'))) = (
            toks.get(i + 1).map(|t| &t.t),
            toks.get(i + 2).map(|t| &t.t),
            toks.get(i + 3).map(|t| &t.t),
            toks.get(i + 4).map(|t| &t.t),
            toks.get(i + 5).map(|t| &t.t),
            toks.get(i + 6).map(|t| &t.t),
        ) else {
            continue;
        };
        let Some(value) = parse_int(v) else { continue };
        let mods = &in_mod[i];
        let key = if mods.is_empty() { name.clone() } else { format!("{}::{}", mods.join("::"), name) };
        let pub_before = i > 0 && toks[i - 1].t == Tk::Id("pub".into());
        let start_tok = if pub_before { i - 1 } else { i };
        let (mut ls, le) = line_bounds(src, toks[start_tok].s);
        let le = line_bounds(src, toks[i + 6].s).1.max(le);
        // Документация над константой.
        let mut doc = Vec::new();
        while ls > 0 {
            let (ps, _) = line_bounds(src, ls - 1);
            let line = src[ps..ls].trim();
            if let Some(d) = line.strip_prefix("///") {
                doc.insert(0, d.trim().to_string());
                ls = ps;
            } else {
                break;
            }
        }
        let top = mods.is_empty() && brace[i] == 0;
        consts.insert(key, ConstDef { value, top, span: (ls, le), doc: doc.join(" ") });
    }

    let mut p = Parser { t: &toks, src, consts, menus: HashMap::new(), used: HashSet::new() };

    // --- Переменные-меню `let x = Menu::new()…;` (по порядку, с затенением) ---
    let mut i = 0;
    while i < toks.len() {
        if toks[i].t == Tk::Id("let".into()) {
            let mut j = i + 1;
            if p.id(j) == Some("mut") {
                j += 1;
            }
            if let (Some(name), true) = (p.id(j).map(|s| s.to_string()), p.is_p(j + 1, '=')) {
                match p.expr(j + 2) {
                    Some((Arg::Menu(m, _), k)) if p.is_p(k, ';') => {
                        let (ls, _) = line_bounds(src, toks[i].s);
                        let (_, le) = line_bounds(src, toks[k].s);
                        p.menus.insert(name, (m, (ls, le)));
                    }
                    _ => {
                        p.menus.remove(&name);
                    }
                }
            }
        }
        i += 1;
    }

    // --- Главное меню: самая большая цепочка с подменю вне тестов ---
    let mut best: Option<(usize, usize, Vec<MenuNode>, Vec<String>)> = None; // (начало, конец, меню, переменные)
    for i in 0..toks.len() {
        if in_tests(i) || p.id(i) != Some("Menu") {
            continue;
        }
        let (path, _) = p.path(i);
        if path != ["Menu", "new"] {
            continue;
        }
        let Some((Arg::Menu(m, vars), k)) = p.expr(i) else { continue };
        if !m.iter().any(|n| matches!(n, MenuNode::Sub { .. })) {
            continue;
        }
        let end = toks[k - 1].e;
        if best.as_ref().map(|b| count_nodes(&b.2) < count_nodes(&m)).unwrap_or(true) {
            best = Some((toks[i].s, end, m, vars));
        }
    }
    // То же для цепочек, начинающихся с переменной-меню (`Menu::new()` внутри let).
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut model = AppModel {
        title: title.into(),
        menu: Vec::new(),
        status: Vec::new(),
        commands: Vec::new(),
        russian: src.contains("Language::Ru"),
        startup_form: String::new(),
    };
    if let Some((s, e, m, vars)) = best {
        edits.push((s, e, "ui::menu_bar()".into()));
        for v in vars {
            if let Some((_, (ls, le))) = p.menus.get(&v) {
                edits.push((*ls, *le, String::new()));
            }
        }
        let mut cmds = Vec::new();
        menu_commands(&m, &mut cmds);
        p.used.extend(cmds.into_iter().filter(|c| !c.starts_with("cm::") && c.parse::<u16>().is_err()));
        model.menu = m;
        report.menu = true;
    } else if src.contains("Menu::new()") && src.contains(".submenu") {
        report.notes.push("строка меню собирается не одной цепочкой (функции, циклы) — оставлена в коде".into());
    }

    // --- Строка состояния ---
    let mut found_status = false;
    let mut multi_status = false;
    for i in 0..toks.len() {
        if in_tests(i) || p.id(i) != Some("vec") || !p.is_p(i + 1, '!') || !p.is_p(i + 2, '[') {
            continue;
        }
        let (path, j) = p.path(i + 3);
        if path == ["StatusDef", "range"] {
            multi_status = true;
            continue;
        }
        if path != ["StatusDef", "all"] || !p.is_p(j, '(') || p.id(j + 1) != Some("vec") || !p.is_p(j + 2, '!') || !p.is_p(j + 3, '[') {
            continue;
        }
        let mut k = j + 4;
        let mut items = Vec::new();
        let mut used = Vec::new();
        let ok = loop {
            if p.is_p(k, ']') {
                break true;
            }
            let (ip, kk) = p.path(k);
            if ip != ["StatusItem", "new"] {
                break false;
            }
            let Some((a, kk)) = p.args(kk) else { break false };
            let [Arg::Str(text), Arg::Key(key), c] = a.as_slice() else { break false };
            let Some(command) = p.command(c, &mut used) else { break false };
            items.push(StatusEntry { text: text.clone(), key: key_name(key), command });
            k = kk;
            if p.is_p(k, ',') {
                k += 1;
            }
        };
        // ] ) [,] ]
        if !ok || !p.is_p(k, ']') || !p.is_p(k + 1, ')') {
            continue;
        }
        let mut z = k + 2;
        if p.is_p(z, ',') {
            z += 1;
        }
        if !p.is_p(z, ']') {
            continue;
        }
        edits.push((toks[i].s, toks[z].e, "ui::status_line()".into()));
        p.used.extend(used);
        model.status = items;
        report.status = true;
        found_status = true;
        break;
    }
    if !found_status {
        if multi_status {
            report.notes.push("строка состояния из нескольких StatusDef::range — оставлена в коде".into());
        } else if src.contains("StatusDef") {
            report.notes.push("строка состояния записана не списком StatusItem::new — оставлена в коде".into());
        }
    }

    // --- Команды: константы, на которые ссылаются меню и строка состояния ---
    let mut used: Vec<String> = p.used.iter().cloned().collect();
    used.sort_by_key(|n| (p.consts.get(n).map(|c| c.value).unwrap_or(0), n.clone()));
    for name in &used {
        if let Some(c) = p.consts.get(name) {
            model.commands.push(CommandDef { name: name.clone(), value: c.value as u16, action: Action::None, comment: c.doc.clone() });
            edits.push((c.span.0, c.span.1, String::new()));
        }
    }
    report.commands = used.clone();

    // --- Подключение модуля ui ---
    if report.menu || report.status {
        let mut at = None;
        for (i, t) in toks.iter().enumerate() {
            if t.t == Tk::Id("use".into()) && brace[i] == 0 && in_mod[i].is_empty() {
                if let Some(k) = (i..toks.len()).find(|&k| toks[k].t == Tk::P(';')) {
                    at = Some(line_bounds(src, toks[k].s).1);
                }
            }
        }
        let at = at.unwrap_or_else(|| {
            // После начальных комментариев `//!` и атрибутов `#![…]`.
            let mut pos = 0;
            for line in src.split_inclusive('\n') {
                let t = line.trim_start();
                if t.starts_with("//!") || t.starts_with("#![") || t.is_empty() {
                    pos += line.len();
                } else {
                    break;
                }
            }
            pos
        });
        let mut text = String::from("\n// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).\nmod ui;\n");
        if !used.is_empty() {
            text.push_str("#[allow(unused_imports)]\nuse ui::cmd::*;\n");
        }
        edits.push((at, at, text));
    }

    // --- Применяем правки с конца ---
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out = src.to_string();
    let mut last_start = usize::MAX;
    for (s, e, text) in edits {
        if e > last_start {
            continue; // пересечение — пропускаем (не должно случаться)
        }
        out.replace_range(s..e, &text);
        last_start = s;
    }
    let _ = p.src;
    (out, model, report)
}

// ======================================================================
// Импорт программы в проект
// ======================================================================

fn copy_dir(from: &Path, to: &Path, files: &mut Vec<String>) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {}", to.display(), e))?;
    for e in std::fs::read_dir(from).map_err(|e| format!("{}: {}", from.display(), e))?.flatten() {
        let p = e.path();
        let dst = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &dst, files)?;
        } else {
            std::fs::copy(&p, &dst).map_err(|e| format!("{}: {}", p.display(), e))?;
            files.push(dst.display().to_string());
        }
    }
    Ok(())
}

/// Относительный путь от каталога `from` до `to` (оба абсолютные).
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let f: Vec<_> = from.components().collect();
    let t: Vec<_> = to.components().collect();
    let common = f.iter().zip(t.iter()).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return to.to_path_buf();
    }
    let mut p = PathBuf::new();
    for _ in common..f.len() {
        p.push("..");
    }
    for c in &t[common..] {
        p.push(c.as_os_str());
    }
    p
}

/// Импортировать программу: `main` — главный файл (`*.rs`; если он
/// называется `main.rs`, копируется весь его каталог), проект создаётся в
/// `parent/name`. Путь `Dependency::Path` задаётся от каталога проекта.
pub fn import_program(main: &Path, parent: &Path, name: &str, dep: &Dependency) -> Result<(Project, Report), String> {
    let src = std::fs::read_to_string(main).map_err(|e| format!("{}: {}", main.display(), e))?;
    let src_dir = main.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    // Пустой проект: Cargo.toml, каталоги, файл проекта.
    let mut p = Project::create(parent, name, dep)?;
    let dir = p.dir.clone();
    let mut report = Report::default();
    let mut files = Vec::new();
    // Каталог программы целиком (если главный файл — main.rs своего каталога).
    if main.file_name().map(|f| f == "main.rs").unwrap_or(false) {
        for e in std::fs::read_dir(&src_dir).map_err(|e| e.to_string())?.flatten() {
            let path = e.path();
            if path == main {
                continue;
            }
            let dst = dir.join("src").join(e.file_name());
            if path.is_dir() {
                copy_dir(&path, &dst, &mut files)?;
            } else {
                std::fs::copy(&path, &dst).map_err(|e| format!("{}: {}", path.display(), e))?;
                files.push(dst.display().to_string());
            }
        }
    }
    // Модули из #[path = "…"] (каталог модуля) и файлы include_str!/include_bytes!.
    let toks = lex(&src);
    for (i, t) in toks.iter().enumerate() {
        match &t.t {
            Tk::Id(s) if s == "path" && i > 1 && toks[i - 1].t == Tk::P('[') => {
                if let (Some(Tk::P('=')), Some(Tk::Str(rel))) = (toks.get(i + 1).map(|x| &x.t), toks.get(i + 2).map(|x| &x.t)) {
                    let from = src_dir.join(rel);
                    if let (Some(fd), Some(rd)) = (from.parent(), Path::new(rel).parent()) {
                        let to = dir.join("src").join(rd);
                        if fd.is_dir() && fd != src_dir && !to.exists() {
                            copy_dir(fd, &to, &mut files)?;
                        } else if from.is_file() {
                            let _ = std::fs::create_dir_all(to.as_path());
                            let dst = dir.join("src").join(rel);
                            std::fs::copy(&from, &dst).map_err(|e| e.to_string())?;
                            files.push(dst.display().to_string());
                        }
                    }
                }
            }
            Tk::Id(s) if (s == "include_str" || s == "include_bytes") && toks.get(i + 1).map(|x| x.t == Tk::P('!')).unwrap_or(false) => {
                if let Some(Tk::Str(rel)) = toks.get(i + 3).map(|x| &x.t) {
                    let from = src_dir.join(rel);
                    let dst = dir.join("src").join(rel);
                    if from.is_file() && !dst.exists() {
                        if let Some(d) = dst.parent() {
                            let _ = std::fs::create_dir_all(d);
                        }
                        std::fs::copy(&from, &dst).map_err(|e| e.to_string())?;
                        files.push(dst.display().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    // Данные примеров tvision-rs: «examples/data/…» от каталога пакета.
    if src.contains("\"examples/data/") || src.contains("join(\"examples/data") {
        let data = src_dir.join("data");
        let data = if data.is_dir() { data } else { src_dir.parent().map(|d| d.join("data")).unwrap_or_default() };
        if data.is_dir() {
            copy_dir(&data, &dir.join("examples").join("data"), &mut files)?;
            report.notes.push("данные скопированы в examples/data".into());
        }
    }
    // Главный файл: меню, строка состояния и команды — в модель.
    let (code, app, conv) = convert_source(&src, name);
    std::fs::write(dir.join("src").join("main.rs"), code).map_err(|e| e.to_string())?;
    report.menu = conv.menu;
    report.status = conv.status;
    report.had_menu = conv.had_menu;
    report.had_status = conv.had_status;
    report.commands = conv.commands;
    report.notes.extend(conv.notes);
    report.files = files;
    // У импортированной программы своя модель — форма «О программе» не нужна.
    let _ = std::fs::remove_file(p.form_path("about"));
    p.app = app;
    p.forms.clear();
    p.open_files = vec!["src/main.rs".into()];
    p.save()?;
    crate::codegen::generate_all(&p, &[])?;
    Ok((p, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"//! Пример.

use tvision::*;

/// Поздороваться.
const CM_HELLO: u16 = 100;
const CM_OTHER: u16 = 101;

fn menu() -> Menu {
    let file = Menu::new().item_key("~П~ривет", CM_HELLO, kb::F2, "F2").separator().item_key("~В~ыход", cm::QUIT, kb::ALT_X, "Alt-X");
    Menu::new().submenu("~Ф~айл", file).submenu_key("~≡~", kb::alt(' '), Menu::new().item("~О~ программе", 7)).help(3)
}

fn status() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![StatusItem::new("~F2~ Привет", kb::F2, CM_HELLO), StatusItem::new("", KeyEvent::new(Key::Del, Mods::SHIFT), cm::CUT)])]
}

fn main() {
    let _ = CM_OTHER;
    let _ = (menu(), status());
}

#[cfg(test)]
mod tests {
    fn m() -> tvision::Menu { tvision::Menu::new().submenu("x", tvision::Menu::new()) }
}
"#;

    #[test]
    fn converts_menu_status_commands() {
        let (out, app, r) = convert_source(SAMPLE, "t");
        assert!(r.menu && r.status, "{:?}", r);
        assert_eq!(r.commands, vec!["CM_HELLO".to_string()]);
        assert!(out.contains("fn menu() -> Menu {\n    \n    ui::menu_bar()\n}") || out.contains("ui::menu_bar()"), "{}", out);
        assert!(!out.contains("let file"), "{}", out);
        assert!(out.contains("ui::status_line()"));
        assert!(!out.contains("const CM_HELLO"));
        assert!(!out.contains("Поздороваться"));
        assert!(out.contains("const CM_OTHER: u16 = 101;"), "чужая константа остаётся");
        assert!(out.contains("mod ui;\n#[allow(unused_imports)]\nuse ui::cmd::*;"));
        assert!(out.contains("tvision::Menu::new().submenu(\"x\""), "тесты не трогаем");
        assert_eq!(app.menu.len(), 2);
        let MenuNode::Sub { title, key, help, .. } = &app.menu[1] else { panic!() };
        assert_eq!((title.as_str(), key.as_str(), *help), ("~≡~", "Alt+Space", 3));
        let MenuNode::Sub { children, .. } = &app.menu[0] else { panic!() };
        assert_eq!(children[2], MenuNode::Item { title: "~В~ыход".into(), command: "cm::QUIT".into(), key: "Alt-X".into(), help: 0 });
        assert_eq!(app.status[1].key, "Shift+Del");
        assert_eq!(app.commands[0].comment, "Поздороваться.");
        assert_eq!(parse_key(&app.status[1].key), Some(KeyEvent::new(Key::Del, Mods::SHIFT)));
    }

    #[test]
    fn relative() {
        assert_eq!(relative_path(Path::new("/a/b/c"), Path::new("/a/x")), PathBuf::from("../../x"));
    }
}
