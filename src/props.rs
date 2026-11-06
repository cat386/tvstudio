//! Свойства элементов и формы: какие есть у каждого вида, как их показать
//! в инспекторе и как применить введённое значение.

use crate::model::{is_ident, Control, Form, Kind, Pal};

/// Свойство элемента или формы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    Name,
    X,
    Y,
    W,
    H,
    Text,
    Items,
    Command,
    Default,
    Link,
    MaxLen,
    Validator,
    History,
    Value,
    Max,
    Scrollbar,
    Columns,
    Header,
    Grid,
    Editable,
    RowNumbers,
    Checks,
    Lines,
    Expanded,
    Disabled,
    HelpCtx,
    // Свойства формы
    FormName,
    FormTitle,
    FormX,
    FormY,
    FormW,
    FormH,
    FormCentered,
    FormPalette,
    FormHelp,
}

impl Prop {
    /// Подпись свойства (для элемента вида `kind`).
    pub fn label(self, kind: Option<Kind>) -> &'static str {
        match self {
            Prop::Name | Prop::FormName => "Имя",
            Prop::X | Prop::FormX => "X",
            Prop::Y | Prop::FormY => "Y",
            Prop::W | Prop::FormW => "Ширина",
            Prop::H | Prop::FormH => "Высота",
            Prop::Text => match kind {
                Some(Kind::Button) | Some(Kind::Label) => "Надпись",
                Some(Kind::InputLine) | Some(Kind::ComboBox) | Some(Kind::Memo) => "Нач. текст",
                Some(Kind::Table) => "Строки",
                _ => "Текст",
            },
            Prop::Items => match kind {
                Some(Kind::Table) => "Столбцы",
                Some(Kind::TreeView) => "Узлы",
                _ => "Пункты",
            },
            Prop::Command => "Команда",
            Prop::Default => "По умолчанию",
            Prop::Link => "Связь",
            Prop::MaxLen => "Длина",
            Prop::Validator => "Проверка",
            Prop::History => "История",
            Prop::Value => match kind {
                Some(Kind::CheckBoxes) => "Отмечены",
                Some(Kind::RadioButtons) | Some(Kind::ListBox) => "Выбран",
                Some(Kind::Table) => "Строка",
                _ => "Значение",
            },
            Prop::Max => "Максимум",
            Prop::Scrollbar => "Прокрутка",
            Prop::Columns => "Столбцы",
            Prop::Header => "Заголовки",
            Prop::Grid => "Сетка",
            Prop::Editable => "Правка",
            Prop::RowNumbers => "Номера строк",
            Prop::Checks => "Флажки",
            Prop::Lines => "Линии",
            Prop::Expanded => "Раскрыто",
            Prop::Disabled => "Недоступен",
            Prop::HelpCtx | Prop::FormHelp => "Справка",
            Prop::FormTitle => "Заголовок",
            Prop::FormCentered => "По центру",
            Prop::FormPalette => "Цвет",
        }
    }

    /// Подсказка: что означает свойство и как его вводить.
    pub fn hint(self) -> &'static str {
        match self {
            Prop::Name | Prop::FormName => "идентификатор Rust: латиница, цифры, _",
            Prop::Text => "~X~ — горячая буква, \\n — перевод строки, \\x03 в начале — по центру",
            Prop::Items => "через |; таблица: Имя:20; дерево: > — уровень вложенности",
            Prop::Command => "cm::OK, cm::CANCEL… или имя своей команды",
            Prop::Default
            | Prop::Scrollbar
            | Prop::Disabled
            | Prop::FormCentered
            | Prop::Header
            | Prop::Grid
            | Prop::Editable
            | Prop::RowNumbers
            | Prop::Checks
            | Prop::Lines
            | Prop::Expanded => "да / нет",
            Prop::Link => "имя элемента, который выбирает метка",
            Prop::Validator => "picture:###-## | range:0..100 | filter:0123456789 | lookup:a|b",
            Prop::History => "номер списка истории, 0 — без истории",
            Prop::Value => "флажки — битовая маска, переключатели и список — номер с 0",
            Prop::FormPalette => "серый / синий / бирюзовый",
            Prop::HelpCtx | Prop::FormHelp => "номер контекста справки",
            _ => "",
        }
    }

    /// Многострочное ли свойство (в диалоге свойств — поле `Memo`).
    pub fn multiline(self) -> bool {
        matches!(self, Prop::Text | Prop::Items)
    }

    /// Логическое ли свойство.
    pub fn is_bool(self) -> bool {
        matches!(
            self,
            Prop::Default
                | Prop::Scrollbar
                | Prop::Disabled
                | Prop::FormCentered
                | Prop::Header
                | Prop::Grid
                | Prop::Editable
                | Prop::RowNumbers
                | Prop::Checks
                | Prop::Lines
                | Prop::Expanded
        )
    }
}

