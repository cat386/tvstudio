//! Диалоги среды: свойства элемента, новый проект, новая форма,
//! параметры среды и программы, недавние проекты.

use std::path::PathBuf;

use tvision::*;

use crate::cmds::*;
use crate::json::{self, Json};
use crate::model::{is_crate_name, is_ident, AppModel, Dependency, Form, FormClass, Kind, ProjectKind};
use crate::props::{self, Prop};

// ======================================================================
// Окна без тени
// ======================================================================
//
// Окна tvision-rs рождаются с тенью (`sf::SHADOW`); в среде теней нет ни
// у одного окна. Всё, что среда показывает, идёт через эти функции:
// `exec` (модальные диалоги), `insert` (окна на рабочем столе) и свои
// `message_box` / `input_box` вместо библиотечных (те создают диалог внутри).

/// Окно без тени.
pub fn flat<T: View>(mut v: T) -> T {
    v.vs_mut().state &= !sf::SHADOW;
    v
}

/// Модальный диалог без тени (вместо `app.exec_dialog`).
pub fn exec<T: View>(app: &mut Program, v: T) -> (u16, T) {
    app.exec_dialog(flat(v))
}

/// Окно на рабочий стол без тени (вместо `app.insert_window`).
pub fn insert<T: View>(app: &mut Program, v: T) -> ViewId {
    app.insert_window(flat(v))
}

/// Диалог сообщения без тени (для `ctx.exec` из представлений).
pub fn build_message_box(r: Rect, msg: &str, options: u16) -> Dialog {
    flat(tvision::build_message_box(r, msg, options))
}

/// Окно сообщения 40×9 в центре экрана, без тени.
pub fn message_box(app: &mut Program, msg: &str, options: u16) -> u16 {
    message_box_rect(app, Rect::new(0, 0, 40, 9), msg, options, true)
}

/// Окно сообщения заданного размера, без тени.
pub fn message_box_rect(app: &mut Program, r: Rect, msg: &str, options: u16, centered: bool) -> u16 {
    let mut d = build_message_box(r, msg, options);
    if !centered {
        d.win.group.vs.options &= !of::CENTERED;
    }
    exec(app, d).0
}

/// Выбор из списка (Enter или двойной щелчок — выбрать, Esc — отмена).
/// `Some(номер)` выбранного пункта.
pub fn choose(app: &mut Program, title: &str, items: &[String]) -> Option<usize> {
    let w = (items.iter().map(|s| tvision::text::cstr_width(s)).max().unwrap_or(10) as i32 + 6).clamp(36, 76);
    let h = (items.len() as i32).clamp(3, 14) + 6;
    let mut d = Dialog::centered(w, h, title);
    let mut list = ListBox::new(Rect::new(2, 2, w - 2, h - 4), 1, None);
    list.set_items(items.to_vec());
    let list_id = d.add(list);
    d.add(Button::new(Rect::new(w - 26, h - 3, w - 14, h - 1), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(w - 13, h - 3, w - 2, h - 1), "Отмена", cm::CANCEL, bf::NORMAL));
    d.select(list_id);
    let (cmd, d) = exec(app, d);
    if cmd != cm::OK {
        return None;
    }
    d.win.group.find::<ListBox>(list_id).map(|l| l.focused).filter(|&i| i < items.len())
}

/// Окно ввода строки без тени (как `tvision::input_box`). `Some(текст)` — при OK.
pub fn input_box(app: &mut Program, title: &str, label: &str, initial: &str, limit: usize) -> Option<String> {
    let label_w = tvision::text::cstr_width(label) as i32;
    let w = (label_w + 30).clamp(30, 70);
    let mut d = Dialog::centered(w, 8, title);
    let il = d.add(InputLine::new(Rect::new(4 + label_w, 2, w - 3, 3), limit));
    d.add(Label::new(Rect::new(2, 2, 3 + label_w, 3), label, Some(il)));
    d.add(Button::new(Rect::new(w - 24, 5, w - 14, 7), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(w - 12, 5, w - 2, 7), "Отмена", cm::CANCEL, bf::NORMAL));
    d.select(il);
    if let Some(v) = d.win.group.find_mut::<InputLine>(il) {
        v.set_text(initial);
    }
    let (cmd, d) = exec(app, d);
    if cmd != cm::OK {
        return None;
    }
    d.win.group.find::<InputLine>(il).map(|v| v.text())
}

