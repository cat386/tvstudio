//! Модель приложения, которое проектируется в TV Studio.
//!
//! * [`Form`] — диалоговое окно с элементами ([`Control`]);
//! * [`AppModel`] — «каркас» программы: строка меню, строка состояния,
//!   команды и действия, которые выполняются по командам;
//! * [`Project`] — каталог проекта Cargo + файл проекта `*.tvsp`
//!   + файлы форм `forms/*.tvf`.
//!
//! Модель не зависит от интерфейса: из неё строятся живые виджеты
//! (`builder`) и генерируется код на Rust (`codegen`).

use std::path::{Path, PathBuf};

use tvision::{Key, KeyEvent, Mods, Rect};

use crate::json::Json;

// ======================================================================
// Элементы формы
// ======================================================================

/// Вид элемента формы.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    StaticText,
    Label,
    Button,
    InputLine,
    CheckBoxes,
    RadioButtons,
    ListBox,
    Memo,
    ComboBox,
    ScrollBar,
    ProgressBar,
    Table,
    TreeView,
}

impl Kind {
    /// Все виды — в порядке палитры компонентов.
    pub const ALL: [Kind; 13] = [
        Kind::StaticText,
        Kind::Label,
        Kind::Button,
        Kind::InputLine,
        Kind::CheckBoxes,
        Kind::RadioButtons,
        Kind::ListBox,
        Kind::Memo,
        Kind::ComboBox,
        Kind::ScrollBar,
        Kind::ProgressBar,
        Kind::Table,
        Kind::TreeView,
    ];

    /// Имя для файлов формы.
    pub fn id(self) -> &'static str {
        match self {
            Kind::StaticText => "static_text",
            Kind::Label => "label",
            Kind::Button => "button",
            Kind::InputLine => "input_line",
            Kind::CheckBoxes => "check_boxes",
            Kind::RadioButtons => "radio_buttons",
            Kind::ListBox => "list_box",
            Kind::Memo => "memo",
            Kind::ComboBox => "combo_box",
            Kind::ScrollBar => "scroll_bar",
            Kind::ProgressBar => "progress_bar",
            Kind::Table => "table",
            Kind::TreeView => "tree_view",
        }
    }

    pub fn from_id(s: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.id() == s)
    }

    /// Название для пользователя.
    pub fn title(self) -> &'static str {
        match self {
            Kind::StaticText => "Текст",
            Kind::Label => "Метка",
            Kind::Button => "Кнопка",
            Kind::InputLine => "Строка ввода",
            Kind::CheckBoxes => "Флажки",
            Kind::RadioButtons => "Переключатели",
            Kind::ListBox => "Список",
            Kind::Memo => "Многострочное поле",
            Kind::ComboBox => "Поле со списком",
            Kind::ScrollBar => "Полоса прокрутки",
            Kind::ProgressBar => "Полоса выполнения",
            Kind::Table => "Таблица",
            Kind::TreeView => "Дерево",
        }
    }

    /// Тип tvision-rs, который соответствует элементу.
    pub fn rust_type(self) -> &'static str {
        match self {
            Kind::StaticText => "StaticText",
            Kind::Label => "Label",
            Kind::Button => "Button",
            Kind::InputLine => "InputLine",
            Kind::CheckBoxes => "CheckBoxes",
            Kind::RadioButtons => "RadioButtons",
            Kind::ListBox => "ListBox",
            Kind::Memo => "Memo",
            Kind::ComboBox => "InputLine + Combo",
            Kind::ScrollBar => "ScrollBar",
            Kind::ProgressBar => "ProgressBar",
            Kind::Table => "Table",
            Kind::TreeView => "TreeView",
        }
    }

    /// Начало имени нового элемента.
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::StaticText => "text",
            Kind::Label => "label",
            Kind::Button => "button",
            Kind::InputLine => "input",
            Kind::CheckBoxes => "checks",
            Kind::RadioButtons => "radios",
            Kind::ListBox => "list",
            Kind::Memo => "memo",
            Kind::ComboBox => "combo",
            Kind::ScrollBar => "scroll",
            Kind::ProgressBar => "progress",
            Kind::Table => "table",
            Kind::TreeView => "tree",
        }
    }

    /// Размер нового элемента.
    pub fn default_size(self) -> (i32, i32) {
        match self {
            Kind::StaticText => (20, 1),
            Kind::Label => (12, 1),
            Kind::Button => (12, 2),
            Kind::InputLine => (24, 1),
            Kind::CheckBoxes | Kind::RadioButtons => (20, 2),
            Kind::ListBox => (24, 5),
            Kind::Memo => (30, 4),
            Kind::ComboBox => (24, 1),
            Kind::ScrollBar => (1, 6),
            Kind::ProgressBar => (30, 1),
            Kind::Table => (40, 6),
            Kind::TreeView => (30, 8),
        }
    }

    /// Участвует ли элемент в обмене данными диалога (`get_data`/`set_data`).
    pub fn has_data(self) -> bool {
        matches!(
            self,
            Kind::InputLine | Kind::CheckBoxes | Kind::RadioButtons | Kind::ListBox | Kind::Memo | Kind::ComboBox | Kind::ProgressBar | Kind::Table
        )
    }

    /// Получает ли элемент фокус (участвует в переходе по Tab).
    pub fn selectable(self) -> bool {
        matches!(
            self,
            Kind::Button
                | Kind::InputLine
                | Kind::CheckBoxes
                | Kind::RadioButtons
                | Kind::ListBox
                | Kind::Memo
                | Kind::ComboBox
                | Kind::Table
                | Kind::TreeView
        )
    }

    /// Тип поля в структуре данных формы.
    pub fn data_type(self) -> &'static str {
        match self {
            Kind::InputLine | Kind::Memo | Kind::ComboBox => "String",
            Kind::CheckBoxes | Kind::RadioButtons => "u32",
            Kind::ListBox | Kind::ProgressBar | Kind::Table => "i64",
            _ => "()",
        }
    }
}

/// Элемент формы. Одна структура на все виды: у каждого вида
/// используются только «свои» поля (см. `props::control_props`).
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub kind: Kind,
    /// Имя (идентификатор Rust) — поле в структуре данных и в `Ids`.
    pub name: String,
    /// Границы в координатах диалога (строка 0 — рамка).
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Надпись кнопки/метки, текст, начальный текст строки ввода и поля.
    pub text: String,
    /// Пункты флажков, переключателей, списка, поля со списком.
    pub items: Vec<String>,
    /// Команда кнопки: `cm::OK` или имя своей команды (`ABOUT`).
    pub command: String,
    /// Кнопка по умолчанию (нажимается по Enter).
    pub default: bool,
    /// Метка: имя элемента, к которому она привязана.
    pub link: String,
    /// Строка ввода: наибольшая длина.
    pub max_len: i64,
    /// Проверка ввода: `""`, `picture:…`, `range:a..b`, `filter:…`, `lookup:a|b`.
    pub validator: String,
    /// Строка ввода: номер списка истории (0 — без истории).
    pub history: i64,
    /// Начальное значение: флажки (маска), переключатели (номер),
    /// список (номер), полоса выполнения (значение).
    pub value: i64,
    /// Полоса выполнения: наибольшее значение.
    pub max: i64,
    /// Список и многострочное поле: своя вертикальная полоса прокрутки.
    pub scrollbar: bool,
    /// Список: число столбцов.
    pub columns: i64,
    /// Таблица: строка заголовков, линии сетки, правка ячеек, номера строк.
    pub header: bool,
    pub grid: bool,
    pub editable: bool,
    pub row_numbers: bool,
    /// Дерево: флажки у узлов, линии ветвей, узлы раскрыты.
    pub checks: bool,
    pub lines: bool,
    pub expanded: bool,
    /// Элемент недоступен.
    pub disabled: bool,
    /// Контекст справки.
    pub help_ctx: i64,
}