/// Свойства элемента вида `kind`.
pub fn control_props(kind: Kind) -> Vec<Prop> {
    use Prop::*;
    let mut v = vec![Name, X, Y, W, H];
    v.extend_from_slice(match kind {
        Kind::StaticText => &[Text][..],
        Kind::Label => &[Text, Link],
        Kind::Button => &[Text, Command, Default],
        Kind::InputLine => &[Text, MaxLen, Validator, History],
        Kind::CheckBoxes | Kind::RadioButtons => &[Items, Value],
        Kind::ListBox => &[Items, Value, Columns, Scrollbar],
        Kind::Memo => &[Text, Scrollbar],
        Kind::ComboBox => &[Text, Items, MaxLen],
        Kind::ScrollBar => &[],
        Kind::ProgressBar => &[Value, Max],
        Kind::Table => &[Items, Text, Value, Header, Grid, Editable, RowNumbers, Scrollbar],
        Kind::TreeView => &[Items, Expanded, Lines, Checks, Scrollbar],
    });
    v.push(Disabled);
    v.push(HelpCtx);
    v
}

/// Свойства формы.
pub fn form_props() -> Vec<Prop> {
    use Prop::*;
    vec![FormName, FormTitle, FormX, FormY, FormW, FormH, FormCentered, FormPalette, FormHelp]
}

/// Свойства выделения: элемента `sel` или формы.
pub fn props_for(form: &Form, sel: Option<usize>) -> Vec<Prop> {
    match sel.and_then(|i| form.controls.get(i)) {
        Some(c) => control_props(c.kind),
        None => form_props(),
    }
}

fn yes_no(b: bool) -> String {
    if b { "да" } else { "нет" }.into()
}

/// Разбор логического значения.
pub fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_lowercase().as_str() {
        "да" | "д" | "yes" | "y" | "true" | "1" | "+" | "вкл" | "on" => Ok(true),
        "нет" | "н" | "no" | "n" | "false" | "0" | "-" | "выкл" | "off" | "" => Ok(false),
        _ => Err(format!("«{}» — ожидается «да» или «нет»", s)),
    }
}

fn parse_int(s: &str, what: &str) -> Result<i64, String> {
    let t = s.trim();
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16)
    } else if let Some(b) = t.strip_prefix("0b") {
        i64::from_str_radix(b, 2)
    } else {
        t.parse::<i64>()
    };
    v.map_err(|_| format!("{}: «{}» — ожидается целое число", what, s))
}

/// Экранирование для однострочного показа: перевод строки → `\n`, символ 3 → `\x03`.
pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\x03', "\\x03")
}

