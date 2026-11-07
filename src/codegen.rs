//! Генерация кода на Rust по модели проекта.
//!
//! Что создаётся:
//! * `src/ui.rs` — команды (`ui::cmd`), строка меню, строка состояния,
//!   действия команд (`ui::handle_command`), настройка при запуске;
//! * `src/forms/mod.rs` и `src/forms/<форма>.rs` — для каждой формы:
//!   структура данных, `build()` (построить диалог) и `run()` (показать
//!   модально и прочитать данные);
//! * `src/main.rs` — только если его ещё нет: это файл программиста.
//!
//! Сгенерированные файлы помечены первой строкой [`GENERATED_MARK`] и
//! перезаписываются при каждой генерации.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::builder::input_width;
use crate::model::{build_order, camel, key_expr, parse_key, parse_tree, rust_str, Action, AppModel, Control, Form, Kind, MenuNode, Pal, Project};

/// Метка сгенерированных файлов (первая строка).
pub const GENERATED_MARK: &str = "// Сгенерировано TV Studio. Не редактируйте: файл перезаписывается.";

// ======================================================================
// main.rs — создаётся один раз
// ======================================================================

/// Текст `src/main.rs` нового проекта.
pub fn main_rs(p: &Project) -> String {
    format!(
        r#"//! Программа «{title}».
//!
//! Каркас создан TV Studio. Этот файл ваш: здесь пишется поведение
//! программы. Меню, строка состояния, команды и формы описаны в модели
//! проекта ({name}.tvsp) и генерируются в `ui.rs` и `forms/` при каждой
//! сборке — их руками не правят.

mod forms;
mod ui;

use tvision::*;

/// Обработчик событий программы (то, что в C++ было `TApplication::handleEvent`).
struct App;

impl AppHandler for App {{
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {{
        let Some(command) = ev.command() else {{ return }};
        // Действия, заданные в модели: открыть форму, показать сообщение.
        if ui::handle_command(app, command) {{
            ev.clear();
            return;
        }}
        // Свои команды обрабатывайте здесь, например:
        // if command == ui::cmd::MY_COMMAND {{
        //     message_box(app, "Привет!", mf::INFORMATION | mf::OK_BUTTON);
        //     ev.clear();
        // }}
    }}
}}

fn main() -> std::io::Result<()> {{
    ui::setup();
    let mut app = Program::with_terminal(Some(ui::menu_bar()), Some(ui::status_line()))?;
    ui::startup(&mut app);
    app.run(&mut App);
    Ok(())
}}
"#,
        title = p.app.title.replace('"', "'"),
        name = p.name
    )
}

// ======================================================================
// ui.rs
// ======================================================================

/// Выражение для клавиши; `None`, если строка пустая или неверная.
fn key_code(s: &str) -> Option<String> {
    parse_key(s).map(|k| key_expr(&k))
}

fn menu_code(app: &AppModel, items: &[MenuNode], indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    out.push_str("Menu::new()");
    for it in items {
        out.push('\n');
        out.push_str(&pad);
        match it {
            MenuNode::Item { title, command, key, help } => {
                let cmd = app.command_expr(command);
                match key_code(key) {
                    Some(k) => {
                        let _ = write!(out, "    .item_key({}, {}, {}, {})", rust_str(title), cmd, k, rust_str(key.trim()));
                    }
                    None => {
                        let _ = write!(out, "    .item({}, {})", rust_str(title), cmd);
                    }
                }
                if *help != 0 {
                    let _ = write!(out, "\n{}    .help({})", pad, help);
                }
            }
            MenuNode::Separator => out.push_str("    .separator()"),
            MenuNode::Sub { title, key, help, children } => {
                match key_code(key) {
                    Some(k) => {
                        let _ = write!(out, "    .submenu_key(\n{}        {},\n{}        {},\n{}        ", pad, rust_str(title), pad, k, pad);
                    }
                    None => {
                        let _ = write!(out, "    .submenu(\n{}        {},\n{}        ", pad, rust_str(title), pad);
                    }
                }
                menu_code(app, children, indent + 2, out);
                let _ = write!(out, ",\n{}    )", pad);
                if *help != 0 {
                    let _ = write!(out, "\n{}    .help({})", pad, help);
                }
            }
        }
    }
}

