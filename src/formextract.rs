//! Извлечение форм из кода: диалоги, собранные в программе вызовами
//! `Dialog::centered(…)` / `Dialog::new(…)` и `d.add(…)`, превращаются в
//! формы TV Studio (`forms/*.tvf`).
//!
//! Внутри — маленький интерпретатор подмножества Rust: выражения
//! (арифметика, строки, массивы, `format!`, `vec!`, цепочки методов),
//! `let` с деструктуризацией и циклы `for … in …iter().enumerate()` /
//! `a..b` раскрываются, так что вычисляемые координаты тоже понимаются.
//! Известные виджеты tvision-rs становятся элементами формы; свои виджеты
//! программы, элементы внутри условий и всё, что не вычисляется,
//! попадают в отчёт.
//!
//! Код программы при этом не меняется — формы получаются «рядом» с ним:
//! их можно открыть в дизайнере, проверить (Shift+F9) и, если нужно,
//! перевести код на сгенерированные `forms::…::build()`.

use std::collections::HashMap;
use std::path::Path;

use crate::importer::{lex, Tk, Tok};
use crate::model::{is_ident, Control, Form, Kind, Pal, BUILTIN_COMMANDS};

// ======================================================================
// Выражения
// ======================================================================

#[derive(Clone, Debug)]
enum E {
    Num(i64),
    Str(String),
    Chr(char),
    Path(Vec<String>),
    Call(Box<E>, Vec<E>),
    Method(Box<E>, String, Vec<E>),
    #[allow(dead_code)]
    Field(Box<E>, String),
    Arr(Vec<E>),
    Tuple(Vec<E>),
    Mac(String, Vec<E>),
    Bin(&'static str, Box<E>, Box<E>),
    Neg(Box<E>),
    Range(Box<E>, Box<E>),
    Unknown,
}

struct EP<'a> {
    t: &'a [Tok],
    i: usize,
    end: usize,
    /// Не разбирать `Путь { … }` как литерал структуры (заголовок `for`).
    no_struct: bool,
}