impl Control {
    /// Новый элемент вида `kind` в точке (x, y) с настройками по умолчанию.
    pub fn new(kind: Kind, name: &str, x: i32, y: i32) -> Self {
        let (w, h) = kind.default_size();
        let mut c = Control {
            kind,
            name: name.to_string(),
            x,
            y,
            w,
            h,
            text: String::new(),
            items: Vec::new(),
            command: String::new(),
            default: false,
            link: String::new(),
            max_len: 64,
            validator: String::new(),
            history: 0,
            value: 0,
            max: 100,
            scrollbar: false,
            columns: 1,
            header: true,
            grid: true,
            editable: false,
            row_numbers: false,
            checks: false,
            lines: true,
            expanded: true,
            disabled: false,
            help_ctx: 0,
        };
        match kind {
            Kind::StaticText => c.text = "Текст".into(),
            Kind::Label => c.text = "~М~етка".into(),
            Kind::Button => {
                c.text = "O~K~".into();
                c.command = "cm::OK".into();
            }
            Kind::CheckBoxes | Kind::RadioButtons => c.items = vec!["Первый".into(), "Второй".into()],
            Kind::ListBox => {
                c.items = vec!["Элемент 1".into(), "Элемент 2".into(), "Элемент 3".into()];
                c.scrollbar = true;
            }
            Kind::Memo => c.scrollbar = true,
            Kind::ComboBox => {
                c.items = vec!["Один".into(), "Два".into(), "Три".into()];
                c.max_len = 32;
            }
            Kind::ProgressBar => c.value = 40,
            Kind::Table => {
                // Столбцы — «заголовок:ширина», строки — ячейки через |.
                c.items = vec!["Имя:16".into(), "Телефон:14".into()];
                c.text = "Аня|123-45-67\nБорис|234-56-78\nВера|345-67-89".into();
                c.scrollbar = true;
            }
            Kind::TreeView => {
                // Узлы — по строке, вложенность — два пробела на уровень.
                c.items = ["Документы", "  Письма", "  Отчёты", "Музыка", "Фото", "  2026"].iter().map(|s| s.to_string()).collect();
                c.scrollbar = true;
            }
            _ => {}
        }
        c
    }

    /// Границы элемента.
    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.x + self.w, self.y + self.h)
    }

    pub fn to_json(&self) -> Json {
        let mut p: Vec<(&str, Json)> = vec![
            ("kind", Json::s(self.kind.id())),
            ("name", Json::s(&self.name)),
            ("x", Json::i(self.x as i64)),
            ("y", Json::i(self.y as i64)),
            ("w", Json::i(self.w as i64)),
            ("h", Json::i(self.h as i64)),
        ];
        let d = Control::new(self.kind, "", 0, 0);
        if !self.text.is_empty() {
            p.push(("text", Json::s(&self.text)));
        }
        if !self.items.is_empty() {
            p.push(("items", Json::from_strings(&self.items)));
        }
        if !self.command.is_empty() {
            p.push(("command", Json::s(&self.command)));
        }
        if self.default {
            p.push(("default", Json::Bool(true)));
        }
        if !self.link.is_empty() {
            p.push(("link", Json::s(&self.link)));
        }
        if self.max_len != d.max_len {
            p.push(("max_len", Json::i(self.max_len)));
        }
        if !self.validator.is_empty() {
            p.push(("validator", Json::s(&self.validator)));
        }
        if self.history != 0 {
            p.push(("history", Json::i(self.history)));
        }
        if self.value != 0 {
            p.push(("value", Json::i(self.value)));
        }
        if self.max != d.max {
            p.push(("max", Json::i(self.max)));
        }
        p.push(("scrollbar", Json::Bool(self.scrollbar)));
        if self.columns != 1 {
            p.push(("columns", Json::i(self.columns)));
        }
        for (k, v, dv) in [
            ("header", self.header, d.header),
            ("grid", self.grid, d.grid),
            ("editable", self.editable, d.editable),
            ("row_numbers", self.row_numbers, d.row_numbers),
            ("checks", self.checks, d.checks),
            ("lines", self.lines, d.lines),
            ("expanded", self.expanded, d.expanded),
        ] {
            if v != dv {
                p.push((k, Json::Bool(v)));
            }
        }
        if self.disabled {
            p.push(("disabled", Json::Bool(true)));
        }
        if self.help_ctx != 0 {
            p.push(("help_ctx", Json::i(self.help_ctx)));
        }
        Json::obj(p)
    }

    pub fn from_json(j: &Json) -> Result<Self, String> {
        let kind = Kind::from_id(&j.str("kind")).ok_or_else(|| format!("неизвестный вид элемента «{}»", j.str("kind")))?;
        let mut c = Control::new(kind, &j.str("name"), j.int("x", 1) as i32, j.int("y", 1) as i32);
        c.w = j.int("w", c.w as i64) as i32;
        c.h = j.int("h", c.h as i64) as i32;
        c.text = j.str("text");
        c.items = j.strings("items");
        c.command = j.str("command");
        c.default = j.bool("default", false);
        c.link = j.str("link");
        c.max_len = j.int("max_len", c.max_len);
        c.validator = j.str("validator");
        c.history = j.int("history", 0);
        c.value = j.int("value", 0);
        c.max = j.int("max", c.max);
        c.scrollbar = j.bool("scrollbar", c.scrollbar);
        c.columns = j.int("columns", 1).max(1);
        c.header = j.bool("header", c.header);
        c.grid = j.bool("grid", c.grid);
        c.editable = j.bool("editable", c.editable);
        c.row_numbers = j.bool("row_numbers", c.row_numbers);
        c.checks = j.bool("checks", c.checks);
        c.lines = j.bool("lines", c.lines);
        c.expanded = j.bool("expanded", c.expanded);
        c.disabled = j.bool("disabled", false);
        c.help_ctx = j.int("help_ctx", 0);
        Ok(c)
    }
}

/// Столбец таблицы из записи «Заголовок:ширина» (ширина не обязательна).
pub fn parse_column(s: &str) -> (String, Option<i32>) {
    match s.rsplit_once(':') {
        Some((t, w)) if w.trim().parse::<i32>().is_ok() => (t.trim().to_string(), w.trim().parse::<i32>().ok().map(|w| w.clamp(1, 200))),
        _ => (s.trim().to_string(), None),
    }
}