/// Текст `src/ui.rs`.
pub fn ui_rs(p: &Project, forms: &[Form]) -> String {
    let app = &p.app;
    let mut s = String::new();
    let _ = writeln!(s, "{}", GENERATED_MARK);
    let _ = writeln!(s, "//! Каркас программы «{}»: команды, меню, строка состояния, действия.", app.title);
    let _ = writeln!(s, "//! Источник — модель проекта `{}.tvsp` (Проект → Модель приложения).", p.name);
    s.push_str("#![allow(dead_code, unused_imports, unused_variables)]\n\nuse tvision::*;\n\n");

    // --- Команды ---
    s.push_str("/// Свои команды программы. Номера 100..=255 можно запрещать\n");
    s.push_str("/// (`app.disable_command`), остальные разрешены всегда.\npub mod cmd {\n");
    for c in &app.commands {
        if !c.comment.is_empty() {
            let _ = writeln!(s, "    /// {}", c.comment);
        }
        let _ = writeln!(s, "    pub const {}: u16 = {};", c.name, c.value);
    }
    s.push_str("}\n\n");

    // --- Меню ---
    s.push_str("/// Строка меню.\npub fn menu_bar() -> Menu {\n    ");
    menu_code(app, &app.menu, 1, &mut s);
    s.push_str("\n}\n\n");

    // --- Строка состояния ---
    s.push_str("/// Строка состояния.\npub fn status_line() -> Vec<StatusDef> {\n    vec![StatusDef::all(vec![\n");
    for e in &app.status {
        let cmd = app.command_expr(&e.command);
        match key_code(&e.key) {
            Some(k) => {
                let _ = writeln!(s, "        StatusItem::new({}, {}, {}),", rust_str(&e.text), k, cmd);
            }
            None => {
                let _ = writeln!(s, "        StatusItem {{ text: {}.into(), key: None, command: {} }},", rust_str(&e.text), cmd);
            }
        }
    }
    s.push_str("    ])]\n}\n\n");

    // --- Действия ---
    s.push_str("/// Действия команд из модели. Возвращает `true`, если команда обработана.\n");
    s.push_str("pub fn handle_command(app: &mut Program, command: u16) -> bool {\n    match command {\n");
    for c in &app.commands {
        match &c.action {
            Action::None => {}
            Action::Form(f) if forms.iter().any(|x| &x.name == f) => {
                let _ = writeln!(
                    s,
                    "        cmd::{} => {{\n            let mut data = crate::forms::{f}::{}Data::default();\n            crate::forms::{f}::run(app, &mut data);\n            true\n        }}",
                    c.name,
                    camel(f),
                    f = f
                );
            }
            Action::Form(f) => {
                let _ = writeln!(s, "        // cmd::{}: формы «{}» нет в проекте", c.name, f);
            }
            Action::Message(m) => {
                let _ = writeln!(
                    s,
                    "        cmd::{} => {{\n            message_box(app, {}, mf::INFORMATION | mf::OK_BUTTON);\n            true\n        }}",
                    c.name,
                    rust_str(m)
                );
            }
        }
    }
    s.push_str("        _ => false,\n    }\n}\n\n");

    // --- Настройка ---
    s.push_str("/// Настройка до создания программы.\npub fn setup() {\n");
    if app.russian {
        s.push_str("    // Русские надписи стандартных диалогов (Да/Нет/Отмена, файлы...).\n");
        s.push_str("    locale::set_language(locale::Language::Ru);\n");
    }
    s.push_str("}\n\n/// Действия сразу после запуска (стартовая форма).\npub fn startup(app: &mut Program) {\n");
    if !app.startup_form.is_empty() && forms.iter().any(|f| f.name == app.startup_form) {
        let f = &app.startup_form;
        let _ = writeln!(s, "    let mut data = crate::forms::{f}::{}Data::default();\n    crate::forms::{f}::run(app, &mut data);", camel(f), f = f);
    } else {
        s.push_str("    let _ = app;\n");
    }
    s.push_str("}\n");
    s
}