impl<'a> EP<'a> {
    fn p(&self, c: char) -> bool {
        self.i < self.end && self.t[self.i].t == Tk::P(c)
    }
    fn p_at(&self, k: usize, c: char) -> bool {
        k < self.end && self.t[k].t == Tk::P(c)
    }
    fn id(&self) -> Option<String> {
        match self.t.get(self.i).filter(|_| self.i < self.end).map(|t| &t.t) {
            Some(Tk::Id(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Пропустить сбалансированную скобку, начиная с открывающей.
    fn skip_group(&mut self) {
        let (o, c) = match &self.t[self.i].t {
            Tk::P('(') => ('(', ')'),
            Tk::P('[') => ('[', ']'),
            Tk::P('{') => ('{', '}'),
            Tk::P('<') => ('<', '>'),
            _ => {
                self.i += 1;
                return;
            }
        };
        let mut d = 0;
        while self.i < self.end {
            if self.t[self.i].t == Tk::P(o) {
                d += 1;
            } else if self.t[self.i].t == Tk::P(c) {
                d -= 1;
                if d == 0 {
                    self.i += 1;
                    return;
                }
            }
            self.i += 1;
        }
    }

    /// Пропустить до `,` или закрывающей скобки на нулевой глубине.
    fn skip_arg(&mut self) {
        let mut d = 0;
        while self.i < self.end {
            match &self.t[self.i].t {
                Tk::P('(') | Tk::P('[') | Tk::P('{') => d += 1,
                Tk::P(')') | Tk::P(']') | Tk::P('}') => {
                    if d == 0 {
                        return;
                    }
                    d -= 1;
                }
                Tk::P(',') if d == 0 => return,
                _ => {}
            }
            self.i += 1;
        }
    }

    /// Список выражений до закрывающей `close` (открывающая уже пройдена).
    fn list(&mut self, close: char) -> Vec<E> {
        let mut v = Vec::new();
        loop {
            if self.p(close) || self.i >= self.end {
                self.i += 1;
                return v;
            }
            let save = self.i;
            let e = self.expr().unwrap_or(E::Unknown);
            if !(self.p(',') || self.p(close)) {
                self.i = save;
                self.skip_arg();
                v.push(E::Unknown);
            } else {
                v.push(e);
            }
            if self.p(',') {
                self.i += 1;
            }
        }
    }

    fn expr(&mut self) -> Option<E> {
        let a = self.bitor()?;
        if self.p('.') && self.p_at(self.i + 1, '.') {
            self.i += 2;
            if self.p('=') {
                self.i += 1;
            }
            let b = self.bitor()?;
            return Some(E::Range(Box::new(a), Box::new(b)));
        }
        Some(a)
    }

    fn bitor(&mut self) -> Option<E> {
        let mut a = self.shift()?;
        while self.p('|') && !self.p_at(self.i + 1, '|') {
            self.i += 1;
            let b = self.shift()?;
            a = E::Bin("|", Box::new(a), Box::new(b));
        }
        Some(a)
    }

    fn shift(&mut self) -> Option<E> {
        let mut a = self.add()?;
        loop {
            let op = if self.p('<') && self.p_at(self.i + 1, '<') {
                "<<"
            } else if self.p('>') && self.p_at(self.i + 1, '>') {
                ">>"
            } else {
                break;
            };
            self.i += 2;
            let b = self.add()?;
            a = E::Bin(op, Box::new(a), Box::new(b));
        }
        Some(a)
    }

    fn add(&mut self) -> Option<E> {
        let mut a = self.mul()?;
        while (self.p('+') || self.p('-')) && !self.p_at(self.i + 1, '=') {
            let op = if self.p('+') { "+" } else { "-" };
            self.i += 1;
            let b = self.mul()?;
            a = E::Bin(op, Box::new(a), Box::new(b));
        }
        Some(a)
    }

    fn mul(&mut self) -> Option<E> {
        let mut a = self.cast()?;
        while (self.p('*') || self.p('/') || self.p('%')) && !self.p_at(self.i + 1, '=') {
            let op = if self.p('*') {
                "*"
            } else if self.p('/') {
                "/"
            } else {
                "%"
            };
            self.i += 1;
            let b = self.cast()?;
            a = E::Bin(op, Box::new(a), Box::new(b));
        }
        Some(a)
    }

    fn cast(&mut self) -> Option<E> {
        let a = self.unary()?;
        while self.id().as_deref() == Some("as") {
            self.i += 1;
            // Тип: путь из идентификаторов.
            while self.id().is_some() {
                self.i += 1;
                if self.p(':') && self.p_at(self.i + 1, ':') {
                    self.i += 2;
                } else {
                    break;
                }
            }
        }
        Some(a)
    }

    fn unary(&mut self) -> Option<E> {
        if self.p('-') {
            self.i += 1;
            return Some(E::Neg(Box::new(self.unary()?)));
        }
        if self.p('&') || self.p('*') || self.p('!') {
            self.i += 1;
            if self.id().as_deref() == Some("mut") {
                self.i += 1;
            }
            return self.unary();
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Option<E> {
        let mut e = self.primary()?;
        loop {
            if self.p('(') {
                self.i += 1;
                let a = self.list(')');
                e = E::Call(Box::new(e), a);
            } else if self.p('.') && !self.p_at(self.i + 1, '.') {
                self.i += 1;
                match self.t.get(self.i).map(|t| t.t.clone()) {
                    Some(Tk::Id(name)) => {
                        self.i += 1;
                        // Турбо-рыба ::<…>
                        if self.p(':') && self.p_at(self.i + 1, ':') && self.p_at(self.i + 2, '<') {
                            self.i += 2;
                            self.skip_group();
                        }
                        if self.p('(') {
                            self.i += 1;
                            let a = self.list(')');
                            e = E::Method(Box::new(e), name, a);
                        } else {
                            e = E::Field(Box::new(e), name);
                        }
                    }
                    Some(Tk::Num(n)) => {
                        self.i += 1;
                        e = E::Field(Box::new(e), n);
                    }
                    _ => return None,
                }
            } else if self.p('[') {
                self.skip_group();
                e = E::Unknown;
            } else if self.p('?') {
                self.i += 1;
            } else {
                return Some(e);
            }
        }
    }

    fn primary(&mut self) -> Option<E> {
        if self.i >= self.end {
            return None;
        }
        match self.t[self.i].t.clone() {
            Tk::Num(n) => {
                self.i += 1;
                let t: String = n.chars().filter(|&c| c != '_').collect();
                let t = ["usize", "isize", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"]
                    .iter()
                    .find_map(|s| t.strip_suffix(s).map(|x| x.to_string()))
                    .unwrap_or(t);
                Some(t.parse::<i64>().map(E::Num).unwrap_or(E::Unknown))
            }
            Tk::Str(s) => {
                self.i += 1;
                Some(E::Str(s))
            }
            Tk::Chr(c) => {
                self.i += 1;
                Some(E::Chr(c))
            }
            Tk::P('(') => {
                self.i += 1;
                let v = self.list(')');
                Some(if v.len() == 1 { v.into_iter().next().unwrap() } else { E::Tuple(v) })
            }
            Tk::P('[') => {
                // [x; n] — повтор: не вычисляем.
                let save = self.i;
                self.i += 1;
                let v = self.list(']');
                if v.iter().any(|e| matches!(e, E::Unknown)) && self.t[save..self.i].iter().any(|t| t.t == Tk::P(';')) {
                    return Some(E::Unknown);
                }
                Some(E::Arr(v))
            }
            Tk::P('|') => {
                // Замыкание: параметры, затем тело.
                self.i += 1;
                while self.i < self.end && !self.p('|') {
                    self.i += 1;
                }
                self.i += 1;
                if self.p('{') {
                    self.skip_group();
                } else {
                    let _ = self.expr();
                }
                Some(E::Unknown)
            }
            Tk::Id(w) if w == "move" => {
                self.i += 1;
                self.primary()
            }
            Tk::Id(w) if w == "if" || w == "match" || w == "loop" || w == "while" || w == "unsafe" => None,
            Tk::Id(_) => {
                let mut p = Vec::new();
                while let Some(s) = self.id() {
                    p.push(s);
                    self.i += 1;
                    if self.p(':') && self.p_at(self.i + 1, ':') {
                        self.i += 2;
                        if self.p('<') {
                            self.skip_group();
                            if self.p(':') && self.p_at(self.i + 1, ':') {
                                self.i += 2;
                            }
                        }
                    } else {
                        break;
                    }
                }
                if self.p('!') && !self.p_at(self.i + 1, '=') {
                    self.i += 1;
                    let close = if self.p('(') {
                        ')'
                    } else if self.p('[') {
                        ']'
                    } else if self.p('{') {
                        '}'
                    } else {
                        return None;
                    };
                    self.i += 1;
                    let a = self.list(close);
                    return Some(E::Mac(p.last().cloned().unwrap_or_default(), a));
                }
                if self.p('{') && !self.no_struct && p.first().map(|s| s.chars().next().unwrap_or('a').is_uppercase()).unwrap_or(false) {
                    self.skip_group();
                    return Some(E::Unknown);
                }
                Some(E::Path(p))
            }
            _ => None,
        }
    }
}

// ======================================================================
// Значения
// ======================================================================

/// Элемент, который ещё не добавлен в диалог (или уже добавлен — в списке).
#[derive(Clone, Debug)]
struct Pend {
    c: Control,
    /// Переменная полосы прокрутки (`Some(sb)`).
    sb: Option<String>,
    /// Метка: переменная связанного элемента.
    link: Option<String>,
    /// Combo / History: переменная строки ввода.
    target: Option<String>,
    /// Для History — номер списка.
    hist: i64,
    /// Номера элементов, на которые ссылаются `sb`, `link`, `target`
    /// (разрешаются при добавлении: в цикле переменная переприсваивается).
    sb_i: Option<usize>,
    link_i: Option<usize>,
    target_i: Option<usize>,
}

#[derive(Clone, Debug)]
enum V {
    I(i64),
    S(String),
    A(Vec<V>),
    T(Vec<V>),
    Rect(i32, i32, i32, i32),
    Ctl(Box<Pend>),
    /// Модель таблицы с известными заголовками.
    Model(Vec<String>),
    /// Столбец таблицы: ширина и заголовок.
    Col(i32, Option<String>),
    U,
}

impl V {
    fn int(&self) -> Option<i64> {
        match self {
            V::I(n) => Some(*n),
            _ => None,
        }
    }
    fn str(&self) -> Option<String> {
        match self {
            V::S(s) => Some(s.clone()),
            _ => None,
        }
    }
}

fn path_name(e: &E) -> Option<String> {
    match e {
        E::Path(p) if p.len() == 1 => Some(p[0].clone()),
        _ => None,
    }
}

/// `Some(x)` → имя `x`.
fn some_var(e: &E) -> Option<String> {
    match e {
        E::Call(f, a) if matches!(&**f, E::Path(p) if p.len() == 1 && p[0] == "Some") => a.first().and_then(path_name),
        _ => None,
    }
}

/// Есть ли в выражении путь, кончающийся на `name` (например, `bf::DEFAULT`).
fn has_path_end(e: &E, name: &str) -> bool {
    match e {
        E::Path(p) => p.last().map(|s| s == name).unwrap_or(false),
        E::Bin(_, a, b) => has_path_end(a, name) || has_path_end(b, name),
        _ => false,
    }
}

fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ======================================================================
// Интерпретатор
// ======================================================================

/// Извлечённая форма и что о ней стоит знать.
#[derive(Clone, Debug)]
pub struct Extracted {
    pub form: Form,
    pub notes: Vec<String>,
    /// Имена констант-команд, которые встретились у кнопок.
    pub commands: Vec<(String, i64)>,
}

struct Run<'a> {
    t: &'a [Tok],
    src: &'a str,
    /// Текст текущего оператора (для замечаний).
    cur: String,
    globals: &'a HashMap<String, V>,
    env: HashMap<String, V>,
    dvar: Option<String>,
    form: Option<Form>,
    items: Vec<(Option<String>, Pend)>,
    /// Переменная → номер последнего добавленного под этим именем элемента.
    var_idx: HashMap<String, usize>,
    /// Замыкания `let f = |a, b| …`: параметры и тело (диапазон лексем, блок ли).
    closures: HashMap<String, (Vec<String>, usize, usize, bool)>,
    pending: HashMap<String, Pend>,
    notes: Vec<String>,
    commands: Vec<(String, i64)>,
    done: Vec<Extracted>,
    base_name: String,
}

impl<'a> Run<'a> {
    fn lookup(&self, name: &str) -> V {
        self.env.get(name).cloned().or_else(|| self.globals.get(name).cloned()).unwrap_or(V::U)
    }

    fn eval(&mut self, e: &E) -> V {
        match e {
            E::Num(n) => V::I(*n),
            E::Str(s) => V::S(s.clone()),
            E::Chr(c) => V::S(c.to_string()),
            E::Neg(x) => match self.eval(x) {
                V::I(n) => V::I(-n),
                _ => V::U,
            },
            E::Path(p) if p.len() == 1 => self.lookup(&p[0]),
            E::Path(p) => self.globals.get(&p.join("::")).cloned().unwrap_or(V::U),
            E::Bin(op, a, b) => match (self.eval(a), self.eval(b)) {
                (V::I(x), V::I(y)) => V::I(match *op {
                    "+" => x + y,
                    "-" => x - y,
                    "*" => x * y,
                    "/" if y != 0 => x / y,
                    "%" if y != 0 => x % y,
                    "|" => x | y,
                    "<<" => x << y.clamp(0, 62),
                    ">>" => x >> y.clamp(0, 62),
                    _ => return V::U,
                }),
                _ => V::U,
            },
            E::Range(a, b) => match (self.eval(a), self.eval(b)) {
                (V::I(x), V::I(y)) if y - x <= 500 => V::A((x..y).map(V::I).collect()),
                _ => V::U,
            },
            E::Arr(v) => V::A(v.iter().map(|x| self.eval(x)).collect()),
            E::Tuple(v) => V::T(v.iter().map(|x| self.eval(x)).collect()),
            E::Mac(m, a) if m == "format" => a.first().map(|x| self.eval(x)).unwrap_or(V::U),
            E::Mac(m, a) if m == "vec" => V::A(a.iter().map(|x| self.eval(x)).collect()),
            E::Method(r, m, a) => self.method(r, m, a),
            E::Call(f, a) => self.call(f, a),
            _ => V::U,
        }
    }

    fn method(&mut self, r: &E, m: &str, a: &[E]) -> V {
        let rv = self.eval(r);
        match (rv, m) {
            (V::Ctl(mut p), _) => {
                self.apply(&mut p, m, a);
                V::Ctl(p)
            }
            (V::Col(w, t), "title") => V::Col(w, a.first().and_then(|x| self.eval(x).str()).or(t)),
            (v @ V::Col(..), _) => v,
            (v @ V::Model(_), _) => v,
            (
                v,
                "iter" | "into_iter" | "to_vec" | "clone" | "to_string" | "into" | "to_owned" | "as_str" | "collect" | "copied" | "cloned" | "trim",
            ) => v,
            // .map(|s| s.to_string()) над строками — те же строки.
            (V::A(v), "map") if v.iter().all(|x| matches!(x, V::S(_))) => V::A(v),
            (V::A(v), "enumerate") => V::A(v.into_iter().enumerate().map(|(i, x)| V::T(vec![V::I(i as i64), x])).collect()),
            (V::A(v), "len") => V::I(v.len() as i64),
            (V::S(s), "len") => V::I(s.chars().count() as i64),
            (V::I(x), "max") => a.first().and_then(|y| self.eval(y).int()).map(|y| V::I(x.max(y))).unwrap_or(V::U),
            (V::I(x), "min") => a.first().and_then(|y| self.eval(y).int()).map(|y| V::I(x.min(y))).unwrap_or(V::U),
            (V::I(x), "clamp") => match (a.first().and_then(|y| self.eval(y).int()), a.get(1).and_then(|y| self.eval(y).int())) {
                (Some(lo), Some(hi)) if lo <= hi => V::I(x.clamp(lo, hi)),
                _ => V::U,
            },
            _ => V::U,
        }
    }

    fn rect(&mut self, e: Option<&E>) -> Option<(i32, i32, i32, i32)> {
        match self.eval(e?) {
            V::Rect(a, b, c, d) => Some((a, b, c, d)),
            _ => None,
        }
    }

    fn text(&mut self, e: Option<&E>) -> String {
        let Some(e) = e else { return String::new() };
        match self.eval(e) {
            V::S(s) => s,
            _ => match e {
                E::Path(p) => format!("{{{}}}", p.join("::")),
                _ => "…".into(),
            },
        }
    }

    fn strings(&mut self, e: Option<&E>) -> Vec<String> {
        match e.map(|x| self.eval(x)) {
            Some(V::A(v)) => v.into_iter().map(|x| x.str().unwrap_or_else(|| "?".into())).collect(),
            _ => vec!["?".into()],
        }
    }

    fn command(&mut self, e: Option<&E>) -> String {
        let Some(e) = e else { return "cm::OK".into() };
        if let E::Path(p) = e {
            if p.len() == 2 && p[0] == "cm" {
                let n = format!("cm::{}", p[1]);
                if BUILTIN_COMMANDS.iter().any(|(b, _, _)| *b == n) {
                    return n;
                }
            }
            if p.len() == 1 {
                if let Some(V::I(v)) = self.globals.get(&p[0]) {
                    self.commands.push((p[0].clone(), *v));
                    return p[0].clone();
                }
            }
        }
        match self.eval(e) {
            V::I(n) if (1..=65535).contains(&n) => n.to_string(),
            _ => {
                self.notes.push("у кнопки команда, которую не удалось вычислить — поставлена cm::OK".into());
                "cm::OK".into()
            }
        }
    }

    fn ctl(&self, kind: Kind, r: (i32, i32, i32, i32)) -> Control {
        let mut c = Control::new(kind, "", r.0, r.1);
        c.w = (r.2 - r.0).max(1);
        c.h = (r.3 - r.1).max(1);
        // Полосу прокрутки элемента добавим, только если она найдётся рядом.
        c.scrollbar = false;
        c.text.clear();
        c.items.clear();
        c
    }

    /// Вызов замыкания: параметры связываются с аргументами; если передан
    /// диалог (`&mut d`), внутри он доступен под именем параметра.
    fn call_closure(&mut self, name: &str, a: &[E]) -> V {
        let Some((params, cb, ce, is_block)) = self.closures.get(name).cloned() else { return V::U };
        let saved_env = self.env.clone();
        let saved_dvar = self.dvar.clone();
        for (pn, arg) in params.iter().zip(a) {
            if path_name(arg).is_some() && path_name(arg) == self.dvar {
                self.dvar = Some(pn.clone());
                continue;
            }
            let v = self.eval(arg);
            self.bind(std::slice::from_ref(pn), v);
        }
        let out = if is_block {
            self.block(cb, ce, 2);
            V::U
        } else {
            let mut ep = EP { t: self.t, i: cb, end: ce, no_struct: false };
            match ep.expr() {
                Some(e) => self.eval(&e),
                None => V::U,
            }
        };
        let pend = std::mem::take(&mut self.pending);
        self.env = saved_env;
        self.pending = pend;
        self.dvar = saved_dvar;
        out
    }

    /// Конструктор виджета tvision-rs.
    fn call(&mut self, f: &E, a: &[E]) -> V {
        if let Some(n) = path_name(f) {
            if self.closures.contains_key(&n) {
                return self.call_closure(&n, a);
            }
        }
        let E::Path(p) = f else { return V::U };
        let n = p.len();
        let last2 = if n >= 2 { (p[n - 2].as_str(), p[n - 1].as_str()) } else { ("", p.first().map(|s| s.as_str()).unwrap_or("")) };
        let pend = |c: Control| V::Ctl(Box::new(Pend { c, sb: None, link: None, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }));
        match last2 {
            ("", "Some") | ("Box", "new") => a.first().map(|x| self.eval(x)).unwrap_or(V::U),
            ("Rect", "new") => {
                let v: Vec<Option<i64>> = a.iter().map(|x| self.eval(x).int()).collect();
                match v.as_slice() {
                    [Some(a), Some(b), Some(c), Some(d)] => V::Rect(*a as i32, *b as i32, *c as i32, *d as i32),
                    _ => V::U,
                }
            }
            ("TableColumn", "new") => V::Col(a.first().and_then(|x| self.eval(x).int()).unwrap_or(12) as i32, None),
            ("VecTableModel", "new") => V::Model(self.strings(a.first())),
            ("StaticText", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::StaticText, r);
                c.text = self.text(a.get(1));
                pend(c)
            }
            ("Label", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::Label, r);
                c.text = self.text(a.get(1));
                let link = a.get(2).and_then(some_var);
                V::Ctl(Box::new(Pend { c, sb: None, link, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }))
            }
            ("Button", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::Button, r);
                c.text = self.text(a.get(1));
                c.command = self.command(a.get(2));
                c.default = a.get(3).map(|x| has_path_end(x, "DEFAULT")).unwrap_or(false);
                pend(c)
            }
            ("InputLine", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::InputLine, r);
                c.max_len = a.get(1).and_then(|x| self.eval(x).int()).unwrap_or(64).max(1);
                pend(c)
            }
            (_, "check_boxes") | (_, "radio_buttons") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(if last2.1 == "check_boxes" { Kind::CheckBoxes } else { Kind::RadioButtons }, r);
                c.items = self.strings(a.get(1));
                pend(c)
            }
            ("ListBox", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::ListBox, r);
                c.columns = a.get(1).and_then(|x| self.eval(x).int()).unwrap_or(1).clamp(1, 16);
                let sb = a.get(2).and_then(some_var);
                V::Ctl(Box::new(Pend { c, sb, link: None, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }))
            }
            ("Memo", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let c = self.ctl(Kind::Memo, r);
                let sb = a.get(2).and_then(some_var);
                V::Ctl(Box::new(Pend { c, sb, link: None, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }))
            }
            ("ScrollBar", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                pend(self.ctl(Kind::ScrollBar, r))
            }
            ("ProgressBar", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::ProgressBar, r);
                c.max = a.get(1).and_then(|x| self.eval(x).int()).unwrap_or(100).max(1);
                c.value = 0;
                pend(c)
            }
            ("Combo", "new") | ("History", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let combo = last2.0 == "Combo";
                let mut c = self.ctl(Kind::ScrollBar, r); // временно; сольётся со строкой ввода
                if combo {
                    c.items = self.strings(a.get(3));
                }
                let hist = if combo { 0 } else { a.get(2).and_then(|x| self.eval(x).int()).unwrap_or(1) };
                let target = a.get(1).and_then(path_name);
                V::Ctl(Box::new(Pend {
                    c,
                    sb: None,
                    link: None,
                    target: target.map(|t| format!("{}{}", if combo { "combo:" } else { "hist:" }, t)),
                    hist,
                    sb_i: None,
                    link_i: None,
                    target_i: None,
                }))
            }
            ("Table", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let mut c = self.ctl(Kind::Table, r);
                if let V::Model(h) = a.get(1).map(|x| self.eval(x)).unwrap_or(V::U) {
                    c.items = h;
                }
                c.text.clear();
                c.header = true;
                let sb = a.get(2).and_then(some_var);
                V::Ctl(Box::new(Pend { c, sb, link: None, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }))
            }
            ("TreeView", "new") => {
                let Some(r) = self.rect(a.first()) else { return V::U };
                let c = self.ctl(Kind::TreeView, r);
                let sb = a.get(2).and_then(some_var);
                self.notes.push("узлы дерева строятся в программе — на форме дерево пустое".into());
                V::Ctl(Box::new(Pend { c, sb, link: None, target: None, hist: 0, sb_i: None, link_i: None, target_i: None }))
            }
            _ => V::U,
        }
    }

    /// Метод, вызванный у ещё не добавленного виджета (в цепочке или отдельно).
    fn apply(&mut self, p: &mut Pend, m: &str, a: &[E]) {
        let c = &mut p.c;
        match m {
            "set_text" => c.text = self.text(a.first()),
            "set_items" => c.items = self.strings(a.first()),
            "set_value" | "focus_item" => c.value = a.first().and_then(|x| self.eval(x).int()).unwrap_or(0),
            "with_validator" => {
                c.validator = match a.first() {
                    Some(E::Call(f, args)) => match &**f {
                        E::Path(pp) if pp.last().map(|s| s == "new").unwrap_or(false) && pp.len() >= 2 => {
                            let kind = pp[pp.len() - 2].as_str();
                            match kind {
                                "FilterValidator" => self.eval(&args[0]).str().map(|s| format!("filter:{}", s)).unwrap_or_default(),
                                "PictureValidator" => self.eval(&args[0]).str().map(|s| format!("picture:{}", s)).unwrap_or_default(),
                                "RangeValidator" => match (args.first().map(|x| self.eval(x).int()), args.get(1).map(|x| self.eval(x).int())) {
                                    (Some(Some(lo)), Some(Some(hi))) => format!("range:{}..{}", lo, hi),
                                    _ => String::new(),
                                },
                                "StringLookupValidator" => format!("lookup:{}", self.strings(args.first()).join("|")),
                                _ => String::new(),
                            }
                        }
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                if c.validator.is_empty() {
                    self.notes.push(format!("своя проверка ввода у «{}» осталась в коде", c.kind.title()));
                }
            }
            "checkboxes" => c.checks = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "lines" => c.lines = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "header" => c.header = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "grid" => c.grid = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "editable" => c.editable = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "row_numbers" => c.row_numbers = a.first().map(|x| has_path_end(x, "true")).unwrap_or(true),
            "with_columns" => {
                if let Some(V::A(cols)) = a.first().map(|x| self.eval(x)) {
                    let heads = c.items.clone();
                    c.items = cols
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            let (w, t) = match v {
                                V::Col(w, t) => (*w, t.clone()),
                                _ => (12, None),
                            };
                            let title = t.or_else(|| heads.get(i).cloned()).unwrap_or_else(|| format!("Столбец {}", i + 1));
                            format!("{}:{}", title, w)
                        })
                        .collect();
                }
            }
            _ => {}
        }
    }

    /// Добавить в диалог элемент из выражения `x` под именем `var`.
    fn add(&mut self, x: &E, var: Option<String>) {
        let v = match path_name(x).and_then(|n| self.pending.remove(&n)) {
            Some(p) => V::Ctl(Box::new(p)),
            None => self.eval(x),
        };
        match v {
            V::Ctl(p) => {
                let mut p = *p;
                p.sb_i = p.sb.as_ref().and_then(|v| self.var_idx.get(v).copied());
                p.link_i = p.link.as_ref().and_then(|v| self.var_idx.get(v).copied());
                p.target_i = p.target.as_ref().and_then(|t| t.split_once(':')).and_then(|(_, v)| self.var_idx.get(v).copied());
                if let Some(v) = &var {
                    self.var_idx.insert(v.clone(), self.items.len());
                }
                self.items.push((var, p));
            }
            _ => {
                let what: String = self.cur.split_whitespace().collect::<Vec<_>>().join(" ");
                let what: String = what.chars().take(70).collect();
                self.notes.push(format!("не распознан (свой виджет или вычисляемые координаты): {}", what));
            }
        }
    }

    fn bind(&mut self, pat: &[String], v: V) {
        match (pat, v) {
            ([one], v) => {
                if let V::Ctl(p) = v {
                    self.pending.insert(one.clone(), *p);
                } else {
                    self.env.insert(one.clone(), v);
                }
            }
            (many, V::T(vs)) => {
                for (n, v) in many.iter().zip(vs) {
                    self.env.insert(n.clone(), v);
                }
            }
            (many, _) => {
                for n in many {
                    self.env.insert(n.clone(), V::U);
                }
            }
        }
    }

    /// Начать форму по конструктору диалога.
    fn start_dialog(&mut self, f: &E, a: &[E], var: &str) {
        self.finish();
        let E::Path(p) = f else { return };
        let ctor = p.last().map(|s| s.as_str()).unwrap_or("");
        let mut form = Form::new("", "");
        let title_e = if ctor == "centered" { a.get(2) } else { a.get(1) };
        if ctor == "centered" {
            // Размер, который вычисляется во время работы, — образец 60×16.
            for (k, dflt) in [(0usize, 60i64), (1, 16)] {
                let v = a.get(k).map(|x| self.eval(x)).and_then(|v| v.int());
                let v = match v {
                    Some(v) => v,
                    None => {
                        if let Some(n) = a.get(k).and_then(path_name) {
                            self.env.insert(n, V::I(dflt));
                        }
                        self.notes.push(format!("размер диалога вычисляется во время работы — взят образец {}", dflt));
                        dflt
                    }
                };
                if k == 0 {
                    form.w = v as i32;
                } else {
                    form.h = v as i32;
                }
            }
            form.centered = true;
        } else if let Some((x, y, x2, y2)) = self.rect(a.first()) {
            form.x = x;
            form.y = y;
            form.w = x2 - x;
            form.h = y2 - y;
            form.centered = false;
        }
        form.title = match title_e.map(|e| (e, self.eval(e))) {
            Some((_, V::S(s))) => s,
            Some((E::Path(p), _)) => format!("{{{}}}", p.join("::")),
            _ => String::new(),
        };
        self.form = Some(form);
        self.dvar = Some(var.to_string());
    }

    /// Закончить текущую форму: связи, полосы прокрутки, имена.
    fn finish(&mut self) {
        let Some(mut form) = self.form.take() else { return };
        let items = std::mem::take(&mut self.items);
        self.var_idx.clear();
        let mut notes = std::mem::take(&mut self.notes);
        let commands = std::mem::take(&mut self.commands);
        // Переменная → номер элемента.
        let mut ctls: Vec<Pend> = Vec::new();
        let mut var_of: HashMap<String, usize> = HashMap::new();
        let mut removed = vec![false; items.len()];
        for (i, (var, p)) in items.iter().enumerate() {
            if let Some(v) = var {
                var_of.insert(v.clone(), i);
            }
            ctls.push(p.clone());
        }
        // Полоса прокрутки рядом со списком/полем/таблицей/деревом — внутрь элемента.
        for i in 0..ctls.len() {
            let Some(j) = ctls[i].sb_i else { continue };
            let (c, s) = (&ctls[i].c, &ctls[j].c);
            if s.kind == Kind::ScrollBar && s.x == c.x + c.w && s.y == c.y && s.h == c.h && s.w == 1 {
                ctls[i].c.w += 1;
                ctls[i].c.scrollbar = true;
                removed[j] = true;
            }
        }
        // Combo и History — к своей строке ввода.
        for i in 0..ctls.len() {
            let Some(t) = ctls[i].target.clone() else { continue };
            removed[i] = true;
            let combo = t.starts_with("combo:");
            let Some(j) = ctls[i].target_i else { continue };
            let side = ctls[i].c.clone();
            let hist = ctls[i].hist;
            let il = &mut ctls[j].c;
            if il.kind == Kind::InputLine && side.y == il.y && side.x == il.x + il.w {
                il.w += side.w;
                if combo {
                    il.kind = Kind::ComboBox;
                    il.items = side.items.clone();
                } else {
                    il.history = hist;
                }
            } else {
                notes.push("кнопка списка/истории не рядом со строкой ввода — пропущена".into());
            }
        }
        // Имена: из переменных программы, иначе — по виду элемента (button1, button2…).
        let mut names: Vec<String> = Vec::new();
        for (var, p) in items.iter() {
            let named = var.clone().filter(|v| is_ident(v));
            let name = match named {
                Some(v) if !names.contains(&v) => v,
                Some(v) => (2..).map(|k| format!("{}{}", v, k)).find(|n| !names.contains(n)).unwrap(),
                None => (1..).map(|k| format!("{}{}", p.c.kind.prefix(), k)).find(|n| !names.contains(n)).unwrap(),
            };
            names.push(name);
        }
        for (i, p) in ctls.iter().enumerate() {
            if removed[i] {
                continue;
            }
            let mut c = p.c.clone();
            c.name = names[i].clone();
            if c.kind == Kind::Label {
                c.link = p.link_i.filter(|&j| !removed[j]).map(|j| names[j].clone()).unwrap_or_default();
            }
            form.controls.push(c);
        }
        // Метки ссылаются только на оставшиеся элементы.
        let alive: Vec<String> = form.controls.iter().map(|c| c.name.clone()).collect();
        for c in form.controls.iter_mut() {
            if !c.link.is_empty() && !alive.contains(&c.link) {
                c.link.clear();
            }
        }
        notes.sort();
        notes.dedup();
        form.name = self.base_name.clone();
        self.done.push(Extracted { form, notes, commands });
        self.dvar = None;
    }

    /// Парная закрывающая `}` для `{` в позиции `k`.
    fn match_brace(&self, k: usize, b: usize) -> usize {
        let mut d = 0;
        let mut m = k;
        while m < b {
            match self.t[m].t {
                Tk::P('{') => d += 1,
                Tk::P('}') => {
                    d -= 1;
                    if d == 0 {
                        return m;
                    }
                }
                _ => {}
            }
            m += 1;
        }
        b
    }

    /// Выполнить операторы блока `[a, b)`.
    fn block(&mut self, a: usize, b: usize, depth: usize) {
        let mut i = a;
        while i < b {
            let start = i;
            let first = match &self.t[i].t {
                Tk::Id(s) => s.clone(),
                _ => String::new(),
            };
            let block_stmt = matches!(first.as_str(), "for" | "if" | "while" | "loop" | "match" | "unsafe") || self.t[i].t == Tk::P('{');
            if block_stmt {
                // Первый `{` на нулевой глубине скобок — тело; у if — ещё else-цепочка.
                let mut d = 0;
                let mut k = i;
                while k < b {
                    match self.t[k].t {
                        Tk::P('(') | Tk::P('[') => d += 1,
                        Tk::P(')') | Tk::P(']') => d -= 1,
                        Tk::P('{') if d == 0 => break,
                        _ => {}
                    }
                    k += 1;
                }
                let body_open = k;
                let mut close = self.match_brace(k, b);
                let body = (body_open + 1, close);
                // else { … } / else if … { … }
                while matches!(self.t.get(close + 1).map(|t| &t.t), Some(Tk::Id(s)) if s == "else") {
                    let mut m = close + 2;
                    while m < b && self.t[m].t != Tk::P('{') {
                        m += 1;
                    }
                    close = self.match_brace(m, b);
                }
                let mut end = close + 1;
                if self.t.get(end).map(|t| t.t == Tk::P(';')).unwrap_or(false) {
                    end += 1;
                }
                if first == "for" {
                    self.for_loop(start, body, depth);
                } else if self.dvar.is_some() && self.mentions_add(start, end.min(b)) {
                    self.notes.push("часть элементов добавляется внутри условия — пропущена".into());
                }
                i = end.max(start + 1);
                continue;
            }
            // Обычный оператор — до `;` на нулевой глубине.
            let mut d = 0;
            let mut k = i;
            while k < b {
                match self.t[k].t {
                    Tk::P('(') | Tk::P('[') | Tk::P('{') => d += 1,
                    Tk::P(')') | Tk::P(']') | Tk::P('}') => d -= 1,
                    Tk::P(';') if d == 0 => break,
                    _ => {}
                }
                k += 1;
            }
            self.stmt(start, k.min(b));
            i = k + 1;
        }
    }

    /// Есть ли в диапазоне `dvar.add(`.
    fn mentions_add(&self, a: usize, b: usize) -> bool {
        let Some(dv) = &self.dvar else { return false };
        (a..b.saturating_sub(2))
            .any(|k| self.t[k].t == Tk::Id(dv.clone()) && self.t[k + 1].t == Tk::P('.') && self.t[k + 2].t == Tk::Id("add".into()))
    }

    fn for_loop(&mut self, start: usize, body: (usize, usize), depth: usize) {
        if depth > 3 {
            return;
        }
        // for ШАБЛОН in ВЫРАЖЕНИЕ {
        let mut k = start + 1;
        let mut pat = Vec::new();
        while k < body.0 - 1 && self.t[k].t != Tk::Id("in".into()) {
            if let Tk::Id(s) = &self.t[k].t {
                if s != "mut" {
                    pat.push(s.clone());
                }
            }
            k += 1;
        }
        let mut ep = EP { t: self.t, i: k + 1, end: body.0 - 1, no_struct: true };
        let Some(e) = ep.expr() else { return };
        let V::A(items) = self.eval(&e) else {
            if self.dvar.is_some() && self.mentions_add(body.0, body.1) {
                self.notes.push("элементы добавляются в цикле по данным программы — пропущены".into());
            }
            return;
        };
        for v in items.into_iter().take(200) {
            self.bind(&pat, v);
            self.block(body.0, body.1, depth + 1);
        }
    }

    /// Обычный оператор `[a, b)` (с `;` в конце или без).
    fn stmt(&mut self, a: usize, b: usize) {
        let b = if b > a && self.t[b - 1].t == Tk::P(';') { b - 1 } else { b };
        if a >= b {
            return;
        }
        if !self.src.is_empty() {
            self.cur = self.src[self.t[a].s..self.t[b - 1].e].to_string();
        }
        // let ШАБЛОН [: Тип] = ВЫРАЖЕНИЕ
        if self.t[a].t == Tk::Id("let".into()) {
            let mut k = a + 1;
            let mut pat = Vec::new();
            let mut d = 0;
            while k < b {
                match &self.t[k].t {
                    Tk::P('(') => d += 1,
                    Tk::P(')') => d -= 1,
                    Tk::P(':') if d == 0 => {
                        // Тип: пропускаем до `=`.
                        while k < b && self.t[k].t != Tk::P('=') {
                            k += 1;
                        }
                        break;
                    }
                    Tk::P('=') if d == 0 => break,
                    Tk::Id(s) if s != "mut" && s != "ref" => pat.push(s.clone()),
                    _ => {}
                }
                k += 1;
            }
            if k >= b {
                return;
            }
            // let f = |a, b| тело — запоминаем замыкание.
            let mut cs = k + 1;
            if self.t[cs].t == Tk::Id("move".into()) {
                cs += 1;
            }
            if cs < b && self.t[cs].t == Tk::P('|') {
                let mut m = cs + 1;
                let mut params = Vec::new();
                let mut expect_name = true;
                let mut d = 0;
                while m < b && !(self.t[m].t == Tk::P('|') && d == 0) {
                    match &self.t[m].t {
                        Tk::P('(') | Tk::P('<') | Tk::P('[') => d += 1,
                        Tk::P(')') | Tk::P('>') | Tk::P(']') => d -= 1,
                        Tk::P(',') if d == 0 => expect_name = true,
                        Tk::P(':') if d == 0 => expect_name = false,
                        Tk::Id(n) if expect_name && n != "mut" => {
                            params.push(n.clone());
                            expect_name = false;
                        }
                        _ => {}
                    }
                    m += 1;
                }
                let body = m + 1;
                if let Some(name) = pat.first() {
                    if body < b && self.t[body].t == Tk::P('{') {
                        let close = self.match_brace(body, b + 1);
                        self.closures.insert(name.clone(), (params, body + 1, close, true));
                    } else {
                        self.closures.insert(name.clone(), (params, body, b, false));
                    }
                }
                return;
            }
            let mut ep = EP { t: self.t, i: k + 1, end: b, no_struct: false };
            let Some(e) = ep.expr() else {
                for n in &pat {
                    self.env.insert(n.clone(), V::U);
                    self.pending.remove(n);
                }
                return;
            };
            // Новый диалог?
            if let E::Call(f, args) = &e {
                if matches!(&**f, E::Path(p) if p.len() >= 2 && p[p.len() - 2] == "Dialog" && (p[p.len() - 1] == "centered" || p[p.len() - 1] == "new"))
                {
                    if let Some(var) = pat.first() {
                        self.start_dialog(f, args, var);
                    }
                    return;
                }
            }
            if let Some((x, var)) = self.as_add(&e).map(|x| (x, pat.first().cloned())) {
                self.add(&x, var);
                return;
            }
            let v = self.eval(&e);
            self.bind(&pat, v);
            return;
        }
        // Присваивание: X.поле = …, X.поле |= …
        let assign = (a..b).find(|&k| self.t[k].t == Tk::P('=') && !matches!(self.t.get(k + 1).map(|t| &t.t), Some(Tk::P('='))));
        if let Some(k) = assign {
            let op = if k > a { self.t[k - 1].t.clone() } else { Tk::P(' ') };
            let lhs: Vec<String> = self.t[a..k].iter().filter_map(|t| if let Tk::Id(s) = &t.t { Some(s.clone()) } else { None }).collect();
            let mut ep = EP { t: self.t, i: k + 1, end: b, no_struct: false };
            let rhs = ep.expr();
            if let (Some(first), Some(rhs)) = (lhs.first(), rhs) {
                let last = lhs.last().map(|s| s.as_str()).unwrap_or("");
                if Some(first) == self.dvar.as_ref() {
                    let v = self.eval(&rhs);
                    if let Some(f) = self.form.as_mut() {
                        if last == "help_ctx" {
                            f.help_ctx = v.int().unwrap_or(0);
                        } else if last == "options" && op == Tk::P('|') && has_path_end(&rhs, "CENTERED") {
                            f.centered = true;
                        }
                    }
                } else if self.pending.contains_key(first) {
                    let v = self.eval(&rhs);
                    let p = self.pending.get_mut(first).unwrap();
                    match last {
                        "value" => p.c.value = v.int().unwrap_or(0),
                        "state" if has_path_end(&rhs, "DISABLED") => p.c.disabled = true,
                        "help_ctx" => p.c.help_ctx = v.int().unwrap_or(0),
                        _ => {}
                    }
                } else if lhs.len() == 1 {
                    let v = self.eval(&rhs);
                    self.env.insert(first.clone(), v);
                }
            }
            return;
        }
        let mut ep = EP { t: self.t, i: a, end: b, no_struct: false };
        let Some(e) = ep.expr() else { return };
        if let Some(x) = self.as_add(&e) {
            self.add(&x, None);
            return;
        }
        // Вызов известного замыкания.
        if let E::Call(f, args) = &e {
            if let Some(n) = path_name(f).filter(|n| self.closures.contains_key(n)) {
                self.call_closure(&n, args);
                return;
            }
        }
        // `bars.push(d.add(bar))` — добавление внутри другого вызова.
        if let E::Method(_, _, args) | E::Call(_, args) = &e {
            let inner: Vec<E> = args.iter().filter_map(|x| self.as_add(x)).collect();
            if !inner.is_empty() {
                for x in inner {
                    self.add(&x, None);
                }
                return;
            }
        }
        // Диалог передан в свою функцию (`&mut d`) — её элементов мы не видим.
        if let Some(dv) = &self.dvar {
            if self.cur.contains(&format!("&mut {}", dv)) {
                let what: String = self.cur.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(70).collect();
                self.notes.push(format!("диалог передаётся в функцию — её элементы пропущены: {}", what));
            }
        }
        if let E::Method(r, m, args) = &e {
            if let Some(v) = path_name(r) {
                if Some(&v) == self.dvar.as_ref() && m == "set_palette" {
                    if let Some(f) = self.form.as_mut() {
                        f.palette = if args.first().map(|x| has_path_end(x, "Blue")).unwrap_or(false) {
                            Pal::Blue
                        } else if args.first().map(|x| has_path_end(x, "Cyan")).unwrap_or(false) {
                            Pal::Cyan
                        } else {
                            Pal::Gray
                        };
                    }
                } else if let Some(mut p) = self.pending.remove(&v) {
                    self.apply(&mut p, m, args);
                    self.pending.insert(v, p);
                }
            }
        }
    }

    /// `dvar.add(x)` → `x`.
    fn as_add(&self, e: &E) -> Option<E> {
        match e {
            E::Method(r, m, a) if m == "add" && path_name(r).as_ref() == self.dvar.as_ref() && self.dvar.is_some() => a.first().cloned(),
            _ => None,
        }
    }
}