/// Строка «метка + поле ввода» (+ кнопка списка). Возвращает id поля.
pub fn field(d: &mut Dialog, x: i32, y: i32, label: &str, text: &str, w: i32, combo: Option<&[&str]>) -> ViewId {
    let iw = if combo.is_some() { w - 3 } else { w };
    let mut il = InputLine::new(Rect::new(x, y, x + iw, y + 1), 512);
    il.set_text(text);
    let id = d.add(il);
    if let Some(items) = combo {
        d.add(Combo::new(Rect::new(x + iw, y, x + w, y + 1), id, cbx::NO_TRANSFER, items));
    }
    d.add(Label::new(Rect::new(1, y, x, y + 1), label, Some(id)));
    id
}

/// Кнопки OK и «Отмена» по центру строки `y`.
pub fn ok_cancel(d: &mut Dialog, y: i32) {
    let w = d.win.group.vs.size.x;
    d.add(Button::new(Rect::new(w / 2 - 13, y, w / 2 - 2, y + 2), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(w / 2 + 1, y, w / 2 + 12, y + 2), "Отмена", cm::CANCEL, bf::NORMAL));
}

fn strs(v: &Value) -> Vec<Value> {
    v.as_list().map(|l| l.to_vec()).unwrap_or_default()
}

fn s_at(v: &[Value], i: usize) -> String {
    v.get(i).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

// ======================================================================
// Свойства элемента
// ======================================================================

/// Многострочное ли свойство для данного вида элемента.
fn is_multi(p: Prop, kind: Option<Kind>) -> bool {
    match p {
        Prop::Items => true,
        Prop::Text => matches!(kind, Some(Kind::StaticText) | Some(Kind::Memo) | Some(Kind::Table)),
        _ => false,
    }
}

/// Диалог всех свойств выделенного элемента (или формы).
pub fn props_dialog(form: &Form, sel: Option<usize>) -> (Dialog, Vec<Prop>) {
    let kind = sel.and_then(|i| form.controls.get(i)).map(|c| c.kind);
    let list = props::props_for(form, sel);
    let lines: i32 = list.iter().map(|p| if is_multi(*p, kind) { 4 } else { 1 }).sum();
    let h = (lines + 6).min(23);
    let title = match sel.and_then(|i| form.controls.get(i)) {
        Some(c) => format!("Свойства: {} ({})", c.name, c.kind.title()),
        None => format!("Свойства формы {}", form.name),
    };
    let mut d = Dialog::centered(66, h, &title);
    d.win.group.vs.help_ctx = HC_INSPECTOR;
    let mut y = 1;
    let x = 16;
    let mut first = None;
    for p in &list {
        if is_multi(*p, kind) {
            let sb = d.add(ScrollBar::new(Rect::new(x + 47, y, x + 48, y + 4)));
            let mut m = Memo::new(Rect::new(x, y, x + 47, y + 4), None, Some(sb));
            m.editor.set_text(&props::get_full(form, sel, *p));
            m.editor.modified = false;
            let id = d.add(m);
            d.add(Label::new(Rect::new(1, y, x, y + 1), p.label(kind), Some(id)));
            first.get_or_insert(id);
            y += 4;
        } else {
            let id = field(&mut d, x, y, p.label(kind), &props::get(form, sel, *p), 48, None);
            first.get_or_insert(id);
            y += 1;
        }
    }
    let hint = if kind.is_some() {
        "Tab — следующее поле. Пункты списка — по одному в строке."
    } else {
        "Класс: Dialog / Window. Цвет: серый, синий, бирюзовый. Да / нет."
    };
    d.add(StaticText::new(Rect::new(2, y + 1, 64, y + 2), hint));
    ok_cancel(&mut d, y + 2);
    if let Some(f) = first {
        d.select(f);
    }
    (d, list)
}

/// Применить данные диалога свойств. При ошибке форма не меняется.
pub fn apply_props(form: &mut Form, sel: Option<usize>, list: &[Prop], data: &Value) -> Result<(), String> {
    let kind = sel.and_then(|i| form.controls.get(i)).map(|c| c.kind);
    let vals = strs(data);
    let mut f = form.clone();
    for (i, p) in list.iter().enumerate() {
        let v = s_at(&vals, i);
        if is_multi(*p, kind) {
            if v != props::get_full(&f, sel, *p) {
                props::set_full(&mut f, sel, *p, &v).map_err(|e| format!("{}: {}", p.label(kind), e))?;
            }
        } else if v != props::get(&f, sel, *p) {
            props::set(&mut f, sel, *p, &v).map_err(|e| format!("{}: {}", p.label(kind), e))?;
        }
    }
    *form = f;
    Ok(())
}

// ======================================================================
// Новый проект
// ======================================================================

/// Параметры нового проекта.
#[derive(Debug)]
pub struct NewProject {
    pub kind: ProjectKind,
    pub name: String,
    pub parent: PathBuf,
    /// Откуда брать tvision-rs; у программы на Rust — `None`.
    pub dep: Option<Dependency>,
}

/// Диалог «Новый проект». `ask_kind` — спросить тип проекта (tvision-rs или
/// просто Rust); без него (импорт) проект всегда на tvision-rs.
pub fn new_project_dialog(cfg: &Config, name: &str, title: &str, ask_kind: bool) -> Dialog {
    let dy = if ask_kind { 3 } else { 0 };
    let mut d = Dialog::centered(72, 16 + dy, title);
    let kind = ask_kind.then(|| {
        let titles: Vec<&str> = ProjectKind::ALL.iter().map(|k| k.title()).collect();
        let k = d.add(RadioButtons::radio_buttons(Rect::new(16, 2, 70, 4), &titles));
        d.add(Label::new(Rect::new(2, 2, 16, 3), "~Т~ип", Some(k)));
        k
    });
    let first = field(&mut d, 16, 2 + dy, "~И~мя", name, 30, None);
    field(&mut d, 16, 4 + dy, "~К~аталог", &cfg.default_parent().display().to_string(), 54, None);
    field(&mut d, 16, 6 + dy, "~t~vision-rs", &cfg.dep_value, 54, None);
    d.add(StaticText::new(Rect::new(2, 7 + dy, 70, 8 + dy), "  ссылка git (https://…) или путь к каталогу tvision-rs на диске"));
    let about = if ask_kind {
        "Будет создан каталог «Каталог/Имя»: Cargo.toml, src/main.rs, файл\nпроекта .tvsp, а у приложения tvision-rs — и форма «О программе».\nПрограмме на Rust строка tvision-rs не нужна."
    } else {
        "Будет создан каталог «Каталог/Имя» с Cargo.toml, src/main.rs,\nфайлом проекта .tvsp и формой «О программе»."
    };
    d.add(StaticText::new(Rect::new(2, 9 + dy, 70, 12 + dy), about));
    ok_cancel(&mut d, 13 + dy);
    d.select(kind.unwrap_or(first));
    d
}

/// Прочитать диалог «Новый проект» (`ask_kind` — как при его создании).
pub fn read_new_project(v: &Value, ask_kind: bool) -> Result<NewProject, String> {
    let mut v = strs(v);
    let kind = if ask_kind {
        let k = v.first().and_then(|x| x.as_u32()).unwrap_or(0) as usize;
        v.remove(0);
        ProjectKind::ALL.get(k).copied().unwrap_or_default()
    } else {
        ProjectKind::Tvision
    };
    let name = s_at(&v, 0).trim().to_string();
    if !is_crate_name(&name) {
        return Err(format!("«{}» не годится как имя пакета: латинские буквы, цифры, _ и -.", name));
    }
    let parent = PathBuf::from(s_at(&v, 1).trim());
    let dep = match kind {
        ProjectKind::Rust => None,
        ProjectKind::Tvision => Some(Dependency::detect(&s_at(&v, 2)).ok_or_else(|| "Укажите, откуда брать tvision-rs.".to_string())?),
    };
    Ok(NewProject { kind, name, parent, dep })
}

// ======================================================================
// Новая форма
// ======================================================================

pub fn new_form_dialog(existing: &[String]) -> Dialog {
    let name = (1..).map(|n| format!("form{}", n)).find(|n| !existing.contains(n)).unwrap();
    let mut d = Dialog::centered(58, 17, "Новая форма");
    let first = field(&mut d, 16, 2, "~И~мя", &name, 30, None);
    field(&mut d, 16, 3, "~З~аголовок", "Диалог", 36, None);
    field(&mut d, 16, 5, "~Ш~ирина", "50", 6, None);
    field(&mut d, 16, 6, "~В~ысота", "14", 6, None);
    let titles: Vec<&str> = FormClass::ALL.iter().map(|c| c.title()).collect();
    let class = d.add(RadioButtons::radio_buttons(Rect::new(16, 8, 52, 10), &titles));
    d.add(Label::new(Rect::new(2, 8, 16, 9), "~К~ласс", Some(class)));
    d.add(StaticText::new(
        Rect::new(2, 11, 56, 13),
        "Имя — модуль forms::имя (латиница, цифры, _).\nDialog — run() ждёт OK/Отмены; Window — open() и дальше.",
    ));
    ok_cancel(&mut d, 14);
    d.select(first);
    d
}

pub fn read_new_form(v: &Value, existing: &[String]) -> Result<Form, String> {
    let v = strs(v);
    let name = s_at(&v, 0).trim().to_string();
    if !is_ident(&name) {
        return Err(format!("«{}» не годится как имя формы.", name));
    }
    if existing.contains(&name) {
        return Err(format!("Форма «{}» уже есть в проекте.", name));
    }
    let mut f = Form::new(&name, &s_at(&v, 1));
    f.w = s_at(&v, 2).trim().parse::<i32>().unwrap_or(50).clamp(30, 200);
    f.h = s_at(&v, 3).trim().parse::<i32>().unwrap_or(14).clamp(6, 80);
    f.class = FormClass::ALL.get(v.get(4).and_then(|x| x.as_u32()).unwrap_or(0) as usize).copied().unwrap_or_default();
    if f.class == FormClass::Window {
        // Немодальное окно закрывается своей рамкой или кнопкой «Закрыть» (cm::CLOSE).
        let mut close = f.new_control(Kind::Button, f.w / 2 - 6, f.h - 3);
        close.name = "close".into();
        close.text = "~З~акрыть".into();
        close.command = "cm::CLOSE".into();
        f.controls.push(close);
        return Ok(f);
    }
    // Кнопки OK и «Отмена» — у нижнего края.
    let mut ok = f.new_control(Kind::Button, f.w / 2 - 13, f.h - 3);
    ok.name = "ok".into();
    ok.default = true;
    let mut cancel = f.new_control(Kind::Button, f.w / 2 + 1, f.h - 3);
    cancel.name = "cancel".into();
    cancel.text = "Отмена".into();
    cancel.command = "cm::CANCEL".into();
    f.controls.push(ok);
    f.controls.push(cancel);
    Ok(f)
}

// ======================================================================
// Параметры программы (модели)
// ======================================================================

pub fn app_options_dialog(app: &AppModel, forms: &[String]) -> Dialog {
    let mut d = Dialog::centered(60, 12, "Параметры программы");
    d.win.group.vs.help_ctx = HC_APP_MODEL;
    let first = field(&mut d, 18, 2, "~З~аголовок", &app.title, 38, None);
    let mut cb = CheckBoxes::check_boxes(Rect::new(18, 4, 56, 5), &["~Р~усские надписи tvision-rs"]);
    cb.value = u32::from(app.russian);
    let cb = d.add(cb);
    d.add(Label::new(Rect::new(1, 4, 18, 5), "~Я~зык", Some(cb)));
    let items: Vec<&str> = forms.iter().map(|s| s.as_str()).collect();
    field(&mut d, 18, 6, "~С~тартовая форма", &app.startup_form, 30, Some(&items));
    ok_cancel(&mut d, 9);
    d.select(first);
    d
}

pub fn read_app_options(app: &mut AppModel, v: &Value, forms: &[String]) -> Result<(), String> {
    let v = strs(v);
    let start = s_at(&v, 2).trim().to_string();
    if !start.is_empty() && !forms.contains(&start) {
        return Err(format!("В проекте нет формы «{}».", start));
    }
    app.title = s_at(&v, 0);
    app.russian = v.get(1).and_then(|x| x.as_u32()).unwrap_or(0) & 1 != 0;
    app.startup_form = start;
    Ok(())
}

// ======================================================================
// Параметры среды
// ======================================================================

/// Сервер LSP для Cargo.toml по умолчанию: crates-lsp (версии и features
/// зависимостей). Установка: `cargo install --locked crates-lsp`.
pub const TOML_LSP_DEFAULT: &str = "crates-lsp";

/// Настройки среды (файл `tvstudio.json` в каталоге настроек пользователя).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Тема: 0 — gruvbox, 1 — классическая, 2 — чёрно-белая.
    pub theme: u32,
    /// Сохранять все файлы перед сборкой.
    pub autosave: bool,
    /// Программа cargo.
    pub cargo: String,
    /// tvision-rs для новых проектов: git (иначе путь).
    pub dep_git: bool,
    pub dep_value: String,
    /// Недавние проекты (файлы .tvsp).
    pub recent: Vec<String>,
    /// Сервер LSP для редактора кода (дополнение, переходы, диагностика).
    pub lsp_enabled: bool,
    /// Команда запуска сервера (по умолчанию — `rust-analyzer`).
    pub lsp_command: String,
    /// Сервер для файлов `.toml` (Cargo.toml): по умолчанию
    /// `crates-lsp`; пустая строка — не запускать.
    pub toml_lsp_command: String,
    /// Редактор кода: подсветка строки и столбца курсора.
    pub hl_line: bool,
    pub hl_col: bool,
    /// Закрывать скобки и кавычки в редакторе кода.
    pub auto_close: bool,
    /// Список дополнения сам при наборе имени.
    pub auto_complete: bool,
    /// Форматировать файлы .rs программой rustfmt при сохранении.
    pub fmt_on_save: bool,
    /// Команда rustfmt.
    pub rustfmt: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: 0,
            autosave: true,
            cargo: "cargo".into(),
            dep_git: true,
            dep_value: "https://github.com/cat386/tvision-rs".into(),
            recent: Vec::new(),
            lsp_enabled: true,
            lsp_command: "rust-analyzer".into(),
            toml_lsp_command: TOML_LSP_DEFAULT.into(),
            hl_line: true,
            hl_col: true,
            auto_close: true,
            auto_complete: true,
            fmt_on_save: true,
            rustfmt: "rustfmt".into(),
        }
    }
}