/// Текст `src/forms/mod.rs`.
pub fn forms_mod_rs(forms: &[Form]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{}", GENERATED_MARK);
    s.push_str("//! Формы программы (по одному модулю на форму).\n\n");
    for f in forms {
        let _ = writeln!(s, "/// Форма «{}».\npub mod {};", f.title, f.name);
    }
    s
}

// ======================================================================
// Форма
// ======================================================================

/// Значение по умолчанию поля данных.
fn data_default(c: &Control) -> String {
    match c.kind {
        Kind::InputLine | Kind::ComboBox | Kind::Memo => format!("{}.to_string()", rust_str(&c.text)),
        Kind::CheckBoxes | Kind::RadioButtons => c.value.max(0).to_string(),
        Kind::ListBox | Kind::ProgressBar | Kind::Table => c.value.to_string(),
        _ => "()".into(),
    }
}

/// Вариант `Value` для поля.
fn value_ctor(c: &Control) -> (&'static str, bool) {
    match c.kind {
        Kind::InputLine | Kind::ComboBox | Kind::Memo => ("Str", true),
        Kind::CheckBoxes | Kind::RadioButtons => ("U32", false),
        _ => ("Int", false),
    }
}

fn rect(x: i32, y: i32, x2: i32, y2: i32) -> String {
    format!("Rect::new({}, {}, {}, {})", x, y, x2, y2)
}

/// Строки настройки, общие для всех элементов (`v` — переменная виджета).
fn common_setup(c: &Control, vs: &str, out: &mut Vec<String>) {
    if c.disabled {
        out.push(format!("{}.state |= sf::DISABLED;", vs));
    }
    if c.help_ctx != 0 {
        out.push(format!("{}.help_ctx = {};", vs, c.help_ctx));
    }
}