impl Control {
    /// Таблица: заголовки и ширины столбцов (ширина по умолчанию — по
    /// самому длинному тексту столбца).
    pub fn table_columns(&self) -> Vec<(String, i32)> {
        let rows = self.table_rows();
        self.items
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (t, w) = parse_column(s);
                let auto = rows.iter().map(|r| r.get(i).map(|c| c.chars().count()).unwrap_or(0)).max().unwrap_or(0).max(t.chars().count()) as i32 + 1;
                (t, w.unwrap_or(auto.min(40)))
            })
            .collect()
    }

    /// Таблица: строки данных (ячейки через `|`), дополненные до числа столбцов.
    pub fn table_rows(&self) -> Vec<Vec<String>> {
        let n = self.items.len();
        self.text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let mut r: Vec<String> = l.split('|').map(|c| c.trim().to_string()).collect();
                r.resize(n.max(r.len()), String::new());
                r
            })
            .collect()
    }
}

/// Узел дерева, разобранный из строк с отступами.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeItem {
    pub text: String,
    pub children: Vec<TreeItem>,
}

/// Строки с отступом (два пробела на уровень) → дерево.
pub fn parse_tree(items: &[String]) -> Vec<TreeItem> {
    fn depth(s: &str) -> usize {
        (s.len() - s.trim_start().len()) / 2
    }
    fn build(items: &[String], i: &mut usize, d: usize) -> Vec<TreeItem> {
        let mut out = Vec::new();
        while *i < items.len() && depth(&items[*i]) >= d {
            let text = items[*i].trim().to_string();
            *i += 1;
            let children = build(items, i, d + 1);
            out.push(TreeItem { text, children });
        }
        out
    }
    let items: Vec<String> = items.iter().filter(|s| !s.trim().is_empty()).cloned().collect();
    let mut i = 0;
    build(&items, &mut i, 0)
}

// ======================================================================
// Форма
// ======================================================================

/// Цвет диалога.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pal {
    Gray,
    Blue,
    Cyan,
}

impl Pal {
    pub fn id(self) -> &'static str {
        match self {
            Pal::Gray => "gray",
            Pal::Blue => "blue",
            Pal::Cyan => "cyan",
        }
    }
    pub fn from_id(s: &str) -> Option<Pal> {
        match s.trim().to_lowercase().as_str() {
            "gray" | "серый" | "серая" => Some(Pal::Gray),
            "blue" | "синий" | "синяя" => Some(Pal::Blue),
            "cyan" | "бирюзовый" | "бирюзовая" => Some(Pal::Cyan),
            _ => None,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Pal::Gray => "серый",
            Pal::Blue => "синий",
            Pal::Cyan => "бирюзовый",
        }
    }
    pub fn palette(self) -> &'static [u8] {
        use tvision::color::palettes;
        match self {
            Pal::Gray => palettes::GRAY_DIALOG,
            Pal::Blue => palettes::BLUE_DIALOG,
            Pal::Cyan => palettes::CYAN_DIALOG,
        }
    }
}

/// Из какого класса tvision-rs строится форма.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FormClass {
    /// `Dialog` — модальный диалог: `run()` показывает его и ждёт
    /// OK / Отмены; Enter нажимает кнопку по умолчанию, Esc — Отмена.
    #[default]
    Dialog,
    /// `Window` — немодальное окно на рабочем столе: `open()` вставляет
    /// его и сразу возвращается; окно двигают, закрывают (и, если можно,
    /// растягивают), команды кнопок идут обработчику программы.
    Window,
}

impl FormClass {
    pub const ALL: [FormClass; 2] = [FormClass::Dialog, FormClass::Window];

    pub fn id(self) -> &'static str {
        match self {
            FormClass::Dialog => "Dialog",
            FormClass::Window => "Window",
        }
    }
    pub fn from_id(s: &str) -> Option<FormClass> {
        match s.trim().to_lowercase().as_str() {
            "dialog" | "диалог" | "" => Some(FormClass::Dialog),
            "window" | "окно" => Some(FormClass::Window),
            _ => None,
        }
    }
    /// Подпись для пользователя.
    pub fn title(self) -> &'static str {
        match self {
            FormClass::Dialog => "Dialog (модальный диалог)",
            FormClass::Window => "Window (немодальное окно)",
        }
    }
}

/// Форма — диалог (`Dialog`) или окно (`Window`).
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    /// Имя формы (модуль `forms::<name>`).
    pub name: String,
    /// Заголовок окна.
    pub title: String,
    /// Положение и размер (с рамкой).
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Центрировать при показе.
    pub centered: bool,
    pub palette: Pal,
    pub help_ctx: i64,
    /// Класс: диалог или окно.
    pub class: FormClass,
    /// Окно: номер (Alt+1…9), 0 — без номера.
    pub number: i16,
    /// Окно: можно растягивать и распахивать.
    pub resizable: bool,
    /// Элементы в порядке перехода по Tab.
    pub controls: Vec<Control>,
}

impl Form {
    pub fn new(name: &str, title: &str) -> Self {
        Form {
            name: name.into(),
            title: title.into(),
            x: 10,
            y: 3,
            w: 50,
            h: 14,
            centered: true,
            palette: Pal::Gray,
            help_ctx: 0,
            class: FormClass::Dialog,
            number: 0,
            resizable: true,
            controls: Vec::new(),
        }
    }

    /// Новый элемент вида `kind` с уникальным именем.
    pub fn new_control(&self, kind: Kind, x: i32, y: i32) -> Control {
        Control::new(kind, &self.unique_name(kind.prefix()), x, y)
    }

    /// Уникальное имя вида `prefix1`, `prefix2`…
    pub fn unique_name(&self, prefix: &str) -> String {
        (1..).map(|n| format!("{}{}", prefix, n)).find(|n| self.find(n).is_none()).unwrap()
    }