impl Config {
    /// Путь к файлу настроек.
    pub fn path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("TVSTUDIO_CONFIG") {
            return Some(PathBuf::from(p));
        }
        #[cfg(windows)]
        let base = std::env::var("APPDATA").ok().map(PathBuf::from);
        #[cfg(not(windows))]
        let base =
            std::env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from).or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")));
        base.map(|b| b.join("tvstudio.json"))
    }

    pub fn load() -> Config {
        let Some(p) = Self::path() else { return Config::default() };
        let Ok(s) = std::fs::read_to_string(p) else { return Config::default() };
        let Ok(j) = json::parse(&s) else { return Config::default() };
        Self::from_json(&j)
    }

    /// Настройки из разобранного файла (недостающее — по умолчанию).
    pub fn from_json(j: &Json) -> Config {
        let mut c = Config::default();
        c.theme = j.int("theme", 0).clamp(0, 2) as u32;
        c.autosave = j.bool("autosave", true);
        c.cargo = Some(j.str("cargo")).filter(|s| !s.is_empty()).unwrap_or(c.cargo);
        c.dep_git = j.bool("dep_git", true);
        c.dep_value = Some(j.str("dep_value")).filter(|s| !s.is_empty()).unwrap_or(c.dep_value);
        c.recent = j.strings("recent");
        c.lsp_enabled = j.bool("lsp_enabled", true);
        c.hl_line = j.bool("hl_line", true);
        c.hl_col = j.bool("hl_col", true);
        c.auto_close = j.bool("auto_close", true);
        c.auto_complete = j.bool("auto_complete", true);
        c.fmt_on_save = j.bool("fmt_on_save", true);
        c.rustfmt = Some(j.str("rustfmt")).filter(|s| !s.trim().is_empty()).unwrap_or(c.rustfmt);
        c.lsp_command = Some(j.str("lsp_command")).filter(|s| !s.trim().is_empty()).unwrap_or(c.lsp_command);
        // Пустая строка — сервер Cargo.toml выключен; нет ключа — по умолчанию.
        // taplo (прежний сервер) больше не поддерживается — заменяется на crates-lsp.
        if let Some(Json::Str(t)) = j.get("toml_lsp_command") {
            let t = t.trim();
            c.toml_lsp_command = if t.starts_with("taplo") { TOML_LSP_DEFAULT.into() } else { t.to_string() };
        }
        c
    }

    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let j = Json::obj(vec![
            ("theme", Json::i(self.theme as i64)),
            ("autosave", Json::Bool(self.autosave)),
            ("cargo", Json::s(&self.cargo)),
            ("dep_git", Json::Bool(self.dep_git)),
            ("dep_value", Json::s(&self.dep_value)),
            ("recent", Json::from_strings(&self.recent)),
            ("lsp_enabled", Json::Bool(self.lsp_enabled)),
            ("lsp_command", Json::s(&self.lsp_command)),
            ("toml_lsp_command", Json::s(&self.toml_lsp_command)),
            ("hl_line", Json::Bool(self.hl_line)),
            ("hl_col", Json::Bool(self.hl_col)),
            ("auto_close", Json::Bool(self.auto_close)),
            ("auto_complete", Json::Bool(self.auto_complete)),
            ("fmt_on_save", Json::Bool(self.fmt_on_save)),
            ("rustfmt", Json::s(&self.rustfmt)),
        ]);
        let _ = std::fs::write(p, j.to_pretty());
    }

    /// Каталог для нового проекта: рядом с последним проектом, иначе —
    /// текущий каталог (но не каталог сборки самой среды `target/…`).
    pub fn default_parent(&self) -> PathBuf {
        if let Some(p) = self.recent.first().map(PathBuf::from).and_then(|f| f.parent()?.parent().map(|d| d.to_path_buf())) {
            if p.is_dir() {
                return p;
            }
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        // Запуск из target/release: поднимаемся выше каталога target.
        let mut c = cwd.as_path();
        while let Some(parent) = c.parent() {
            if c.file_name().map(|n| n == "target").unwrap_or(false) {
                return parent.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| parent.to_path_buf());
            }
            c = parent;
        }
        cwd
    }

    /// Запомнить недавний проект.
    pub fn add_recent(&mut self, file: &str) {
        self.recent.retain(|r| r != file);
        self.recent.insert(0, file.to_string());
        self.recent.truncate(10);
    }

    /// Палитра приложения по теме.
    pub fn palette(&self) -> Vec<Attr> {
        use tvision::color::palettes;
        match self.theme {
            1 => palettes::APP_COLOR.to_vec(),
            2 => palettes::APP_BLACK_WHITE.to_vec(),
            _ => {
                if tvision::color::color_mode() == tvision::color::ColorMode::Ansi16 {
                    palettes::APP_COLOR.to_vec()
                } else {
                    palettes::GRUVBOX.to_vec()
                }
            }
        }
    }
}