/// Код вставки элемента. Возвращает строки (без отступа).
fn control_code(app: &AppModel, c: &Control) -> Vec<String> {
    let mut lines = Vec::new();
    let r = rect(c.x, c.y, c.x + c.w, c.y + c.h);
    let name = &c.name;
    let mut setup = Vec::new();
    // `create` — выражение конструктора; `setup` — строки настройки `v`.
    let create: String = match c.kind {
        Kind::StaticText => format!("StaticText::new({}, {})", r, rust_str(&c.text)),
        Kind::Label => {
            let link = if c.link.is_empty() { "None".into() } else { format!("Some({})", c.link) };
            format!("Label::new({}, {}, {})", r, rust_str(&c.text), link)
        }
        Kind::Button => {
            let flags = if c.default { "bf::DEFAULT" } else { "bf::NORMAL" };
            format!("Button::new({}, {}, {}, {})", r, rust_str(&c.text), app.command_expr(&c.command), flags)
        }
        Kind::InputLine | Kind::ComboBox => {
            let iw = input_width(c);
            if !c.text.is_empty() {
                setup.push(format!("v.set_text({});", rust_str(&c.text)));
            }
            if c.kind == Kind::InputLine {
                if let Some(v) = validator_code(&c.validator) {
                    setup.push(format!("v.validator = Some(Box::new({}));", v));
                }
            }
            format!("InputLine::new({}, {})", rect(c.x, c.y, c.x + iw, c.y + 1), c.max_len.max(1))
        }
        Kind::CheckBoxes | Kind::RadioButtons => {
            let items: Vec<String> = c.items.iter().map(|s| rust_str(s)).collect();
            if c.value != 0 {
                setup.push(format!("v.value = {};", c.value.max(0)));
            }
            let f = if c.kind == Kind::CheckBoxes { "check_boxes" } else { "radio_buttons" };
            format!("Cluster::{}({}, &[{}])", f, r, items.join(", "))
        }
        Kind::ListBox | Kind::Memo => {
            let (vr, sb) = if c.scrollbar && c.w > 1 {
                lines.push(format!("let {}_sb = d.add(ScrollBar::new({}));", name, rect(c.x + c.w - 1, c.y, c.x + c.w, c.y + c.h)));
                (rect(c.x, c.y, c.x + c.w - 1, c.y + c.h), format!("Some({}_sb)", name))
            } else {
                (r.clone(), "None".into())
            };
            if c.kind == Kind::ListBox {
                let items: Vec<String> = c.items.iter().map(|s| format!("{}.to_string()", rust_str(s))).collect();
                setup.push(format!("v.set_items(vec![{}]);", items.join(", ")));
                if c.value > 0 {
                    setup.push(format!("v.focus_item({});", c.value));
                }
                format!("ListBox::new({}, {}, {})", vr, c.columns.max(1), sb)
            } else {
                if !c.text.is_empty() {
                    setup.push(format!("v.editor.set_text({});", rust_str(&c.text)));
                    setup.push("v.editor.modified = false;".into());
                }
                format!("Memo::new({}, None, {})", vr, sb)
            }
        }
        Kind::ScrollBar => format!("ScrollBar::new({})", r),
        Kind::ProgressBar => {
            if c.value != 0 {
                setup.push(format!("v.set_value({});", c.value));
            }
            format!("ProgressBar::new({}, {})", r, c.max.max(1))
        }
        Kind::Table => {
            let (vr, sb) = scroll_code(c, &mut lines);
            let cols = c.table_columns();
            let headers: Vec<String> = cols.iter().map(|(t, _)| rust_str(t)).collect();
            let mut m = format!("let {}_model = VecTableModel::new(\n    &[{}],\n    vec![\n", name, headers.join(", "));
            for r in c.table_rows() {
                let cells: Vec<String> = r.iter().map(|x| format!("{}.into()", rust_str(x))).collect();
                m.push_str(&format!("        vec![{}],\n", cells.join(", ")));
            }
            m.push_str(&format!("    ],\n){};", if c.editable { "\n.editable(true)" } else { "" }));
            lines.push(m);
            let col_defs: Vec<String> = cols.iter().map(|(t, w)| format!("TableColumn::new({}).title({})", w, rust_str(t))).collect();
            if c.value > 0 {
                setup.push(format!("v.set_cursor({}, 0);", c.value));
            }
            format!(
                "Table::new({}, Box::new({}_model), {}, None)\n        .with_columns(vec![{}])\n        .header({})\n        .grid({})\n        .editable({})\n        .row_numbers({})",
                vr,
                name,
                sb,
                col_defs.join(", "),
                c.header,
                c.grid,
                c.editable,
                c.row_numbers
            )
        }
        Kind::TreeView => {
            let (vr, sb) = scroll_code(c, &mut lines);
            setup.push("v.vs.grow_mode = 0;".into());
            let nodes: Vec<String> = parse_tree(&c.items).iter().map(|t| tree_code(t, c.expanded)).collect();
            let nodes =
                if nodes.is_empty() { "vec![]".to_string() } else { format!("vec![\n            {},\n        ]", nodes.join(",\n            ")) };
            format!(
                "TreeView::new(\n        {},\n        {},\n        {},\n        None,\n    )\n    .lines({})\n    .checkboxes({})",
                vr, nodes, sb, c.lines, c.checks
            )
        }
    };
    common_setup(c, if c.kind == Kind::Memo { "v.editor.vs" } else { "v.vs" }, &mut setup);
    if setup.is_empty() {
        lines.push(format!("let {} = d.add({});", name, create));
    } else {
        lines.push(format!("let {} = {{", name));
        lines.push(format!("    let mut v = {};", create));
        for s in setup {
            lines.push(format!("    {}", s));
        }
        lines.push("    d.add(v)".into());
        lines.push("};".into());
    }
    // Значок истории или кнопка списка — справа от строки ввода.
    let side = rect(c.x + input_width(c), c.y, c.x + c.w, c.y + 1);
    if c.kind == Kind::ComboBox {
        let items: Vec<String> = c.items.iter().map(|s| rust_str(s)).collect();
        lines.push(format!("d.add(Combo::new({}, {}, cbx::NO_TRANSFER, &[{}]));", side, name, items.join(", ")));
    } else if c.kind == Kind::InputLine && c.history > 0 {
        lines.push(format!("d.add(History::new({}, {}, {}));", side, name, c.history));
    }
    lines
}