    /// Номер элемента по имени.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.controls.iter().position(|c| c.name == name)
    }

    /// Самый верхний элемент, накрывающий точку (координаты диалога).
    pub fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.controls.iter().rposition(|c| c.rect().contains(tvision::Point::new(x, y)))
    }

    /// Элементы с данными (порядок — порядок полей в структуре данных).
    pub fn data_controls(&self) -> Vec<&Control> {
        build_order(self).into_iter().map(|i| &self.controls[i]).filter(|c| c.kind.has_data()).collect()
    }

    /// Проверка формы перед генерацией кода: список предупреждений.
    pub fn check(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !is_ident(&self.name) {
            out.push(format!("имя формы «{}» не годится как идентификатор Rust", self.name));
        }
        let mut names = std::collections::HashSet::new();
        for c in &self.controls {
            if !is_ident(&c.name) {
                out.push(format!("имя «{}» не годится как идентификатор Rust", c.name));
            }
            if !names.insert(c.name.clone()) {
                out.push(format!("имя «{}» повторяется", c.name));
            }
            if c.kind == Kind::Label && !c.link.is_empty() && self.find(&c.link).is_none() {
                out.push(format!("метка «{}» привязана к несуществующему элементу «{}»", c.name, c.link));
            }
            if c.x < 1 || c.y < 1 || c.x + c.w > self.w - 1 || c.y + c.h > self.h - 1 {
                out.push(format!("элемент «{}» выходит за рамку формы", c.name));
            }
            if c.w < 1 || c.h < 1 {
                out.push(format!("у элемента «{}» нулевой размер", c.name));
            }
        }
        out
    }

    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("format", Json::s("tvstudio-form")),
            ("version", Json::i(1)),
            ("name", Json::s(&self.name)),
            ("title", Json::s(&self.title)),
            ("x", Json::i(self.x as i64)),
            ("y", Json::i(self.y as i64)),
            ("w", Json::i(self.w as i64)),
            ("h", Json::i(self.h as i64)),
            ("centered", Json::Bool(self.centered)),
            ("palette", Json::s(self.palette.id())),
            ("help_ctx", Json::i(self.help_ctx)),
            ("class", Json::s(self.class.id())),
            ("number", Json::i(self.number as i64)),
            ("resizable", Json::Bool(self.resizable)),
            ("controls", Json::Arr(self.controls.iter().map(|c| c.to_json()).collect())),
        ])
    }

    pub fn from_json(j: &Json) -> Result<Self, String> {
        let mut f = Form::new(&j.str("name"), &j.str("title"));
        f.x = j.int("x", 10) as i32;
        f.y = j.int("y", 3) as i32;
        f.w = j.int("w", 50) as i32;
        f.h = j.int("h", 14) as i32;
        f.centered = j.bool("centered", true);
        f.palette = Pal::from_id(&j.str("palette")).unwrap_or(Pal::Gray);
        f.help_ctx = j.int("help_ctx", 0);
        f.class = FormClass::from_id(&j.str("class")).unwrap_or_default();
        f.number = j.int("number", 0).clamp(0, 9) as i16;
        f.resizable = j.bool("resizable", true);
        for c in j.arr("controls") {
            f.controls.push(Control::from_json(c)?);
        }
        Ok(f)
    }

    /// Загрузить форму из файла `*.tvf` (TOML; файлы старых версий — JSON).
    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        Form::from_json(&crate::toml::parse_document(&s).map_err(|e| format!("{}: {}", path.display(), e))?)
    }

    /// Сохранить форму в TOML.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let text = crate::toml::to_string(&self.to_json())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {}", path.display(), e))
    }
}

/// Порядок вставки элементов в диалог: сначала все элементы, кроме меток
/// (в порядке Tab), затем метки — так метка всегда находит свой элемент.
pub fn build_order(f: &Form) -> Vec<usize> {
    let mut v: Vec<usize> = (0..f.controls.len()).filter(|&i| f.controls[i].kind != Kind::Label).collect();
    v.extend((0..f.controls.len()).filter(|&i| f.controls[i].kind == Kind::Label));
    v
}

// ======================================================================
// Команды, меню, строка состояния
// ======================================================================

/// Стандартные команды tvision-rs: имя в коде, номер, описание.
pub const BUILTIN_COMMANDS: &[(&str, u16, &str)] = &[
    ("cm::QUIT", 1, "выход из программы"),
    ("cm::MENU", 3, "открыть строку меню"),
    ("cm::CLOSE", 4, "закрыть окно"),
    ("cm::ZOOM", 5, "распахнуть окно"),
    ("cm::RESIZE", 6, "размер/перемещение окна"),
    ("cm::NEXT", 7, "следующее окно"),
    ("cm::PREV", 8, "предыдущее окно"),
    ("cm::HELP", 9, "справка"),
    ("cm::OK", 10, "OK (закрывает диалог)"),
    ("cm::CANCEL", 11, "Отмена (закрывает диалог)"),
    ("cm::YES", 12, "Да (закрывает диалог)"),
    ("cm::NO", 13, "Нет (закрывает диалог)"),
    ("cm::CUT", 20, "вырезать"),
    ("cm::COPY", 21, "копировать"),
    ("cm::PASTE", 22, "вставить"),
    ("cm::UNDO", 23, "отменить"),
    ("cm::CLEAR", 24, "удалить выделенное"),
    ("cm::TILE", 25, "окна мозаикой"),
    ("cm::CASCADE", 26, "окна каскадом"),
    ("cm::REDO", 27, "повторить"),
    ("cm::DEFAULT", 14, "кнопка по умолчанию"),
    ("cm::NEW", 30, "новый"),
    ("cm::OPEN", 31, "открыть"),
    ("cm::SAVE", 32, "сохранить"),
    ("cm::SAVE_AS", 33, "сохранить как"),
    ("cm::SAVE_ALL", 34, "сохранить всё"),
    ("cm::CH_DIR", 35, "сменить каталог"),
    ("cm::DOS_SHELL", 36, "оболочка ОС"),
    ("cm::CLOSE_ALL", 37, "закрыть все окна"),
    ("cm::FIND", 82, "найти"),
    ("cm::REPLACE", 83, "заменить"),
    ("cm::SEARCH_AGAIN", 84, "искать дальше"),
    ("cm::SELECT_ALL", 524, "выделить всё"),
];

/// Действие, которое выполняет команда (в сгенерированном `ui::handle_command`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Ничего: команду обрабатывает ваш код в `main.rs`.
    None,
    /// Открыть форму (модальный диалог).
    Form(String),
    /// Показать сообщение.
    Message(String),
}

impl Action {
    pub fn to_str(&self) -> String {
        match self {
            Action::None => String::new(),
            Action::Form(f) => format!("form:{}", f),
            Action::Message(m) => format!("message:{}", m),
        }
    }
    pub fn parse(s: &str) -> Action {
        let s = s.trim();
        if let Some(f) = s.strip_prefix("form:") {
            Action::Form(f.trim().to_string())
        } else if let Some(m) = s.strip_prefix("message:") {
            Action::Message(m.to_string())
        } else if s.is_empty() {
            Action::None
        } else {
            Action::Message(s.to_string())
        }
    }
    pub fn describe(&self) -> String {
        match self {
            Action::None => "— (ваш код)".into(),
            Action::Form(f) => format!("открыть форму {}", f),
            Action::Message(m) => format!("сообщение «{}»", m),
        }
    }
}

/// Своя команда программы.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandDef {
    /// Имя константы (`ABOUT` → `cmd::ABOUT`).
    pub name: String,
    pub value: u16,
    pub action: Action,
    /// Комментарий к константе.
    pub comment: String,
}

/// Пункт меню.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuNode {
    Item {
        title: String,
        command: String,
        key: String,
        help: i64,
    },
    /// Подменю; `key` — клавиша, раскрывающая его (как Alt+Пробел у «≡»),
    /// `help` — контекст справки.
    Sub {
        title: String,
        key: String,
        help: i64,
        children: Vec<MenuNode>,
    },
    Separator,
}

