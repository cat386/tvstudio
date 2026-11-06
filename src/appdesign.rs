//! Модель приложения: редакторы строки меню, строки состояния и команд.
//!
//! Все три — один диалог [`ListEditor`] со списком и кнопками; правка
//! пункта — во вложенном диалоге. Редактор работает с копией модели
//! (`AppModel`): если в пункте меню указана новая команда, она сразу
//! добавляется в список команд. «Отмена» отбрасывает все изменения.

use tvision::*;

use crate::cmds::*;
use crate::model::{is_ident, parse_key, Action, AppModel, CommandDef, MenuNode, StatusEntry, BUILTIN_COMMANDS};
use crate::toolwins::Relay;

/// Строка «плоского» меню: уровень вложенности + пункт.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuRow {
    pub depth: usize,
    pub sub: bool,
    pub sep: bool,
    pub title: String,
    pub command: String,
    pub key: String,
    pub help: i64,
}

/// Дерево меню → плоский список.
pub fn flatten(items: &[MenuNode], depth: usize, out: &mut Vec<MenuRow>) {
    for it in items {
        match it {
            MenuNode::Item { title, command, key, help } => {
                out.push(MenuRow { depth, sub: false, sep: false, title: title.clone(), command: command.clone(), key: key.clone(), help: *help })
            }
            MenuNode::Separator => {
                out.push(MenuRow { depth, sub: false, sep: true, title: String::new(), command: String::new(), key: String::new(), help: 0 })
            }
            MenuNode::Sub { title, children } => {
                out.push(MenuRow { depth, sub: true, sep: false, title: title.clone(), command: String::new(), key: String::new(), help: 0 });
                flatten(children, depth + 1, out);
            }
        }
    }
}

/// Плоский список → дерево. Лишняя глубина (пункт «внутри» не-подменю)
/// выравнивается до допустимой.
pub fn unflatten(rows: &[MenuRow]) -> Vec<MenuNode> {
    fn build(rows: &[MenuRow], i: &mut usize, depth: usize) -> Vec<MenuNode> {
        let mut out = Vec::new();
        while *i < rows.len() && rows[*i].depth >= depth {
            let r = &rows[*i];
            *i += 1;
            if r.sub {
                let children = build(rows, i, depth + 1);
                out.push(MenuNode::Sub { title: r.title.clone(), children });
            } else if r.sep {
                out.push(MenuNode::Separator);
            } else {
                out.push(MenuNode::Item { title: r.title.clone(), command: r.command.clone(), key: r.key.clone(), help: r.help });
            }
        }
        out
    }
    let mut i = 0;
    let mut v = build(rows, &mut i, 0);
    // Строки, оставшиеся «вне» (глубина меньше нуля невозможна) — на верхний уровень.
    while i < rows.len() {
        let d = rows[i].depth;
        v.extend(build(rows, &mut i, d));
    }
    v
}

/// Что редактируется.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeKind {
    Menu,
    Status,
    Commands,
}

/// Метки вложенных диалогов.
const TAG_EDIT: u16 = 1;
const TAG_INFO: u16 = 2;

/// Отложенная правка: какую строку правим и новая ли она.
#[derive(Clone, Copy, Debug)]
struct Pending {
    index: usize,
    new: bool,
    /// Для меню: новая строка — подменю.
    sub: bool,
}

/// Диалог-редактор списка (меню, строка состояния, команды).
pub struct ListEditor {
    pub dlg: Dialog,
    list_id: ViewId,
    pub kind: LeKind,
    /// Рабочая копия модели.
    pub app: AppModel,
    /// Плоское меню (для `LeKind::Menu`).
    pub rows: Vec<MenuRow>,
    /// Имена форм проекта (для действий команд).
    pub forms: Vec<String>,
    pending: Option<Pending>,
}

fn button(x: i32, y: i32, w: i32, title: &str, cmd: u16) -> Button {
    Button::new(Rect::new(x, y, x + w, y + 2), title, cmd, bf::NORMAL)
}