/// Полоса прокрутки элемента (в последнем столбце): строка вставки и
/// (границы элемента, выражение id полосы).
fn scroll_code(c: &Control, lines: &mut Vec<String>) -> (String, String) {
    if c.scrollbar && c.w > 1 {
        lines.push(format!("let {}_sb = d.add(ScrollBar::new({}));", c.name, rect(c.x + c.w - 1, c.y, c.x + c.w, c.y + c.h)));
        (rect(c.x, c.y, c.x + c.w - 1, c.y + c.h), format!("Some({}_sb)", c.name))
    } else {
        (rect(c.x, c.y, c.x + c.w, c.y + c.h), "None".into())
    }
}

/// Выражение `TreeNode` с детьми.
fn tree_code(t: &crate::model::TreeItem, expanded: bool) -> String {
    let mut s = format!("TreeNode::new({})", rust_str(&t.text));
    if expanded && !t.children.is_empty() {
        s.push_str(".expanded(true)");
    }
    for ch in &t.children {
        s.push_str(&format!(".child({})", tree_code(ch, expanded)));
    }
    s
}

/// Код валидатора по описанию.
fn validator_code(spec: &str) -> Option<String> {
    let (kind, arg) = spec.split_once(':')?;
    match kind.trim() {
        "picture" => Some(format!("PictureValidator::new({}, true)", rust_str(arg))),
        "filter" => Some(format!("FilterValidator::new({})", rust_str(arg))),
        "range" => crate::props::parse_range(arg).ok().map(|(a, b)| format!("RangeValidator::new({}, {})", a, b)),
        "lookup" => {
            let items: Vec<String> = arg.split('|').map(|s| rust_str(s.trim())).collect();
            Some(format!("StringLookupValidator::new(&[{}])", items.join(", ")))
        }
        _ => None,
    }
}