impl MenuNode {
    pub fn to_json(&self) -> Json {
        match self {
            MenuNode::Item { title, command, key, help } => {
                Json::obj(vec![("title", Json::s(title)), ("command", Json::s(command)), ("key", Json::s(key)), ("help", Json::i(*help))])
            }
            MenuNode::Sub { title, key, help, children } => {
                let mut v = vec![("title", Json::s(title))];
                if !key.is_empty() {
                    v.push(("key", Json::s(key)));
                }
                if *help != 0 {
                    v.push(("help", Json::i(*help)));
                }
                v.push(("items", Json::Arr(children.iter().map(|c| c.to_json()).collect())));
                Json::obj(v)
            }
            MenuNode::Separator => Json::obj(vec![("separator", Json::Bool(true))]),
        }
    }
    pub fn from_json(j: &Json) -> MenuNode {
        if j.bool("separator", false) {
            MenuNode::Separator
        } else if j.get("items").is_some() {
            MenuNode::Sub {
                title: j.str("title"),
                key: j.str("key"),
                help: j.int("help", 0),
                children: j.arr("items").iter().map(MenuNode::from_json).collect(),
            }
        } else {
            MenuNode::Item { title: j.str("title"), command: j.str("command"), key: j.str("key"), help: j.int("help", 0) }
        }
    }
}

/// Элемент строки состояния. Пустой текст — «скрытая» клавиша.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusEntry {
    pub text: String,
    pub key: String,
    pub command: String,
}

/// Каркас программы: меню, строка состояния, команды.
#[derive(Clone, Debug, PartialEq)]
pub struct AppModel {
    /// Заголовок программы (для окна «О программе» и комментариев).
    pub title: String,
    pub menu: Vec<MenuNode>,
    pub status: Vec<StatusEntry>,
    pub commands: Vec<CommandDef>,
    /// Русские надписи стандартных диалогов tvision-rs.
    pub russian: bool,
    /// Форма, которая открывается при запуске программы (пусто — нет).
    pub startup_form: String,
}

impl AppModel {
    /// Модель новой программы: меню «Файл» и «Окно», строка состояния,
    /// команда ABOUT открывает форму `about`.
    pub fn starter(title: &str) -> Self {
        let item = |t: &str, c: &str, k: &str| MenuNode::Item { title: t.into(), command: c.into(), key: k.into(), help: 0 };
        AppModel {
            title: title.into(),
            menu: vec![
                MenuNode::Sub {
                    title: "~Ф~айл".into(),
                    key: String::new(),
                    help: 0,
                    children: vec![item("~О~ программе...", "ABOUT", ""), MenuNode::Separator, item("~В~ыход", "cm::QUIT", "Alt+X")],
                },
                MenuNode::Sub {
                    title: "~О~кно".into(),
                    key: String::new(),
                    help: 0,
                    children: vec![
                        item("~С~ледующее", "cm::NEXT", "F6"),
                        item("Р~а~спахнуть", "cm::ZOOM", "F5"),
                        item("~З~акрыть", "cm::CLOSE", "Alt+F3"),
                    ],
                },
            ],
            status: vec![
                StatusEntry { text: "~Alt+X~ Выход".into(), key: "Alt+X".into(), command: "cm::QUIT".into() },
                StatusEntry { text: "~F10~ Меню".into(), key: "F10".into(), command: "cm::MENU".into() },
                StatusEntry { text: String::new(), key: "Alt+F3".into(), command: "cm::CLOSE".into() },
                StatusEntry { text: String::new(), key: "F6".into(), command: "cm::NEXT".into() },
                StatusEntry { text: String::new(), key: "F5".into(), command: "cm::ZOOM".into() },
            ],
            commands: vec![CommandDef {
                name: "ABOUT".into(),
                value: 100,
                action: Action::Form("about".into()),
                comment: "окно «О программе»".into(),
            }],
            russian: true,
            startup_form: String::new(),
        }
    }

    /// Номер команды по имени (`cm::OK`, `ABOUT`, `cmd::ABOUT` или число).
    pub fn command_value(&self, name: &str) -> Option<u16> {
        let n = name.trim();
        if let Ok(v) = n.parse::<u16>() {
            return Some(v);
        }
        if let Some(&(_, v, _)) = BUILTIN_COMMANDS.iter().find(|(c, _, _)| *c == n) {
            return Some(v);
        }
        let n = n.strip_prefix("cmd::").unwrap_or(n);
        self.commands.iter().find(|c| c.name == n).map(|c| c.value)
    }

    /// Выражение Rust для команды: `cm::OK`, `cmd::ABOUT` или число.
    pub fn command_expr(&self, name: &str) -> String {
        let n = name.trim();
        if n.is_empty() {
            return "0".into();
        }
        if BUILTIN_COMMANDS.iter().any(|(c, _, _)| *c == n) || n.parse::<u16>().is_ok() {
            return n.to_string();
        }
        let n = n.strip_prefix("cmd::").unwrap_or(n);
        if self.commands.iter().any(|c| c.name == n) {
            format!("cmd::{}", n)
        } else {
            // Неизвестная команда — пусть компилятор подскажет.
            format!("cmd::{}", n)
        }
    }

    /// Свободный номер для новой команды (100..=255 можно запрещать).
    pub fn next_command_value(&self) -> u16 {
        (100..=255).chain(1000..u16::MAX).find(|v| !self.commands.iter().any(|c| c.value == *v)).unwrap_or(1000)
    }