/// Константы файла: числа и массивы строк (`NAME`, `mod::NAME`).
fn file_consts(t: &[Tok]) -> HashMap<String, V> {
    let mut out = HashMap::new();
    let mut stack: Vec<(String, i32)> = Vec::new();
    let mut depth = 0;
    let mut pending_mod: Option<String> = None;
    let mut i = 0;
    while i < t.len() {
        match &t[i].t {
            Tk::Id(s) if s == "mod" => {
                if let (Some(Tk::Id(n)), Some(Tk::P('{'))) = (t.get(i + 1).map(|x| &x.t), t.get(i + 2).map(|x| &x.t)) {
                    pending_mod = Some(n.clone());
                }
            }
            Tk::P('{') => {
                depth += 1;
                if let Some(n) = pending_mod.take() {
                    stack.push((n, depth));
                }
            }
            Tk::P('}') => {
                if stack.last().map(|(_, d)| *d == depth).unwrap_or(false) {
                    stack.pop();
                }
                depth -= 1;
            }
            Tk::Id(s) if (s == "const" || s == "static") => {
                if let Some(Tk::Id(name)) = t.get(i + 1).map(|x| &x.t) {
                    // До `=`, затем значение до `;`.
                    // `;` внутри типа `[&str; 3]` — не конец объявления.
                    let mut k = i + 2;
                    let mut d = 0;
                    while k < t.len() {
                        match t[k].t {
                            Tk::P('[') | Tk::P('(') => d += 1,
                            Tk::P(']') | Tk::P(')') => d -= 1,
                            Tk::P('=') | Tk::P(';') if d == 0 => break,
                            _ => {}
                        }
                        k += 1;
                    }
                    let mut e = k + 1;
                    let mut d = 0;
                    while e < t.len() {
                        match t[e].t {
                            Tk::P('[') | Tk::P('(') | Tk::P('{') => d += 1,
                            Tk::P(']') | Tk::P(')') | Tk::P('}') => d -= 1,
                            Tk::P(';') if d == 0 => break,
                            _ => {}
                        }
                        e += 1;
                    }
                    if k < t.len() && t[k].t == Tk::P('=') {
                        let mut ep = EP { t, i: k + 1, end: e, no_struct: true };
                        if let Some(x) = ep.expr() {
                            let globals = HashMap::new();
                            let mut r = Run {
                                t,
                                src: "",
                                cur: String::new(),
                                globals: &globals,
                                env: HashMap::new(),
                                dvar: None,
                                form: None,
                                items: vec![],
                                var_idx: HashMap::new(),
                                closures: HashMap::new(),
                                pending: HashMap::new(),
                                notes: vec![],
                                commands: vec![],
                                done: vec![],
                                base_name: String::new(),
                            };
                            let v = r.eval(&x);
                            if !matches!(v, V::U) {
                                let key = if stack.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{}::{}", stack.iter().map(|s| s.0.clone()).collect::<Vec<_>>().join("::"), name)
                                };
                                out.insert(key, v);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Имя формы по окружению диалога: функция, а для `new`/`run`… — тип `impl`.
fn context_name(t: &[Tok], at: usize) -> String {
    let mut fname = None;
    let mut impl_name = None;
    for k in (0..at).rev() {
        if fname.is_none() && t[k].t == Tk::Id("fn".into()) {
            if let Some(Tk::Id(n)) = t.get(k + 1).map(|x| &x.t) {
                fname = Some(n.clone());
            }
        }
        if t[k].t == Tk::Id("impl".into()) {
            // impl Тип / impl Типаж for Тип
            let mut m = k + 1;
            let mut last = None;
            while m < at && t[m].t != Tk::P('{') {
                if let Tk::Id(n) = &t[m].t {
                    last = Some(n.clone());
                }
                m += 1;
            }
            impl_name = last;
            break;
        }
    }
    let generic = ["new", "build", "run", "main", "handle_event", "default", "show", "exec", "open"];
    match (fname, impl_name) {
        (Some(f), Some(i)) if generic.contains(&f.as_str()) => to_snake(&i),
        (Some(f), _) if !generic.contains(&f.as_str()) => f,
        (_, Some(i)) => to_snake(&i),
        (Some(f), None) => f,
        _ => "dialog".into(),
    }
}

/// Найти диалоги в исходнике. `globals` — константы других файлов
/// программы (например, главного), чтобы понимать команды кнопок.
pub fn extract(src: &str, extra_consts: &HashMap<String, i64>) -> Vec<Extracted> {
    let t = lex(src);
    let mut globals = file_consts(&t);
    for (k, v) in extra_consts {
        globals.entry(k.clone()).or_insert(V::I(*v));
    }
    // Тесты не смотрим: диалоги там собираются для проверок.
    let test_start = src.find("#[cfg(test)]").unwrap_or(usize::MAX);
    let mut out = Vec::new();
    let mut seen_blocks = Vec::new();
    for i in 0..t.len() {
        if t[i].s >= test_start {
            break;
        }
        // Dialog :: centered | new
        if !(t[i].t == Tk::Id("Dialog".into())
            && t.get(i + 1).map(|x| x.t == Tk::P(':')).unwrap_or(false)
            && matches!(t.get(i + 3).map(|x| &x.t), Some(Tk::Id(s)) if s == "centered" || s == "new"))
        {
            continue;
        }
        // Охватывающий блок `{ … }`.
        let mut d = 0;
        let mut open = None;
        for k in (0..i).rev() {
            match t[k].t {
                Tk::P('}') => d += 1,
                Tk::P('{') => {
                    if d == 0 {
                        open = Some(k);
                        break;
                    }
                    d -= 1;
                }
                _ => {}
            }
        }
        let Some(open) = open else { continue };
        if seen_blocks.contains(&open) {
            continue;
        }
        seen_blocks.push(open);
        let mut d = 0;
        let mut close = t.len();
        for (k, tok) in t.iter().enumerate().skip(open) {
            match tok.t {
                Tk::P('{') => d += 1,
                Tk::P('}') => {
                    d -= 1;
                    if d == 0 {
                        close = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        let mut r = Run {
            t: &t,
            src,
            cur: String::new(),
            globals: &globals,
            env: HashMap::new(),
            dvar: None,
            form: None,
            items: vec![],
            var_idx: HashMap::new(),
            closures: HashMap::new(),
            pending: HashMap::new(),
            notes: vec![],
            commands: vec![],
            done: vec![],
            base_name: context_name(&t, i),
        };
        r.block(open + 1, close, 0);
        r.finish();
        out.extend(r.done.into_iter().filter(|x| !x.form.controls.is_empty()));
    }
    out
}

/// Числовые константы файла (для команд кнопок в других файлах).
pub fn int_consts(src: &str) -> HashMap<String, i64> {
    file_consts(&lex(src)).into_iter().filter_map(|(k, v)| v.int().map(|n| (k, n))).collect()
}

/// Есть ли смысл искать в файле диалоги (быстрая проверка).
pub fn has_dialogs(path: &Path) -> bool {
    std::fs::read_to_string(path).map(|s| s.contains("Dialog::centered") || s.contains("Dialog::new")).unwrap_or(false)
}

// ======================================================================
// Формы — в проект
// ======================================================================

/// Отчёт: какие формы добавлены и что в них не попало.
#[derive(Clone, Debug, Default)]
pub struct FormsReport {
    /// (имя формы, файл программы, замечания).
    pub added: Vec<(String, String, Vec<String>)>,
    /// Формы, которые уже были в проекте.
    pub existing: Vec<String>,
}

impl FormsReport {
    pub fn summary(&self) -> String {
        let mut s = format!("форм добавлено: {}", self.added.len());
        if !self.existing.is_empty() {
            s.push_str(&format!(" (уже были: {})", self.existing.join(", ")));
        }
        for (f, file, notes) in &self.added {
            s.push_str(&format!("\n  {} ← {}", f, file));
            for n in notes {
                s.push_str(&format!("\n      · {}", n));
            }
        }
        s
    }
}

fn rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            if p.file_name().map(|n| n != "forms").unwrap_or(true) {
                rs_files(&p, out);
            }
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// Убрать из `main.rs` объявление `const ИМЯ: u16 = …;` (с документацией).
fn remove_const(src: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| {
        let t = l.trim_start_matches("pub ");
        t.starts_with(&format!("const {}:", name)) && t.trim_end().ends_with(';')
    })?;
    let mut from = at;
    while from > 0 && lines[from - 1].trim_start().starts_with("///") {
        from -= 1;
    }
    let mut out: String = lines[..from].concat();
    out.push_str(&lines[at + 1..].concat());
    Some(out)
}

/// Подключить в `main.rs` модули `ui` и `forms` (и команды `ui::cmd`).
fn ensure_mods(src: &str, with_cmds: bool) -> String {
    let mut s = src.to_string();
    let has = |s: &str, l: &str| s.lines().any(|x| x.trim() == l);
    let mut add = String::new();
    if !has(&s, "mod forms;") {
        add.push_str("mod forms;\n");
    }
    if !has(&s, "mod ui;") {
        add.push_str("mod ui;\n");
    }
    if with_cmds && !has(&s, "use ui::cmd::*;") {
        add.push_str("#[allow(unused_imports)]\nuse ui::cmd::*;\n");
    }
    if add.is_empty() {
        return s;
    }
    // Перед `mod ui;`, если он есть, иначе — после последнего `use` верхнего уровня.
    if let Some(pos) = s.find("\nmod ui;\n") {
        s.insert_str(pos + 1, &add.replace("mod ui;\n", ""));
        return s;
    }
    let mut pos = 0;
    let mut off = 0;
    for l in s.split_inclusive('\n') {
        off += l.len();
        if l.starts_with("use ") && l.trim_end().ends_with(';') {
            pos = off;
        }
    }
    if pos == 0 {
        for l in s.split_inclusive('\n') {
            let t = l.trim_start();
            if t.starts_with("//!") || t.starts_with("#![") || t.trim().is_empty() {
                pos += l.len();
            } else {
                break;
            }
        }
    }
    s.insert_str(pos, &format!("\n// Формы из кода — forms/*.tvf (генерирует TV Studio).\n{}", add));
    s
}

/// Найти диалоги в исходниках проекта и добавить их формами
/// (`forms/<имя>.tvf`). Код программы не меняется, кроме объявлений
/// модулей и переноса констант команд кнопок в модель.
pub fn extract_into_project(p: &mut crate::model::Project) -> Result<FormsReport, String> {
    use crate::codegen::GENERATED_MARK;
    use crate::model::{Action, CommandDef};
    let src_dir = p.dir.join("src");
    let main_path = src_dir.join("main.rs");
    let mut main = std::fs::read_to_string(&main_path).map_err(|e| format!("{}: {}", main_path.display(), e))?;
    // Значения команд: константы main.rs и команды модели.
    let mut known = int_consts(&main);
    for c in &p.app.commands {
        known.insert(c.name.clone(), c.value as i64);
    }
    let mut files = Vec::new();
    rs_files(&src_dir, &mut files);
    let mut report = FormsReport::default();
    let mut moved = false;
    let mut names: Vec<String> = p.forms.clone();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        if text.starts_with(GENERATED_MARK) || !(text.contains("Dialog::centered") || text.contains("Dialog::new")) {
            continue;
        }
        let rel = p.rel(&f);
        let is_main = f == main_path;
        let mut extra = known.clone();
        if !is_main {
            extra.extend(int_consts(&text));
        }
        for x in extract(&text, &extra) {
            let mut form = x.form;
            let base = if is_ident(&form.name) { form.name.clone() } else { "dialog".to_string() };
            if p.forms.contains(&base) {
                report.existing.push(base);
                continue;
            }
            let name = if names.contains(&base) { (2..).map(|k| format!("{}_{}", base, k)).find(|n| !names.contains(n)).unwrap() } else { base };
            form.name = name.clone();
            // Команды кнопок — в модель.
            for (cname, value) in &x.commands {
                if p.app.commands.iter().any(|c| &c.name == cname) || crate::model::BUILTIN_COMMANDS.iter().any(|(b, _, _)| b == cname) {
                    continue;
                }
                p.app.commands.push(CommandDef {
                    name: cname.clone(),
                    value: *value as u16,
                    action: Action::None,
                    comment: format!("кнопка формы {}", name),
                });
                if let Some(m) = remove_const(&main, cname) {
                    main = m;
                    moved = true;
                }
            }
            form.save(&p.form_path(&name))?;
            names.push(name.clone());
            report.added.push((name, rel.clone(), x.notes));
        }
    }
    p.forms = names;
    if !report.added.is_empty() {
        main = ensure_mods(&main, moved || main.contains("use ui::cmd::*;"));
        std::fs::write(&main_path, main).map_err(|e| format!("{}: {}", main_path.display(), e))?;
    }
    p.save()?;
    let forms: Vec<Form> = p.forms.iter().filter_map(|n| Form::load(&p.form_path(n)).ok()).collect();
    crate::codegen::generate_all(p, &forms)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
const CM_GO: u16 = 120;
const ROWS: [&str; 3] = ["~И~мя:", "~Г~ород:", "~П~очта:"];

fn layout_dialog(title: &str) -> Dialog {
    let (w, h) = (44, 12);
    let mut d = Dialog::centered(w, h, title);
    for (i, text) in ROWS.iter().enumerate() {
        let y = 2 + 2 * i as i32;
        let field = d.add(InputLine::new(Rect::new(12, y, w - 2, y + 1), 50).with_validator(FilterValidator::new("0123")));
        d.add(Label::new(Rect::new(2, y, 12, y + 1), text, Some(field)));
    }
    let sb = d.add(ScrollBar::new(Rect::new(30, 8, 31, 10)));
    let mut lb = ListBox::new(Rect::new(3, 8, 30, 10), 1, Some(sb));
    lb.set_items(["a", "b"].iter().map(|s| s.to_string()).collect());
    let list = d.add(lb);
    let mut c = Cluster::check_boxes(Rect::new(32, 8, 42, 9), &["~X~"]);
    c.value = 1;
    d.add(c);
    if title.is_empty() {
        d.add(Button::new(Rect::new(1, 1, 5, 3), "?", cm::OK, bf::NORMAL));
    }
    d.add(MyWidget::new(Rect::new(1, 1, 2, 2)));
    let bw = 12;
    let x0 = (w - (bw + 2 + bw)) / 2;
    d.add(Button::new(Rect::new(x0, h - 3, x0 + bw, h - 1), "O~K~", CM_GO, bf::DEFAULT));
    d
}
"#;

    #[test]
    fn extracts_layout_dialog() {
        let v = extract(SRC, &HashMap::new());
        assert_eq!(v.len(), 1);
        let x = &v[0];
        let f = &x.form;
        assert_eq!(f.name, "layout_dialog");
        assert_eq!(f.title, "{title}");
        assert_eq!((f.w, f.h, f.centered), (44, 12, true));
        let kinds: Vec<Kind> = f.controls.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::InputLine,
                Kind::Label,
                Kind::InputLine,
                Kind::Label,
                Kind::InputLine,
                Kind::Label,
                Kind::ListBox,
                Kind::CheckBoxes,
                Kind::Button
            ]
        );
        assert_eq!(f.controls[2].y, 4);
        assert_eq!(f.controls[2].w, 30);
        assert_eq!(f.controls[2].validator, "filter:0123");
        assert_eq!(f.controls[3].link, f.controls[2].name);
        assert_eq!(f.controls[3].text, "~Г~ород:");
        let lb = &f.controls[6];
        assert_eq!((lb.name.as_str(), lb.w, lb.scrollbar), ("list", 28, true));
        assert_eq!(lb.items, vec!["a", "b"]);
        assert_eq!(f.controls[7].value, 1);
        let ok = &f.controls[8];
        assert_eq!((ok.x, ok.y, ok.command.as_str(), ok.default), (9, 9, "CM_GO", true));
        assert_eq!(x.commands, vec![("CM_GO".to_string(), 120)]);
        assert!(x.notes.iter().any(|n| n.contains("условия")), "{:?}", x.notes);
        assert!(x.notes.iter().any(|n| n.contains("MyWidget")), "{:?}", x.notes);
    }
}