/// Текст модуля формы `src/forms/<имя>.rs`.
pub fn form_rs(p: &Project, f: &Form) -> String {
    let app = &p.app;
    let ty = camel(&f.name);
    let data = f.data_controls();
    let mut s = String::new();
    let _ = writeln!(s, "{}", GENERATED_MARK);
    let _ = writeln!(s, "//! Форма «{}» — источник `forms/{}.tvf` (меняйте её в дизайнере).", f.title, f.name);
    s.push_str("#![allow(dead_code, unused_imports, unused_mut, unused_variables)]\n\n");
    s.push_str("use tvision::*;\n\nuse crate::ui::cmd;\n\n");

    // --- Данные ---
    s.push_str("/// Данные формы — то, чем диалог обменивается с программой\n/// (`set_data`/`get_data`). Порядок полей = порядок элементов.\n");
    let _ = writeln!(s, "#[derive(Clone, Debug, PartialEq)]\npub struct {}Data {{", ty);
    for c in &data {
        let _ = writeln!(s, "    /// {} «{}».\n    pub {}: {},", c.kind.title(), c.name, c.name, c.kind.data_type());
    }
    s.push_str("}\n\n");
    let _ = writeln!(
        s,
        "impl Default for {}Data {{\n    /// Начальные значения — как на форме в дизайнере.\n    fn default() -> Self {{\n        Self {{",
        ty
    );
    for c in &data {
        let _ = writeln!(s, "            {}: {},", c.name, data_default(c));
    }
    s.push_str("        }\n    }\n}\n\n");
    let _ = writeln!(s, "impl {}Data {{", ty);
    s.push_str("    /// Данные → значение для `Dialog::set_data`.\n    pub fn to_value(&self) -> Value {\n        Value::List(vec![");
    for c in &data {
        let (v, clone) = value_ctor(c);
        let _ = write!(s, "\n            Value::{}(self.{}{}),", v, c.name, if clone { ".clone()" } else { "" });
    }
    if !data.is_empty() {
        s.push_str("\n        ");
    }
    s.push_str("])\n    }\n\n");
    s.push_str(
        "    /// Значение из `Dialog::get_data` → данные.\n    pub fn from_value(v: &Value) -> Self {\n        let mut d = Self::default();\n",
    );
    s.push_str("        let mut it = v.as_list().unwrap_or(&[]).iter();\n");
    for c in &data {
        let (v, clone) = value_ctor(c);
        let _ = writeln!(
            s,
            "        if let Some(Value::{}(x)) = it.next() {{\n            d.{} = {};\n        }}",
            v,
            c.name,
            if clone { "x.clone()" } else { "*x" }
        );
    }
    s.push_str("        d\n    }\n}\n\n");

    // --- id элементов ---
    s.push_str("/// id элементов построенного диалога (для доступа из своего кода:\n/// `dialog.win.group.find_mut::<InputLine>(ids.name)`).\n");
    let _ = writeln!(s, "#[derive(Clone, Copy, Debug)]\npub struct {}Ids {{", ty);
    for c in &f.controls {
        let _ = writeln!(s, "    pub {}: ViewId,", c.name);
    }
    s.push_str("}\n\n");

    // --- build ---
    let _ = writeln!(s, "/// Построить диалог «{}».\npub fn build() -> (Dialog, {}Ids) {{", f.title, ty);
    let _ = writeln!(s, "    let mut d = Dialog::new({}, {});", rect(f.x, f.y, f.x + f.w, f.y + f.h), rust_str(&f.title));
    if f.centered {
        s.push_str("    d.win.group.vs.options |= of::CENTERED;\n");
    }
    match f.palette {
        Pal::Gray => {}
        Pal::Blue => s.push_str("    d.set_palette(DialogPalette::Blue);\n"),
        Pal::Cyan => s.push_str("    d.set_palette(DialogPalette::Cyan);\n"),
    }
    if f.help_ctx != 0 {
        let _ = writeln!(s, "    d.win.group.vs.help_ctx = {};", f.help_ctx);
    }
    for i in build_order(f) {
        let c = &f.controls[i];
        let _ = writeln!(s, "    // {} «{}»", c.kind.title(), c.name);
        for l in control_code(app, c) {
            for part in l.split('\n') {
                let _ = writeln!(s, "    {}", part);
            }
        }
    }
    let names: Vec<&str> = f.controls.iter().map(|c| c.name.as_str()).collect();
    let _ = writeln!(s, "    (d, {}Ids {{ {} }})\n}}\n", ty, names.join(", "));

    // --- run ---
    s.push_str("/// Показать форму модально. `data` заносятся в диалог, а если диалог\n");
    s.push_str("/// закрыт не «Отменой», читаются обратно. Возвращает команду закрытия.\n");
    let _ = writeln!(s, "pub fn run(app: &mut Program, data: &mut {}Data) -> u16 {{", ty);
    s.push_str("    let (mut d, _ids) = build();\n    d.set_data(&data.to_value());\n    let (command, d) = app.exec_dialog(d);\n");
    s.push_str("    if command != cm::CANCEL {\n");
    let _ = writeln!(s, "        *data = {}Data::from_value(&d.get_data());", ty);
    s.push_str("    }\n    command\n}\n");
    s
}

// ======================================================================
// Запись файлов
// ======================================================================