fn pad(s: &str, w: usize) -> String {
    let n = tvision::text::str_width(s);
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

impl ListEditor {
    pub fn new(kind: LeKind, app: &AppModel, forms: &[String]) -> Self {
        let title = match kind {
            LeKind::Menu => "Строка меню",
            LeKind::Status => "Строка состояния",
            LeKind::Commands => "Команды программы",
        };
        let mut d = Dialog::centered(76, 22, title);
        d.win.group.vs.help_ctx = HC_APP_MODEL;
        let sb = d.add(ScrollBar::new(Rect::new(53, 2, 54, 19)));
        let mut list = ListBox::new(Rect::new(2, 2, 53, 19), 1, Some(sb));
        list.vs.help_ctx = HC_APP_MODEL;
        let list_id = d.add(list);
        d.add(Label::new(Rect::new(2, 1, 30, 2), "~С~писок:", Some(list_id)));
        let x = 56;
        let w = 18;
        let mut y = 2;
        let mut add = |d: &mut Dialog, t: &str, c: u16| {
            d.add(button(x, y, w, t, c));
            y += 2;
        };
        match kind {
            LeKind::Menu => {
                add(&mut d, "~П~ункт", CM_LE_ADD);
                add(&mut d, "Под~м~еню", CM_LE_SUB);
                add(&mut d, "~Р~азделитель", CM_LE_SEP);
            }
            _ => add(&mut d, "~Д~обавить", CM_LE_ADD),
        }
        add(&mut d, "~И~зменить", CM_LE_EDIT);
        add(&mut d, "~У~далить", CM_LE_DEL);
        add(&mut d, "~В~ыше", CM_LE_UP);
        add(&mut d, "~Н~иже", CM_LE_DOWN);
        if kind == LeKind::Menu {
            d.add(button(x, y, 9, "~←~", CM_LE_LEFT));
            d.add(button(x + 9, y, 9, "~→~", CM_LE_RIGHT));
        }
        d.add(Button::new(Rect::new(x, 19, x + 9, 21), "O~K~", cm::OK, bf::DEFAULT));
        d.add(Button::new(Rect::new(x + 9, 19, x + 18, 21), "Отмена", cm::CANCEL, bf::NORMAL));
        d.add(Relay::new(list_id, CM_LE_EDIT));
        d.select(list_id);
        let mut rows = Vec::new();
        flatten(&app.menu, 0, &mut rows);
        let mut e = ListEditor { dlg: d, list_id, kind, app: app.clone(), rows, forms: forms.to_vec(), pending: None };
        e.refresh(None);
        e
    }

    /// Итоговое меню (после OK).
    pub fn menu(&self) -> Vec<MenuNode> {
        unflatten(&self.rows)
    }

    fn len(&self) -> usize {
        match self.kind {
            LeKind::Menu => self.rows.len(),
            LeKind::Status => self.app.status.len(),
            LeKind::Commands => self.app.commands.len(),
        }
    }

    fn display(&self, i: usize) -> String {
        match self.kind {
            LeKind::Menu => {
                let r = &self.rows[i];
                let ind = "  ".repeat(r.depth);
                if r.sep {
                    format!("{}────────────", ind)
                } else if r.sub {
                    format!("{}▸ {}", ind, r.title)
                } else {
                    format!("{} {}{} {}", pad(&format!("{}{}", ind, r.title), 26), pad(&r.command, 14), "", r.key)
                }
            }
            LeKind::Status => {
                let s = &self.app.status[i];
                let t = if s.text.is_empty() { "(скрытый)".to_string() } else { s.text.clone() };
                format!("{} {} {}", pad(&t, 22), pad(&s.key, 10), s.command)
            }
            LeKind::Commands => {
                let c = &self.app.commands[i];
                format!("{} {} {}", pad(&c.name, 16), pad(&c.value.to_string(), 5), c.action.describe())
            }
        }
    }

    fn list(&mut self) -> &mut ListBox {
        let id = self.list_id;
        self.dlg.win.group.find_mut::<ListBox>(id).unwrap()
    }

    fn cursor(&self) -> usize {
        self.dlg.win.group.find::<ListBox>(self.list_id).map(|l| l.focused).unwrap_or(0)
    }

    /// Обновить список; `focus` — какую строку выделить.
    fn refresh(&mut self, focus: Option<usize>) {
        let items: Vec<String> = (0..self.len()).map(|i| self.display(i)).collect();
        let keep = focus.unwrap_or_else(|| self.cursor());
        let l = self.list();
        l.set_items(items);
        let n = l.items.len();
        if n > 0 {
            l.focus_item(keep.min(n - 1));
        }
    }

    fn info(&mut self, msg: &str, ctx: &mut Ctx) {
        let d = build_message_box(Rect::new(0, 0, 54, 10), msg, mf::ERROR | mf::OK_BUTTON);
        ctx.exec(Box::new(d), Some(self.dlg.id()), TAG_INFO);
    }

    /// Диалог правки строки `p`.
    fn edit_dialog(&self, p: Pending) -> Dialog {
        let cmds: Vec<String> = self.app.all_command_names();
        let cmd_refs: Vec<&str> = cmds.iter().map(|s| s.as_str()).collect();
        // Поле «надпись + ввод (+ список)» в строке y.
        fn field(d: &mut Dialog, y: i32, label: &str, text: &str, combo: Option<&[&str]>, width: i32) -> ViewId {
            let x = 18;
            let w = if combo.is_some() { width - 3 } else { width };
            let mut il = InputLine::new(Rect::new(x, y, x + w, y + 1), 128);
            il.set_text(text);
            let id = d.add(il);
            if let Some(items) = combo {
                d.add(Combo::new(Rect::new(x + w, y, x + w + 3, y + 1), id, cbx::NO_TRANSFER, items));
            }
            d.add(Label::new(Rect::new(2, y, x, y + 1), label, Some(id)));
            id
        }
        let ok_cancel = |d: &mut Dialog, y: i32, w: i32| {
            d.add(Button::new(Rect::new(w / 2 - 13, y, w / 2 - 2, y + 2), "O~K~", cm::OK, bf::DEFAULT));
            d.add(Button::new(Rect::new(w / 2 + 1, y, w / 2 + 12, y + 2), "Отмена", cm::CANCEL, bf::NORMAL));
        };
        match self.kind {
            LeKind::Menu => {
                let r = if p.new {
                    MenuRow { depth: 0, sub: p.sub, sep: false, title: String::new(), command: String::new(), key: String::new(), help: 0 }
                } else {
                    self.rows[p.index].clone()
                };
                if r.sub {
                    let mut d = Dialog::centered(56, 9, "Подменю");
                    let first = field(&mut d, 2, "~Н~адпись", &r.title, None, 34);
                    d.add(StaticText::new(Rect::new(2, 4, 54, 5), "~Ф~айл — буква в тильдах выбирается Alt+буква."));
                    ok_cancel(&mut d, 6, 56);
                    d.select(first);
                    d
                } else {
                    let mut d = Dialog::centered(60, 15, "Пункт меню");
                    let first = field(&mut d, 2, "~Н~адпись", &r.title, None, 38);
                    field(&mut d, 4, "~К~оманда", &r.command, Some(&cmd_refs), 38);
                    field(&mut d, 6, "К~л~авиша", &r.key, None, 20);
                    field(&mut d, 8, "~С~правка", &r.help.to_string(), None, 8);
                    d.add(StaticText::new(Rect::new(2, 10, 58, 11), "Клавиша: F2, Alt+X, Ctrl+N, Shift+F6. Новое имя команды —"));
                    d.add(StaticText::new(Rect::new(2, 11, 58, 12), "новая команда программы (появится в списке команд)."));
                    ok_cancel(&mut d, 12, 60);
                    d.select(first);
                    d
                }
            }
            LeKind::Status => {
                let s = if p.new {
                    StatusEntry { text: String::new(), key: String::new(), command: String::new() }
                } else {
                    self.app.status[p.index].clone()
                };
                let mut d = Dialog::centered(60, 13, "Элемент строки состояния");
                let first = field(&mut d, 2, "~Т~екст", &s.text, None, 38);
                field(&mut d, 4, "~К~лавиша", &s.key, None, 20);
                field(&mut d, 6, "К~о~манда", &s.command, Some(&cmd_refs), 38);
                d.add(StaticText::new(Rect::new(2, 8, 58, 9), "Текст «~F2~ Сохранить»; пустой — клавиша без надписи."));
                ok_cancel(&mut d, 10, 60);
                d.select(first);
                d
            }
            LeKind::Commands => {
                let c = if p.new {
                    CommandDef { name: String::new(), value: self.app.next_command_value(), action: Action::None, comment: String::new() }
                } else {
                    self.app.commands[p.index].clone()
                };
                let forms: Vec<&str> = self.forms.iter().map(|s| s.as_str()).collect();
                let mut d = Dialog::centered(64, 18, "Команда");
                let first = field(&mut d, 2, "~И~мя", &c.name, None, 30);
                field(&mut d, 3, "~Н~омер", &c.value.to_string(), None, 8);
                let mut rb =
                    RadioButtons::radio_buttons(Rect::new(18, 5, 50, 8), &["Ваш ~к~од (main.rs)", "Открыть ~ф~орму", "Показать ~с~ообщение"]);
                rb.value = match c.action {
                    Action::None => 0,
                    Action::Form(_) => 1,
                    Action::Message(_) => 2,
                };
                let rb = d.add(rb);
                d.add(Label::new(Rect::new(2, 5, 18, 6), "~Д~ействие", Some(rb)));
                let form = if let Action::Form(f) = &c.action { f.clone() } else { String::new() };
                let msg = if let Action::Message(m) = &c.action { m.clone() } else { String::new() };
                field(&mut d, 9, "~Ф~орма", &form, Some(&forms), 30);
                field(&mut d, 10, "~Т~екст", &msg, None, 42);
                field(&mut d, 12, "Ко~м~ментарий", &c.comment, None, 42);
                d.add(StaticText::new(Rect::new(2, 13, 62, 14), "Имя — константа ui::cmd::ИМЯ; номера 100–255 можно запрещать."));
                ok_cancel(&mut d, 15, 64);
                d.select(first);
                d
            }
        }
    }

    /// Строки из данных диалога правки.
    fn strings_of(v: &Value) -> Vec<Value> {
        v.as_list().map(|l| l.to_vec()).unwrap_or_default()
    }

    /// Убедиться, что команда известна; новое имя — новая команда.
    fn ensure_command(&mut self, name: &str) -> Result<String, String> {
        let n = name.trim();
        if n.is_empty() {
            return Err("Не указана команда.".into());
        }
        if self.app.command_value(n).is_some() {
            return Ok(n.to_string());
        }
        let up = n.strip_prefix("cmd::").unwrap_or(n).to_uppercase();
        if !is_ident(&up) {
            return Err(format!("«{}» — не команда и не годится как имя новой команды.", n));
        }
        if BUILTIN_COMMANDS.iter().any(|(b, _, _)| b.trim_start_matches("cm::") == up) {
            return Ok(format!("cm::{}", up));
        }
        let value = self.app.next_command_value();
        self.app.commands.push(CommandDef { name: up.clone(), value, action: Action::None, comment: String::new() });
        Ok(up)
    }

    /// Применить данные диалога правки. Ошибка — текст для пользователя.
    fn apply(&mut self, p: Pending, data: &Value) -> Result<(), String> {
        let v = Self::strings_of(data);
        let s = |i: usize| v.get(i).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match self.kind {
            LeKind::Menu => {
                let is_sub = if p.new { p.sub } else { self.rows[p.index].sub };
                let depth = if p.new {
                    // Новый пункт — на уровне текущей строки (или внутри подменю под курсором).
                    match self.rows.get(p.index) {
                        Some(r) if r.sub => r.depth + 1,
                        Some(r) => r.depth,
                        None => usize::from(!is_sub && !self.rows.is_empty()),
                    }
                } else {
                    self.rows[p.index].depth
                };
                let title = s(0);
                if title.trim().is_empty() {
                    return Err("Надпись не может быть пустой.".into());
                }
                let mut row = MenuRow { depth, sub: is_sub, sep: false, title, command: String::new(), key: String::new(), help: 0 };
                if !is_sub {
                    row.command = self.ensure_command(&s(1))?;
                    row.key = s(2).trim().to_string();
                    if !row.key.is_empty() && parse_key(&row.key).is_none() {
                        return Err(format!("Непонятная клавиша «{}». Примеры: F2, Alt+X, Ctrl+N.", row.key));
                    }
                    row.help = s(3).trim().parse().unwrap_or(0);
                }
                if p.new {
                    let at = (p.index + 1).min(self.rows.len());
                    self.rows.insert(at, row);
                    self.refresh(Some(at));
                } else {
                    self.rows[p.index] = row;
                    self.refresh(Some(p.index));
                }
            }
            LeKind::Status => {
                let mut e = StatusEntry { text: s(0), key: s(1).trim().to_string(), command: String::new() };
                if !e.key.is_empty() && parse_key(&e.key).is_none() {
                    return Err(format!("Непонятная клавиша «{}». Примеры: F2, Alt+X, Ctrl+N.", e.key));
                }
                if e.key.is_empty() && e.text.is_empty() {
                    return Err("Нужен текст или клавиша.".into());
                }
                e.command = self.ensure_command(&s(2))?;
                if p.new {
                    let at = (p.index + 1).min(self.app.status.len());
                    self.app.status.insert(at, e);
                    self.refresh(Some(at));
                } else {
                    self.app.status[p.index] = e;
                    self.refresh(Some(p.index));
                }
            }
            LeKind::Commands => {
                let name = s(0).trim().to_uppercase();
                if !is_ident(&name) {
                    return Err(format!("«{}» не годится как имя константы.", name));
                }
                if self.app.commands.iter().enumerate().any(|(i, c)| c.name == name && (p.new || i != p.index)) {
                    return Err(format!("Команда {} уже есть.", name));
                }
                let value: u16 = s(1).trim().parse().map_err(|_| "Номер команды — число от 1 до 65535.".to_string())?;
                if value == 0 {
                    return Err("Номер команды — число от 1 до 65535.".into());
                }
                if let Some(&(b, _, _)) = BUILTIN_COMMANDS.iter().find(|(_, bv, _)| *bv == value) {
                    return Err(format!("Номер {} занят стандартной командой {}.", value, b));
                }
                let action = match v.get(2).and_then(|x| x.as_u32()).unwrap_or(0) {
                    1 => {
                        let f = s(3).trim().to_string();
                        if !self.forms.contains(&f) {
                            return Err(format!("В проекте нет формы «{}».", f));
                        }
                        Action::Form(f)
                    }
                    2 => Action::Message(s(4)),
                    _ => Action::None,
                };
                let old_name = if p.new { None } else { Some(self.app.commands[p.index].name.clone()) };
                let c = CommandDef { name: name.clone(), value, action, comment: s(5) };
                if p.new {
                    let at = (p.index + 1).min(self.app.commands.len());
                    self.app.commands.insert(at, c);
                    self.refresh(Some(at));
                } else {
                    self.app.commands[p.index] = c;
                    self.refresh(Some(p.index));
                }
                // Переименование команды — во всех ссылках меню и строки состояния.
                if let Some(old) = old_name.filter(|o| *o != name) {
                    for r in self.rows.iter_mut() {
                        if r.command == old {
                            r.command = name.clone();
                        }
                    }
                    let mut menu = self.app.menu.clone();
                    rename_in_menu(&mut menu, &old, &name);
                    self.app.menu = menu;
                    for e in self.app.status.iter_mut() {
                        if e.command == old {
                            e.command = name.clone();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn start_edit(&mut self, p: Pending, ctx: &mut Ctx) {
        if !p.new && self.kind == LeKind::Menu && self.rows.get(p.index).map(|r| r.sep).unwrap_or(true) {
            return;
        }
        if !p.new && p.index >= self.len() {
            return;
        }
        let d = self.edit_dialog(p);
        self.pending = Some(p);
        ctx.exec(Box::new(d), Some(self.dlg.id()), TAG_EDIT);
    }

    fn command(&mut self, c: u16, ctx: &mut Ctx) -> bool {
        let i = self.cursor();
        let n = self.len();
        match c {
            CM_LE_ADD => self.start_edit(Pending { index: if n == 0 { 0 } else { i }, new: true, sub: false }, ctx),
            CM_LE_SUB => self.start_edit(Pending { index: if n == 0 { 0 } else { i }, new: true, sub: true }, ctx),
            CM_LE_SEP => {
                let depth = self.rows.get(i).map(|r| if r.sub { r.depth + 1 } else { r.depth }).unwrap_or(1);
                let at = (i + 1).min(self.rows.len());
                self.rows
                    .insert(at, MenuRow { depth, sub: false, sep: true, title: String::new(), command: String::new(), key: String::new(), help: 0 });
                self.refresh(Some(at));
            }
            CM_LE_EDIT => self.start_edit(Pending { index: i, new: false, sub: false }, ctx),
            CM_LE_DEL if n > 0 => {
                match self.kind {
                    LeKind::Menu => {
                        // Подменю удаляется вместе со всем содержимым.
                        let d = self.rows[i].depth;
                        let mut end = i + 1;
                        if self.rows[i].sub {
                            while end < self.rows.len() && self.rows[end].depth > d {
                                end += 1;
                            }
                        }
                        self.rows.drain(i..end);
                    }
                    LeKind::Status => {
                        self.app.status.remove(i);
                    }
                    LeKind::Commands => {
                        let name = self.app.commands[i].name.clone();
                        if self.rows.iter().any(|r| r.command == name) || self.app.status.iter().any(|e| e.command == name) {
                            self.info(&format!("\x03Команда {} используется в меню\n\x03или строке состояния.", name), ctx);
                            return true;
                        }
                        self.app.commands.remove(i);
                    }
                }
                self.refresh(Some(i));
            }
            CM_LE_UP | CM_LE_DOWN if n > 1 => {
                let j = if c == CM_LE_UP { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < n) };
                if let Some(j) = j {
                    match self.kind {
                        LeKind::Menu => self.rows.swap(i, j),
                        LeKind::Status => self.app.status.swap(i, j),
                        LeKind::Commands => self.app.commands.swap(i, j),
                    }
                    self.refresh(Some(j));
                }
            }
            CM_LE_LEFT if self.kind == LeKind::Menu && n > 0 => {
                if self.rows[i].depth > 0 {
                    self.rows[i].depth -= 1;
                    self.refresh(Some(i));
                }
            }
            CM_LE_RIGHT if self.kind == LeKind::Menu && n > 0 && i > 0 => {
                let prev = &self.rows[i - 1];
                let max = if prev.sub { prev.depth + 1 } else { prev.depth };
                if self.rows[i].depth < max {
                    self.rows[i].depth += 1;
                    self.refresh(Some(i));
                }
            }
            c if (CM_LE_ADD..=CM_LE_RIGHT).contains(&c) => {}
            _ => return false,
        }
        true
    }
}

fn rename_in_menu(items: &mut [MenuNode], old: &str, new: &str) {
    for it in items {
        match it {
            MenuNode::Item { command, .. } if command == old => *command = new.to_string(),
            MenuNode::Sub { children, .. } => rename_in_menu(children, old, new),
            _ => {}
        }
    }
}

impl View for ListEditor {
    tvision::impl_group_basics!(dlg.win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.dlg.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.dlg.draw(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.dlg.palette()
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.dlg.set_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        let list_focused = self.dlg.win.group.current_view().map(|v| v.vs().id) == Some(self.list_id);
        if let Event::KeyDown(k) = ev {
            if list_focused && k.mods == Mods::NONE {
                let c = match k.key {
                    Key::Enter => Some(CM_LE_EDIT),
                    Key::Ins => Some(CM_LE_ADD),
                    Key::Del => Some(CM_LE_DEL),
                    _ => None,
                };
                if let Some(c) = c {
                    self.command(c, ctx);
                    ev.clear();
                    return;
                }
            }
            if list_focused && k.mods == Mods::CTRL {
                let c = match k.key {
                    Key::Up => Some(CM_LE_UP),
                    Key::Down => Some(CM_LE_DOWN),
                    Key::Left => Some(CM_LE_LEFT),
                    Key::Right => Some(CM_LE_RIGHT),
                    _ => None,
                };
                if let Some(c) = c {
                    self.command(c, ctx);
                    ev.clear();
                    return;
                }
            }
        }
        if let Some(c) = ev.command() {
            if self.command(c, ctx) {
                ev.clear();
                return;
            }
        }
        self.dlg.handle_event(ev, ctx);
    }

    fn exec_result(&mut self, tag: u16, command: u16, view: Box<dyn View>, ctx: &mut Ctx) {
        if tag != TAG_EDIT {
            return;
        }
        let Some(p) = self.pending.take() else { return };
        if command != cm::OK {
            return;
        }
        let data = view.get_data();
        if let Err(e) = self.apply(p, &data) {
            self.info(&format!("\x03{}", e), ctx);
        }
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.dlg.valid(command, ctx)
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        self.dlg.size_limits(owner_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_roundtrip() {
        let app = AppModel::starter("x");
        let mut rows = Vec::new();
        flatten(&app.menu, 0, &mut rows);
        assert_eq!(rows.len(), 2 + 3 + 3);
        assert_eq!(unflatten(&rows), app.menu);
        // Лишняя глубина у пункта без подменю выравнивается.
        rows[2].depth = 5;
        let m = unflatten(&rows);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn apply_creates_commands() {
        let app = AppModel::starter("x");
        let mut e = ListEditor::new(LeKind::Menu, &app, &["about".into()]);
        let data = Value::List(vec![Value::Str("~Н~овый".into()), Value::Str("new_doc".into()), Value::Str("Ctrl+N".into()), Value::Str("0".into())]);
        e.apply(Pending { index: 0, new: true, sub: false }, &data).unwrap();
        assert!(e.app.commands.iter().any(|c| c.name == "NEW_DOC"));
        assert_eq!(e.rows[1].command, "NEW_DOC");
        assert_eq!(e.rows[1].depth, 1);
        let bad = Value::List(vec![Value::Str("x".into()), Value::Str("cm::QUIT".into()), Value::Str("Hyper+Q".into())]);
        assert!(e.apply(Pending { index: 0, new: true, sub: false }, &bad).is_err());
    }
}