/// Обратное к [`escape`].
pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.peek().copied() {
            Some('n') => {
                it.next();
                out.push('\n');
            }
            Some('\\') => {
                it.next();
                out.push('\\');
            }
            Some('x') => {
                it.next();
                let h: String = it.by_ref().take(2).collect();
                match u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push_str("\\x");
                        out.push_str(&h);
                    }
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

/// Значение свойства одной строкой (для инспектора).
pub fn get(form: &Form, sel: Option<usize>, p: Prop) -> String {
    if let Some(c) = sel.and_then(|i| form.controls.get(i)) {
        return get_control(c, p);
    }
    match p {
        Prop::FormName => form.name.clone(),
        Prop::FormTitle => form.title.clone(),
        Prop::FormX => form.x.to_string(),
        Prop::FormY => form.y.to_string(),
        Prop::FormW => form.w.to_string(),
        Prop::FormH => form.h.to_string(),
        Prop::FormCentered => yes_no(form.centered),
        Prop::FormPalette => form.palette.title().into(),
        Prop::FormHelp => form.help_ctx.to_string(),
        _ => String::new(),
    }
}

fn get_control(c: &Control, p: Prop) -> String {
    match p {
        Prop::Name => c.name.clone(),
        Prop::X => c.x.to_string(),
        Prop::Y => c.y.to_string(),
        Prop::W => c.w.to_string(),
        Prop::H => c.h.to_string(),
        Prop::Text => escape(&c.text),
        Prop::Items if c.kind == Kind::TreeView => c.items.iter().map(|s| tree_to_line(s)).collect::<Vec<_>>().join(" | "),
        Prop::Items => c.items.join(" | "),
        Prop::Command => c.command.clone(),
        Prop::Default => yes_no(c.default),
        Prop::Link => c.link.clone(),
        Prop::MaxLen => c.max_len.to_string(),
        Prop::Validator => c.validator.clone(),
        Prop::History => c.history.to_string(),
        Prop::Value => c.value.to_string(),
        Prop::Max => c.max.to_string(),
        Prop::Scrollbar => yes_no(c.scrollbar),
        Prop::Columns => c.columns.to_string(),
        Prop::Header => yes_no(c.header),
        Prop::Grid => yes_no(c.grid),
        Prop::Editable => yes_no(c.editable),
        Prop::RowNumbers => yes_no(c.row_numbers),
        Prop::Checks => yes_no(c.checks),
        Prop::Lines => yes_no(c.lines),
        Prop::Expanded => yes_no(c.expanded),
        Prop::Disabled => yes_no(c.disabled),
        Prop::HelpCtx => c.help_ctx.to_string(),
        _ => String::new(),
    }
}

/// Значение для многострочного редактирования: настоящие переводы строк,
/// пункты — по одному в строке.
pub fn get_full(form: &Form, sel: Option<usize>, p: Prop) -> String {
    match (sel.and_then(|i| form.controls.get(i)), p) {
        (Some(c), Prop::Text) => c.text.replace('\x03', "\\x03"),
        (Some(c), Prop::Items) => c.items.join("\n"),
        _ => get(form, sel, p),
    }
}

/// Применить многострочное значение (из диалога свойств).
pub fn set_full(form: &mut Form, sel: Option<usize>, p: Prop, v: &str) -> Result<(), String> {
    match (sel, p) {
        (Some(i), Prop::Text) if i < form.controls.len() => {
            form.controls[i].text = v.replace("\\x03", "\x03");
            Ok(())
        }
        (Some(i), Prop::Items) if i < form.controls.len() => {
            let tree = form.controls[i].kind == Kind::TreeView;
            form.controls[i].items =
                v.lines().map(|l| if tree { l.trim_end().to_string() } else { l.trim().to_string() }).filter(|l| !l.trim().is_empty()).collect();
            fit_cluster(&mut form.controls[i]);
            Ok(())
        }
        _ => set(form, sel, p, v),
    }
}

/// Узел дерева «  Письма» → «>Письма» (уровень — знаками `>`).
fn tree_to_line(s: &str) -> String {
    let d = (s.len() - s.trim_start().len()) / 2;
    format!("{}{}", ">".repeat(d), s.trim())
}

/// «>Письма» → «  Письма».
fn line_to_tree(s: &str) -> String {
    let t = s.trim();
    let d = t.chars().take_while(|&c| c == '>').count();
    format!("{}{}", "  ".repeat(d), t[d..].trim())
}

/// Высота флажков/переключателей не меньше числа пунктов (как в DLGDSN).
fn fit_cluster(c: &mut Control) {
    if matches!(c.kind, Kind::CheckBoxes | Kind::RadioButtons) && c.columns <= 1 {
        c.h = c.h.max(c.items.len().max(1) as i32);
    }
}

/// Применить значение свойства, введённое одной строкой.
pub fn set(form: &mut Form, sel: Option<usize>, p: Prop, v: &str) -> Result<(), String> {
    let label = p.label(sel.and_then(|i| form.controls.get(i)).map(|c| c.kind));
    let int = |s: &str| parse_int(s, label);
    if let Some(i) = sel {
        if i >= form.controls.len() {
            return Err("элемент не найден".into());
        }
        if p == Prop::Name {
            let n = v.trim();
            if !is_ident(n) {
                return Err(format!("«{}» не годится как имя: нужен идентификатор Rust (латиница, цифры, _)", n));
            }
            if form.controls.iter().enumerate().any(|(j, c)| j != i && c.name == n) {
                return Err(format!("имя «{}» уже занято", n));
            }
            let old = std::mem::replace(&mut form.controls[i].name, n.to_string());
            // Метки, привязанные к старому имени, переходят на новое.
            for c in form.controls.iter_mut() {
                if c.link == old {
                    c.link = n.to_string();
                }
            }
            return Ok(());
        }
        if p == Prop::Link {
            let n = v.trim();
            if !n.is_empty() && form.find(n).is_none() {
                return Err(format!("на форме нет элемента «{}»", n));
            }
            form.controls[i].link = n.to_string();
            return Ok(());
        }
        let c = &mut form.controls[i];
        match p {
            Prop::X => c.x = int(v)? as i32,
            Prop::Y => c.y = int(v)? as i32,
            Prop::W => c.w = (int(v)? as i32).max(1),
            Prop::H => c.h = (int(v)? as i32).max(1),
            Prop::Text => c.text = unescape(v),
            Prop::Items if c.kind == Kind::TreeView => {
                c.items = v.split('|').map(line_to_tree).filter(|s| !s.trim().is_empty()).collect();
            }
            Prop::Items => {
                c.items = v.split('|').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                fit_cluster(c);
            }
            Prop::Command => c.command = v.trim().to_string(),
            Prop::Default => c.default = parse_bool(v)?,
            Prop::MaxLen => c.max_len = int(v)?.clamp(1, 4096),
            Prop::Validator => {
                let s = v.trim();
                check_validator(s)?;
                c.validator = s.to_string();
            }
            Prop::History => c.history = int(v)?.clamp(0, 65535),
            Prop::Value => c.value = int(v)?,
            Prop::Max => c.max = int(v)?.max(1),
            Prop::Scrollbar => c.scrollbar = parse_bool(v)?,
            Prop::Columns => c.columns = int(v)?.clamp(1, 16),
            Prop::Header => c.header = parse_bool(v)?,
            Prop::Grid => c.grid = parse_bool(v)?,
            Prop::Editable => c.editable = parse_bool(v)?,
            Prop::RowNumbers => c.row_numbers = parse_bool(v)?,
            Prop::Checks => c.checks = parse_bool(v)?,
            Prop::Lines => c.lines = parse_bool(v)?,
            Prop::Expanded => c.expanded = parse_bool(v)?,
            Prop::Disabled => c.disabled = parse_bool(v)?,
            Prop::HelpCtx => c.help_ctx = int(v)?.clamp(0, 65535),
            _ => return Err("у элемента нет такого свойства".into()),
        }
        return Ok(());
    }
    match p {
        Prop::FormName => {
            let n = v.trim();
            if !is_ident(n) {
                return Err(format!("«{}» не годится как имя формы: нужен идентификатор Rust", n));
            }
            form.name = n.to_string();
        }
        Prop::FormTitle => form.title = v.to_string(),
        Prop::FormX => form.x = int(v)? as i32,
        Prop::FormY => form.y = int(v)? as i32,
        Prop::FormW => form.w = (int(v)? as i32).clamp(10, 250),
        Prop::FormH => form.h = (int(v)? as i32).clamp(4, 100),
        Prop::FormCentered => form.centered = parse_bool(v)?,
        Prop::FormPalette => form.palette = Pal::from_id(v).ok_or_else(|| format!("«{}» — цвет: серый, синий или бирюзовый", v))?,
        Prop::FormHelp => form.help_ctx = int(v)?.clamp(0, 65535),
        _ => return Err("у формы нет такого свойства".into()),
    }
    Ok(())
}

/// Проверка строки описания проверки ввода.
pub fn check_validator(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Ok(());
    }
    let (kind, arg) = s.split_once(':').ok_or_else(|| format!("«{}»: нужен вид проверки, например range:0..100", s))?;
    match kind.trim() {
        "picture" | "filter" | "lookup" => Ok(()),
        "range" => parse_range(arg).map(|_| ()),
        k => Err(format!("неизвестный вид проверки «{}» (picture, range, filter, lookup)", k)),
    }
}