    /// Все имена команд (стандартные и свои) — для подсказок.
    pub fn all_command_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.commands.iter().map(|c| c.name.clone()).collect();
        v.extend(BUILTIN_COMMANDS.iter().map(|(n, _, _)| n.to_string()));
        v
    }

    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("title", Json::s(&self.title)),
            ("russian", Json::Bool(self.russian)),
            ("startup_form", Json::s(&self.startup_form)),
            ("menu", Json::Arr(self.menu.iter().map(|m| m.to_json()).collect())),
            (
                "status",
                Json::Arr(
                    self.status
                        .iter()
                        .map(|s| Json::obj(vec![("text", Json::s(&s.text)), ("key", Json::s(&s.key)), ("command", Json::s(&s.command))]))
                        .collect(),
                ),
            ),
            (
                "commands",
                Json::Arr(
                    self.commands
                        .iter()
                        .map(|c| {
                            Json::obj(vec![
                                ("name", Json::s(&c.name)),
                                ("value", Json::i(c.value as i64)),
                                ("action", Json::s(&c.action.to_str())),
                                ("comment", Json::s(&c.comment)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    pub fn from_json(j: &Json) -> Self {
        AppModel {
            title: j.str("title"),
            russian: j.bool("russian", true),
            startup_form: j.str("startup_form"),
            menu: j.arr("menu").iter().map(MenuNode::from_json).collect(),
            status: j.arr("status").iter().map(|s| StatusEntry { text: s.str("text"), key: s.str("key"), command: s.str("command") }).collect(),
            commands: j
                .arr("commands")
                .iter()
                .map(|c| CommandDef {
                    name: c.str("name"),
                    value: c.int("value", 100) as u16,
                    action: Action::parse(&c.str("action")),
                    comment: c.str("comment"),
                })
                .collect(),
        }
    }
}

// ======================================================================
// Проект
// ======================================================================

/// Откуда брать tvision-rs в новом проекте.
#[derive(Clone, Debug, PartialEq)]
pub enum Dependency {
    /// Локальный путь (относительный — от каталога проекта).
    Path(String),
    /// Репозиторий git.
    Git(String),
}

impl Dependency {
    /// Определить вид зависимости по строке: ссылка (`https://…`,
    /// `git@…`, `ssh://…`) — git, иначе — путь на диске. Пустая строка — `None`.
    pub fn detect(s: &str) -> Option<Dependency> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if s.contains("://") || s.starts_with("git@") {
            Some(Dependency::Git(s.to_string()))
        } else {
            Some(Dependency::Path(s.to_string()))
        }
    }

    pub fn cargo_line(&self) -> String {
        match self {
            Dependency::Path(p) => format!("tvision-rs = {{ path = \"{}\" }}", cargo_path(p)),
            Dependency::Git(u) => format!("tvision-rs = {{ git = \"{}\" }}", u),
        }
    }
}

/// Тип проекта.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProjectKind {
    /// Приложение на tvision-rs: формы, модель меню и команд, генерация кода.
    #[default]
    Tvision,
    /// Обычная программа на Rust: только код, без форм и модели.
    Rust,
}

impl ProjectKind {
    pub const ALL: [ProjectKind; 2] = [ProjectKind::Tvision, ProjectKind::Rust];

    /// Надпись в диалоге «Новый проект».
    pub fn title(self) -> &'static str {
        match self {
            ProjectKind::Tvision => "Приложение на tvision-rs (формы, модель, код)",
            ProjectKind::Rust => "Программа на Rust (только код)",
        }
    }

    /// Значение поля `kind` в файле проекта.
    pub fn tag(self) -> &'static str {
        match self {
            ProjectKind::Tvision => "tvision",
            ProjectKind::Rust => "rust",
        }
    }

    /// Разбор поля `kind`; нет поля (старые проекты) — tvision-rs.
    pub fn from_tag(s: &str) -> ProjectKind {
        if s == "rust" {
            ProjectKind::Rust
        } else {
            ProjectKind::Tvision
        }
    }
}

/// Проект TV Studio.
#[derive(Clone, Debug)]
pub struct Project {
    /// Каталог проекта (там же `Cargo.toml`).
    pub dir: PathBuf,
    /// Имя проекта = имя пакета Cargo.
    pub name: String,
    /// Тип: приложение tvision-rs или обычная программа на Rust.
    pub kind: ProjectKind,
    pub app: AppModel,
    /// Имена форм (файлы `forms/<имя>.tvf`).
    pub forms: Vec<String>,
    /// Файлы, открытые в редакторе при прошлом сохранении (относительные пути).
    pub open_files: Vec<String>,
}

/// Расширение файла проекта.
pub const PROJECT_EXT: &str = "tvsp";
/// Расширение файла формы.
pub const FORM_EXT: &str = "tvf";

impl Project {
    /// Путь к файлу проекта.
    pub fn file(&self) -> PathBuf {
        self.dir.join(format!("{}.{}", self.name, PROJECT_EXT))
    }

    /// Есть ли у проекта формы и модель приложения (проект tvision-rs).
    pub fn has_forms(&self) -> bool {
        self.kind == ProjectKind::Tvision
    }

    /// Путь к файлу формы.
    pub fn form_path(&self, form: &str) -> PathBuf {
        self.dir.join("forms").join(format!("{}.{}", form, FORM_EXT))
    }

    /// Путь к сгенерированному модулю формы.
    pub fn form_code_path(&self, form: &str) -> PathBuf {
        self.dir.join("src").join("forms").join(format!("{}.rs", form))
    }

    /// Путь относительно каталога проекта (для заголовков и сообщений).
    pub fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.dir).unwrap_or(p).display().to_string().replace('\\', "/")
    }

    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("format", Json::s("tvstudio-project")),
            ("version", Json::i(1)),
            ("name", Json::s(&self.name)),
            ("kind", Json::s(self.kind.tag())),
            ("forms", Json::from_strings(&self.forms)),
            ("open_files", Json::from_strings(&self.open_files)),
            ("app", self.app.to_json()),
        ])
    }

    /// Загрузить проект из файла `*.tvsp` (TOML; файлы старых версий — JSON).
    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let j = crate::toml::parse_document(&s).map_err(|e| format!("{}: {}", path.display(), e))?;
        if j.str("format") != "tvstudio-project" {
            return Err(format!("{}: это не файл проекта TV Studio", path.display()));
        }
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
        let dir = tvision::widgets::editor::absolute(&dir);
        Ok(Project {
            dir,
            name: j.str("name"),
            kind: ProjectKind::from_tag(&j.str("kind")),
            app: AppModel::from_json(j.get("app").unwrap_or(&Json::Null)),
            forms: j.strings("forms"),
            open_files: j.strings("open_files"),
        })
    }

    /// Сохранить файл проекта в TOML.
    pub fn save(&self) -> Result<(), String> {
        let p = self.file();
        let text = crate::toml::to_string(&self.to_json())?;
        std::fs::write(&p, text).map_err(|e| format!("{}: {}", p.display(), e))
    }

    /// Каталог нового проекта `parent/name` (имя и отсутствие проекта проверены).
    fn new_dir(parent: &Path, name: &str) -> Result<PathBuf, String> {
        if !is_crate_name(name) {
            return Err(format!("«{}» не годится как имя пакета (латинские буквы, цифры, _ и -)", name));
        }
        let dir = tvision::widgets::editor::absolute(&parent.join(name));
        if dir.join("Cargo.toml").exists() {
            return Err(format!("в каталоге {} уже есть проект Cargo", dir.display()));
        }
        Ok(dir)
    }

    /// Создать проект — обычную программу на Rust: каталог, `Cargo.toml`
    /// без зависимостей, `src/main.rs` и файл проекта. Ни форм, ни модели.
    pub fn create_rust(parent: &Path, name: &str) -> Result<Self, String> {
        let dir = Self::new_dir(parent, name)?;
        let src = dir.join("src");
        std::fs::create_dir_all(&src).map_err(|e| format!("{}: {}", src.display(), e))?;
        let write = |p: PathBuf, s: String| std::fs::write(&p, s).map_err(|e| format!("{}: {}", p.display(), e));
        write(dir.join("Cargo.toml"), format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n", name))?;
        write(dir.join(".gitignore"), "/target\n".into())?;
        write(src.join("main.rs"), "fn main() {\n    println!(\"Hello, world!\");\n}\n".into())?;
        let p = Project {
            dir,
            name: name.into(),
            kind: ProjectKind::Rust,
            app: AppModel::starter(name),
            forms: vec![],
            open_files: vec!["src/main.rs".into()],
        };
        p.save()?;
        Ok(p)
    }

    /// Взять в проект готовые исходники на Rust: каталог `dir` с `Cargo.toml`.
    /// Рядом с `Cargo.toml` создаётся файл проекта типа «программа на Rust»
    /// (имя — из `[package] name`, иначе имя каталога); исходники не
    /// трогаются. Открыты будут `src/main.rs` или `src/lib.rs`.
    pub fn adopt_rust(dir: &Path) -> Result<Self, String> {
        let dir = tvision::widgets::editor::absolute(dir);
        let cargo = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&cargo).map_err(|_| format!("в каталоге {} нет Cargo.toml — это не исходники Rust", dir.display()))?;
        let name = cargo_package_name(&text)
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().to_string()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "project".into());
        let open_files = ["src/main.rs", "src/lib.rs"].iter().find(|f| dir.join(f).is_file()).map(|f| vec![f.to_string()]).unwrap_or_default();
        let p = Project { dir, name: name.clone(), kind: ProjectKind::Rust, app: AppModel::starter(&name), forms: vec![], open_files };
        if p.file().exists() {
            return Err(format!("файл проекта {} уже есть", p.file().display()));
        }
        p.save()?;
        Ok(p)
    }

    /// Создать новый проект tvision-rs: каталог, `Cargo.toml`, `src/main.rs`,
    /// файл проекта и форму «О программе».
    pub fn create(parent: &Path, name: &str, dep: &Dependency) -> Result<Self, String> {
        let dir = Self::new_dir(parent, name)?;
        if let Dependency::Path(p) = dep {
            // Относительный путь считается от каталога проекта (как в Cargo).
            let lib = normalize(&if Path::new(p).is_absolute() { PathBuf::from(p) } else { dir.join(p) });
            if !lib.join("Cargo.toml").exists() {
                return Err(format!(
                    "по пути «{}» нет tvision-rs (не найден {}).\nУкажите каталог библиотеки или ссылку git https://github.com/cat386/tvision-rs",
                    p,
                    lib.join("Cargo.toml").display()
                ));
            }
        }
        let mk = |p: &Path| std::fs::create_dir_all(p).map_err(|e| format!("{}: {}", p.display(), e));
        mk(&dir.join("src").join("forms"))?;
        mk(&dir.join("forms"))?;
        let write = |p: PathBuf, s: String| std::fs::write(&p, s).map_err(|e| format!("{}: {}", p.display(), e));
        write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{}\n", name, dep.cargo_line()),
        )?;
        write(dir.join(".gitignore"), "/target\n".into())?;
        let title = name.to_string();
        let p = Project {
            dir: dir.clone(),
            name: name.into(),
            kind: ProjectKind::Tvision,
            app: AppModel::starter(&title),
            forms: vec!["about".into()],
            open_files: vec![],
        };
        write(dir.join("src").join("main.rs"), crate::codegen::main_rs(&p))?;
        about_form(&title).save(&p.form_path("about"))?;
        p.save()?;
        Ok(p)
    }
}