/// Проверка модели перед генерацией: ошибки, которые помешают сборке.
pub fn check_project(p: &Project, forms: &[Form]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for f in forms {
        if !seen.insert(f.name.clone()) {
            out.push(format!("форма «{}» повторяется", f.name));
        }
        for w in f.check() {
            out.push(format!("форма {}: {}", f.name, w));
        }
        for c in &f.controls {
            if c.kind == Kind::Button && p.app.command_value(&c.command).is_none() {
                out.push(format!("форма {}: у кнопки «{}» неизвестная команда «{}»", f.name, c.name, c.command));
            }
        }
    }
    let mut names = std::collections::HashSet::new();
    for c in &p.app.commands {
        if !crate::model::is_ident(&c.name) {
            out.push(format!("команда «{}»: имя не годится как идентификатор Rust", c.name));
        }
        if !names.insert(c.name.clone()) {
            out.push(format!("команда «{}» повторяется", c.name));
        }
        if let Action::Form(fname) = &c.action {
            if !forms.iter().any(|f| &f.name == fname) {
                out.push(format!("команда {}: нет формы «{}»", c.name, fname));
            }
        }
    }
    fn walk(app: &AppModel, items: &[MenuNode], out: &mut Vec<String>) {
        for it in items {
            match it {
                MenuNode::Item { title, command, key, .. } => {
                    if app.command_value(command).is_none() {
                        out.push(format!("меню «{}»: неизвестная команда «{}»", title, command));
                    }
                    if !key.trim().is_empty() && parse_key(key).is_none() {
                        out.push(format!("меню «{}»: непонятная клавиша «{}»", title, key));
                    }
                }
                MenuNode::Sub { title, key, children, .. } => {
                    if !key.trim().is_empty() && parse_key(key).is_none() {
                        out.push(format!("подменю «{}»: непонятная клавиша «{}»", title, key));
                    }
                    walk(app, children, out)
                }
                MenuNode::Separator => {}
            }
        }
    }
    walk(&p.app, &p.app.menu, &mut out);
    for e in &p.app.status {
        if p.app.command_value(&e.command).is_none() {
            out.push(format!("строка состояния «{}»: неизвестная команда «{}»", e.text, e.command));
        }
        if !e.key.trim().is_empty() && parse_key(&e.key).is_none() {
            out.push(format!("строка состояния «{}»: непонятная клавиша «{}»", e.text, e.key));
        }
    }
    out
}

/// Записать файл, только если содержимое изменилось (чтобы cargo не
/// пересобирал лишнее). Возвращает `true`, если файл записан.
fn write_if_changed(path: &PathBuf, text: &str) -> Result<bool, String> {
    if std::fs::read_to_string(path).map(|old| old == text).unwrap_or(false) {
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d.display(), e))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(true)
}

/// Сгенерировать все файлы проекта. Возвращает изменённые файлы.
pub fn generate_all(p: &Project, forms: &[Form]) -> Result<Vec<PathBuf>, String> {
    let mut changed = Vec::new();
    let src = p.dir.join("src");
    let main = src.join("main.rs");
    if !main.exists() && write_if_changed(&main, &main_rs(p))? {
        changed.push(main);
    }
    let ui = src.join("ui.rs");
    if write_if_changed(&ui, &ui_rs(p, forms))? {
        changed.push(ui);
    }
    let fm = src.join("forms").join("mod.rs");
    if write_if_changed(&fm, &forms_mod_rs(forms))? {
        changed.push(fm);
    }
    for f in forms {
        let path = p.form_code_path(&f.name);
        if write_if_changed(&path, &form_rs(p, f))? {
            changed.push(path);
        }
    }
    // Модули удалённых форм убираем (только сгенерированные нами).
    if let Ok(rd) = std::fs::read_dir(src.join("forms")) {
        for e in rd.flatten() {
            let path = e.path();
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if path.extension().map(|x| x == "rs").unwrap_or(false)
                && stem != "mod"
                && !forms.iter().any(|f| f.name == stem)
                && std::fs::read_to_string(&path).map(|t| t.starts_with(GENERATED_MARK)).unwrap_or(false)
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{about_form, Dependency};

    #[test]
    fn generates_form_code() {
        let dir = std::env::temp_dir().join(format!("tvs_cg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tv = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tvision-rs");
        let p = Project::create(&dir, "demo_app", &Dependency::Path(tv.display().to_string())).unwrap();
        let mut f = about_form("x");
        let mut i = f.new_control(Kind::InputLine, 3, 6);
        i.validator = "range:1..9".into();
        i.history = 3;
        f.controls.push(i);
        let code = form_rs(&p, &f);
        assert!(code.contains("pub struct AboutData"));
        assert!(code.contains("RangeValidator::new(1, 9)"));
        assert!(code.contains("History::new("));
        let ui = ui_rs(&p, &[f.clone()]);
        assert!(ui.contains("pub const ABOUT: u16 = 100;"));
        assert!(ui.contains("crate::forms::about::run"));
        assert!(check_project(&p, &[f]).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