/// `0..100` → (0, 100).
pub fn parse_range(s: &str) -> Result<(i64, i64), String> {
    let (a, b) = s.split_once("..").ok_or_else(|| format!("«{}»: диапазон записывается как 0..100", s))?;
    let a = a.trim().parse::<i64>().map_err(|_| format!("«{}» — не число", a))?;
    let b = b.trim().parse::<i64>().map_err(|_| format!("«{}» — не число", b))?;
    Ok((a.min(b), a.max(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::about_form;

    #[test]
    fn edit_props() {
        let mut f = about_form("x");
        set(&mut f, Some(1), Prop::Name, "close_btn").unwrap();
        assert_eq!(f.controls[1].name, "close_btn");
        assert!(set(&mut f, Some(1), Prop::Name, "info").is_err());
        assert!(set(&mut f, Some(1), Prop::Default, "может быть").is_err());
        set(&mut f, Some(0), Prop::Text, "a\\nb").unwrap();
        assert_eq!(f.controls[0].text, "a\nb");
        assert_eq!(get(&f, Some(0), Prop::Text), "a\\nb");
        set(&mut f, None, Prop::FormPalette, "синий").unwrap();
        assert_eq!(f.palette, Pal::Blue);
        assert!(set(&mut f, Some(1), Prop::Validator, "range:1..x").is_err());
    }

    #[test]
    fn tree_and_table_props() {
        let mut f = about_form("x");
        f.controls.push(crate::model::Control::new(Kind::TreeView, "tree", 2, 2));
        let i = f.controls.len() - 1;
        assert_eq!(get(&f, Some(i), Prop::Items), "Документы | >Письма | >Отчёты | Музыка | Фото | >2026");
        set(&mut f, Some(i), Prop::Items, "А | >Б | >>В").unwrap();
        assert_eq!(f.controls[i].items, vec!["А", "  Б", "    В"]);
        let t = crate::model::parse_tree(&f.controls[i].items);
        assert_eq!(t[0].children[0].children[0].text, "В");
        set(&mut f, Some(i), Prop::Checks, "да").unwrap();
        assert!(f.controls[i].checks);
        f.controls.push(crate::model::Control::new(Kind::Table, "table", 2, 2));
        let c = f.controls.last().unwrap();
        assert_eq!(c.table_columns(), vec![("Имя".to_string(), 16), ("Телефон".to_string(), 14)]);
        assert_eq!(c.table_rows()[1], vec!["Борис", "234-56-78"]);
    }

    #[test]
    fn escapes() {
        let s = "\x03По центру\nи \\ слеш";
        assert_eq!(unescape(&escape(s)), s);
    }
}