/// Путь для `Cargo.toml`: прямые косые черты; без приставки `\\?\` длинных
/// путей Windows (её даёт `canonicalize`, а cargo такой путь не понимает):
/// `\\?\D:\x` → `D:/x`, `\\?\UNC\srv\share` → `//srv/share`.
pub fn cargo_path(p: &str) -> String {
    let p = if let Some(rest) = p.strip_prefix(r"\\?\UNC\") { format!(r"\\{}", rest) } else { p.strip_prefix(r"\\?\").unwrap_or(p).to_string() };
    p.replace('\\', "/")
}

/// Имя пакета из текста `Cargo.toml`: `name = "…"` в разделе `[package]`.
/// `None` — раздела нет (например, только `[workspace]`) или имя не строкой.
pub fn cargo_package_name(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let l = line.split('#').next().unwrap_or("").trim();
        if l.starts_with('[') {
            in_package = l == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        if k.trim() == "name" {
            let v = v.trim();
            let q = v.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            return v[1..].split(q).next().map(|s| s.to_string()).filter(|s| !s.is_empty());
        }
    }
    None
}

/// Убрать из пути `.` и `..` без обращения к диску (каталога проекта
/// ещё нет, и ОС не раскрыла бы `..` через него).
pub fn normalize(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Форма «О программе» нового проекта.
pub fn about_form(title: &str) -> Form {
    let mut f = Form::new("about", "О программе");
    f.w = 40;
    f.h = 11;
    let mut t = Control::new(Kind::StaticText, "info", 3, 2);
    t.w = 34;
    t.h = 4;
    t.text = format!("\x03{}\n\n\x03Создано в TV Studio\n\x03на tvision-rs", title);
    f.controls.push(t);
    let mut b = Control::new(Kind::Button, "ok", 14, 7);
    b.default = true;
    f.controls.push(b);
    f
}

// ======================================================================
// Клавиши
// ======================================================================

/// Разбор сочетания клавиш: `F2`, `Alt+X`, `Ctrl+N`, `Shift+F6`,
/// `Ctrl+Ins`, `Shift+Del`. Разделитель — `+` или `-`.
pub fn parse_key(s: &str) -> Option<KeyEvent> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split(['+', '-']).filter(|p| !p.is_empty()).collect();
    let (last, mods_part) = parts.split_last()?;
    let mut mods = Mods::NONE;
    for m in mods_part {
        match m.to_lowercase().as_str() {
            "ctrl" | "ctl" | "control" => mods.ctrl = true,
            "alt" => mods.alt = true,
            "shift" => mods.shift = true,
            _ => return None,
        }
    }
    let l = last.to_lowercase();
    let key = match l.as_str() {
        "enter" => Key::Enter,
        "esc" | "escape" => Key::Esc,
        "tab" => Key::Tab,
        "backspace" | "bksp" => Key::Backspace,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "pgup" | "pageup" => Key::PgUp,
        "pgdn" | "pagedown" => Key::PgDn,
        "ins" | "insert" => Key::Ins,
        "del" | "delete" => Key::Del,
        "space" => Key::Char(' '),
        _ if l.len() >= 2 && l.starts_with('f') && l[1..].chars().all(|c| c.is_ascii_digit()) => {
            let n: u8 = l[1..].parse().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            Key::F(n)
        }
        _ if last.chars().count() == 1 => Key::Char(last.chars().next()?.to_ascii_uppercase()),
        _ => return None,
    };
    Some(KeyEvent::new(key, mods))
}

/// Выражение Rust, создающее сочетание клавиш.
pub fn key_expr(k: &KeyEvent) -> String {
    let key = match k.key {
        Key::Char(c) => format!("Key::Char({:?})", c),
        Key::F(n) => format!("Key::F({})", n),
        other => format!("Key::{:?}", other),
    };
    let m = k.mods;
    let mods = match (m.shift, m.ctrl, m.alt) {
        (false, false, false) => "Mods::NONE".to_string(),
        (true, false, false) => "Mods::SHIFT".to_string(),
        (false, true, false) => "Mods::CTRL".to_string(),
        (false, false, true) => "Mods::ALT".to_string(),
        (true, true, false) => "Mods::CTRL_SHIFT".to_string(),
        (s, c, a) => format!("Mods {{ shift: {}, ctrl: {}, alt: {} }}", s, c, a),
    };
    format!("KeyEvent::new({}, {})", key, mods)
}

// ======================================================================
// Имена
// ======================================================================

/// Годится ли строка как идентификатор Rust (латиница, цифры, `_`).
pub fn is_ident(s: &str) -> bool {
    let mut ch = s.chars();
    match ch.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    ch.all(|c| c.is_ascii_alphanumeric() || c == '_') && !is_keyword(s)
}

/// Годится ли строка как имя пакета Cargo.
pub fn is_crate_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Ключевые слова Rust (их нельзя брать именами).
pub fn is_keyword(s: &str) -> bool {
    crate::highlight::RUST_KEYWORDS.contains(&s)
}

/// `about_dialog` → `AboutDialog`.
pub fn camel(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Строка → литерал Rust в кавычках.
pub fn rust_str(s: &str) -> String {
    format!("{:?}", s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(parse_key("Alt+X"), Some(KeyEvent::new(Key::Char('X'), Mods::ALT)));
        assert_eq!(parse_key("F10"), Some(KeyEvent::new(Key::F(10), Mods::NONE)));
        assert_eq!(parse_key("Ctrl-F5"), Some(KeyEvent::new(Key::F(5), Mods::CTRL)));
        assert_eq!(parse_key("Shift+Del"), Some(KeyEvent::new(Key::Del, Mods::SHIFT)));
        assert_eq!(parse_key("Hyper+Q"), None);
        assert_eq!(key_expr(&parse_key("alt+x").unwrap()), "KeyEvent::new(Key::Char('X'), Mods::ALT)");
    }

    /// Проект на Rust: только Cargo.toml, main.rs и файл проекта; тип
    /// сохраняется, а старые файлы проекта без поля `kind` — tvision-rs.
    #[test]
    fn rust_project() {
        let base = std::env::temp_dir().join(format!("tvs_rustproj_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let p = Project::create_rust(&base, "plain").unwrap();
        assert!(!p.has_forms());
        let cargo = std::fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"plain\"") && !cargo.contains("tvision"), "{}", cargo);
        assert!(std::fs::read_to_string(p.dir.join("src/main.rs")).unwrap().contains("Hello, world!"));
        assert!(!p.dir.join("forms").exists() && !p.dir.join("src/forms").exists());
        let q = Project::load(&p.file()).unwrap();
        assert_eq!((q.kind, q.forms.len()), (ProjectKind::Rust, 0));
        assert!(Project::create_rust(&base, "plain").is_err(), "второй раз в тот же каталог");
        // Старый файл проекта (без kind) — проект tvision-rs.
        let text = std::fs::read_to_string(p.file()).unwrap();
        assert!(text.starts_with("format = \"tvstudio-project\"\n"), "файл проекта — TOML:\n{}", text);
        let mut j = crate::toml::parse_document(&text).unwrap();
        if let Json::Obj(v) = &mut j {
            v.retain(|(k, _)| k != "kind");
        }
        // …и записан в старом формате JSON — тоже читается.
        std::fs::write(p.file(), j.to_pretty()).unwrap();
        assert_eq!(Project::load(&p.file()).unwrap().kind, ProjectKind::Tvision);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Пути в Cargo.toml: без приставки `\\?\` и с прямыми косыми.
    #[test]
    fn cargo_paths() {
        assert_eq!(cargo_path(r"\\?\D:\a\tvision-rs"), "D:/a/tvision-rs");
        assert_eq!(cargo_path(r"\\?\UNC\srv\share\tv"), "//srv/share/tv");
        assert_eq!(cargo_path(r"D:\develop\tvision-rs"), "D:/develop/tvision-rs");
        assert_eq!(cargo_path("../tvision-rs"), "../tvision-rs");
        assert_eq!(Dependency::Path(r"\\?\C:\x".into()).cargo_line(), "tvision-rs = { path = \"C:/x\" }");
    }

    /// Каталог с Cargo.toml берётся в проект на Rust; имя — из [package].
    #[test]
    fn adopt_cargo_dir() {
        assert_eq!(cargo_package_name("[package]\nname = \"my-tool\" # имя\n[dependencies]\nname = \"x\"\n").as_deref(), Some("my-tool"));
        assert_eq!(cargo_package_name("[dependencies]\nname = \"x\"\n[package]\nversion = \"1\"\nname='q'\n").as_deref(), Some("q"));
        assert_eq!(cargo_package_name("[workspace]\nmembers = [\"a\"]\n"), None);
        assert_eq!(cargo_package_name("[package]\nname.workspace = true\n"), None);

        let base = std::env::temp_dir().join(format!("tvs_adopt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("tool dir");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        assert!(Project::adopt_rust(&dir).unwrap_err().contains("нет Cargo.toml"));
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"my-tool\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
        let p = Project::adopt_rust(&dir).unwrap();
        assert_eq!((p.name.as_str(), p.kind), ("my-tool", ProjectKind::Rust));
        assert_eq!(p.file(), dir.join("my-tool.tvsp"));
        assert_eq!(p.open_files, vec!["src/lib.rs".to_string()]);
        assert_eq!(std::fs::read_to_string(dir.join("src/lib.rs")).unwrap(), "pub fn f() {}\n", "исходники не тронуты");
        assert!(Project::adopt_rust(&dir).is_err(), "файл проекта уже есть");
        // Workspace без [package] — имя каталога.
        let ws = base.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(ws.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        let p = Project::adopt_rust(&ws).unwrap();
        assert_eq!((p.name.as_str(), p.open_files.len()), ("ws", 0));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn form_roundtrip() {
        let mut f = about_form("Тест");
        let mut l = f.new_control(Kind::Label, 2, 2);
        l.link = "info".into();
        f.controls.push(l);
        let j = crate::toml::to_string(&f.to_json()).unwrap();
        let g = Form::from_json(&crate::toml::parse(&j).unwrap()).unwrap();
        assert_eq!(f, g);
        assert_eq!(f.unique_name("label"), "label2");
    }

    #[test]
    fn app_roundtrip() {
        let a = AppModel::starter("x");
        let doc = Json::obj(vec![("app", a.to_json())]);
        let b = AppModel::from_json(crate::toml::parse(&crate::toml::to_string(&doc).unwrap()).unwrap().get("app").unwrap());
        assert_eq!(a, b);
        assert_eq!(a.command_value("ABOUT"), Some(100));
        assert_eq!(a.command_value("cm::QUIT"), Some(1));
        assert_eq!(a.command_expr("ABOUT"), "cmd::ABOUT");
        assert_eq!(a.next_command_value(), 101);
    }

    #[test]
    fn dependency_detect() {
        assert_eq!(
            Dependency::detect(" https://github.com/cat386/tvision-rs "),
            Some(Dependency::Git("https://github.com/cat386/tvision-rs".into()))
        );
        assert_eq!(Dependency::detect("git@github.com:cat386/tvision-rs.git"), Some(Dependency::Git("git@github.com:cat386/tvision-rs.git".into())));
        assert_eq!(Dependency::detect("D:\\develop\\tvision-rs"), Some(Dependency::Path("D:\\develop\\tvision-rs".into())));
        assert_eq!(Dependency::detect(""), None);
        // Несуществующий путь — понятная ошибка, а не сломанный Cargo.toml.
        let base = std::env::temp_dir().join(format!("tvs_dep_{}", std::process::id()));
        let e = Project::create(&base, "x", &Dependency::Path("../нет-такой".into())).unwrap_err();
        assert!(e.contains("нет tvision-rs"), "{}", e);
        assert!(!base.join("x").exists());
    }

    #[test]
    fn names() {
        assert!(is_ident("ok_button"));
        assert!(!is_ident("1x"));
        assert!(!is_ident("fn"));
        assert_eq!(camel("about_dialog"), "AboutDialog");
    }
}