pub fn options_dialog(cfg: &Config) -> Dialog {
    let mut d = Dialog::centered(66, 28, "Параметры среды");
    let mut th = RadioButtons::radio_buttons(Rect::new(18, 2, 50, 5), &["~g~ruvbox (256 цветов)", "~к~лассическая", "~ч~ёрно-белая"]);
    th.value = cfg.theme;
    let th = d.add(th);
    d.add(Label::new(Rect::new(1, 2, 18, 3), "~Т~ема", Some(th)));
    let mut cb = CheckBoxes::check_boxes(Rect::new(18, 6, 62, 8), &["~С~охранять всё перед сборкой", "Сервер ~L~SP в редакторе кода"]);
    cb.value = u32::from(cfg.autosave) | (u32::from(cfg.lsp_enabled) << 1);
    let cb = d.add(cb);
    d.add(Label::new(Rect::new(1, 6, 18, 7), "Сб~о~рка", Some(cb)));
    field(&mut d, 18, 9, "~c~argo", &cfg.cargo, 44, None);
    field(&mut d, 18, 11, "t~v~ision-rs", &cfg.dep_value, 44, None);
    d.add(StaticText::new(Rect::new(18, 12, 64, 13), "ссылка git или путь для новых проектов"));
    field(&mut d, 18, 14, "Сервер L~S~P", &cfg.lsp_command, 44, None);
    d.add(StaticText::new(Rect::new(18, 15, 64, 16), "rustup component add rust-analyzer"));
    field(&mut d, 18, 16, "Сервер Cargo.to~m~l", &cfg.toml_lsp_command, 44, None);
    d.add(StaticText::new(Rect::new(18, 17, 64, 18), "cargo install --locked crates-lsp"));
    let mut hl = CheckBoxes::check_boxes(
        Rect::new(18, 19, 62, 24),
        &[
            "Подсвечивать ~с~троку курсора",
            "Подсвечивать с~т~олбец курсора",
            "~r~ustfmt при сохранении (.rs)",
            "~З~акрывать скобки и кавычки",
            "~П~одсказки при наборе",
        ],
    );
    hl.value = u32::from(cfg.hl_line)
        | (u32::from(cfg.hl_col) << 1)
        | (u32::from(cfg.fmt_on_save) << 2)
        | (u32::from(cfg.auto_close) << 3)
        | (u32::from(cfg.auto_complete) << 4);
    let hl = d.add(hl);
    d.add(Label::new(Rect::new(1, 19, 18, 20), "~Р~едактор", Some(hl)));
    ok_cancel(&mut d, 25);
    d.select(th);
    d
}

pub fn read_options(cfg: &mut Config, v: &Value) {
    let v = strs(v);
    cfg.theme = v.first().and_then(|x| x.as_u32()).unwrap_or(0).min(2);
    let flags = v.get(1).and_then(|x| x.as_u32()).unwrap_or(3);
    cfg.autosave = flags & 1 != 0;
    cfg.lsp_enabled = flags & 2 != 0;
    let cargo = s_at(&v, 2);
    if !cargo.trim().is_empty() {
        cfg.cargo = cargo.trim().to_string();
    }
    let dv = s_at(&v, 3);
    if let Some(dep) = Dependency::detect(&dv) {
        cfg.dep_git = matches!(dep, Dependency::Git(_));
        cfg.dep_value = dv.trim().to_string();
    }
    let lsp = s_at(&v, 4);
    if !lsp.trim().is_empty() {
        cfg.lsp_command = lsp.trim().to_string();
    }
    // Сервер Cargo.toml (crates-lsp): пустое поле — выключен.
    cfg.toml_lsp_command = s_at(&v, 5).trim().to_string();
    let hl = v.get(6).and_then(|x| x.as_u32()).unwrap_or(31);
    cfg.hl_line = hl & 1 != 0;
    cfg.hl_col = hl & 2 != 0;
    cfg.fmt_on_save = hl & 4 != 0;
    cfg.auto_close = hl & 8 != 0;
    cfg.auto_complete = hl & 16 != 0;
}

// ======================================================================
// Недавние проекты
// ======================================================================

pub fn recent_dialog(recent: &[String]) -> (Dialog, ViewId) {
    let mut d = Dialog::centered(70, 16, "Недавние проекты");
    let sb = d.add(ScrollBar::new(Rect::new(66, 2, 67, 12)));
    let mut l = ListBox::new(Rect::new(2, 2, 66, 12), 1, Some(sb));
    l.set_items(recent.to_vec());
    let id = d.add(l);
    ok_cancel(&mut d, 13);
    d.select(id);
    (d, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::about_form;

    #[test]
    fn props_dialog_roundtrip() {
        let mut f = about_form("x");
        let (d, list) = props_dialog(&f, Some(1));
        let mut data = strs(&d.get_data());
        assert_eq!(data.len(), list.len());
        // Меняем надпись кнопки (свойство Text = индекс 5).
        let ti = list.iter().position(|p| *p == Prop::Text).unwrap();
        data[ti] = Value::Str("~З~акрыть".into());
        apply_props(&mut f, Some(1), &list, &Value::List(data.clone())).unwrap();
        assert_eq!(f.controls[1].text, "~З~акрыть");
        // Ошибка — форма не меняется.
        let wi = list.iter().position(|p| *p == Prop::W).unwrap();
        data[wi] = Value::Str("широко".into());
        let before = f.clone();
        assert!(apply_props(&mut f, Some(1), &list, &Value::List(data)).is_err());
        assert_eq!(f, before);
    }

    #[test]
    fn new_form_has_buttons() {
        let f = read_new_form(
            &Value::List(vec![Value::Str("edit_user".into()), Value::Str("Пользователь".into()), Value::Str("40".into()), Value::Str("10".into())]),
            &[],
        )
        .unwrap();
        assert_eq!(f.controls.len(), 2);
        assert!(f.check().is_empty(), "{:?}", f.check());
    }

    #[test]
    fn new_form_as_window() {
        let v = |c: u32| {
            Value::List(vec![
                Value::Str("notes".into()),
                Value::Str("Заметки".into()),
                Value::Str("40".into()),
                Value::Str("10".into()),
                Value::U32(c),
            ])
        };
        let f = read_new_form(&v(1), &[]).unwrap();
        assert_eq!(f.class, FormClass::Window);
        assert_eq!(f.controls.len(), 1);
        assert_eq!(f.controls[0].command, "cm::CLOSE");
        assert!(f.check().is_empty(), "{:?}", f.check());
        assert_eq!(read_new_form(&v(0), &[]).unwrap().class, FormClass::Dialog);
        // Класс сохраняется в .tvf и читается обратно.
        let back = Form::from_json(&f.to_json()).unwrap();
        assert_eq!(back, f);
        // Свойства формы: у окна есть номер и растяжение.
        let list = crate::props::props_for(&f, None);
        assert!(list.contains(&Prop::FormNumber) && list.contains(&Prop::FormResizable));
        let mut g = f.clone();
        crate::props::set(&mut g, None, Prop::FormClass, "диалог").unwrap();
        assert_eq!(g.class, FormClass::Dialog);
        assert!(!crate::props::props_for(&g, None).contains(&Prop::FormNumber));
        assert!(crate::props::set(&mut g, None, Prop::FormClass, "кнопка").is_err());
    }

    /// Сервер для Cargo.toml: по умолчанию crates-lsp; taplo (прежний
    /// сервер) заменяется на crates-lsp, своё значение и «выключен» — остаются.
    #[test]
    fn toml_server_setting_migrates() {
        let load = |v: Option<&str>| {
            let mut pairs = vec![("theme", Json::Num(1.0))];
            if let Some(v) = v {
                pairs.push(("toml_lsp_command", Json::s(v)));
            }
            Config::from_json(&Json::obj(pairs)).toml_lsp_command
        };
        assert_eq!(load(None), "crates-lsp");
        assert_eq!(load(Some("taplo lsp stdio")), "crates-lsp", "taplo заменён");
        assert_eq!(load(Some("taplo lsp stdio --colors never")), "crates-lsp", "taplo заменён");
        assert_eq!(load(Some("/opt/bin/crates-lsp")), "/opt/bin/crates-lsp", "своё — как есть");
        assert_eq!(load(Some("")), "", "выключен");
    }

    /// Параметры среды: что показал диалог, то и прочиталось (в том числе
    /// сервер Cargo.toml; пустое поле — выключен).
    #[test]
    fn options_dialog_round_trip() {
        for toml in [TOML_LSP_DEFAULT, "/opt/bin/crates-lsp", ""] {
            let cfg = Config {
                toml_lsp_command: toml.into(),
                lsp_command: "ra".into(),
                hl_col: false,
                auto_close: false,
                auto_complete: false,
                theme: 1,
                ..Config::default()
            };
            let d = options_dialog(&cfg);
            let mut back = Config::default();
            read_options(&mut back, &d.get_data());
            assert_eq!(back, cfg);
        }
    }

    /// «Новый проект»: тип читается первым; программе на Rust tvision-rs
    /// не нужен. Импорт (без вопроса о типе) — всегда tvision-rs.
    #[test]
    fn new_project_dialog_kinds() {
        let cfg = Config { dep_value: String::new(), ..Config::default() };
        let mut d = new_project_dialog(&cfg, "plain", "Новый проект", true);
        let mut v = strs(&d.get_data());
        v[0] = Value::U32(1);
        d.set_data(&Value::List(v));
        let np = read_new_project(&d.get_data(), true).unwrap();
        assert_eq!((np.kind, np.name.as_str(), np.dep.is_none()), (ProjectKind::Rust, "plain", true));
        // Тип по умолчанию — tvision-rs; без строки tvision-rs — ошибка.
        let d = new_project_dialog(&cfg, "app", "Новый проект", true);
        assert!(read_new_project(&d.get_data(), true).unwrap_err().contains("tvision-rs"));
        let d = new_project_dialog(&Config::default(), "app", "Импорт", false);
        let np = read_new_project(&d.get_data(), false).unwrap();
        assert_eq!(np.kind, ProjectKind::Tvision);
        assert!(np.dep.is_some());
    }
}
