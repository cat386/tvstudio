//! Приложение из урока руководства пользователя (урок 9: `docs/guide/09-contacts.md`):
//! записная книжка «Контакты».
//!
//! Запуск:
//! ```text
//! cargo run --example guide_contacts                 # с образцом данных
//! cargo run --example guide_contacts -- мои.tsv      # открыть файл
//! ```
//!
//! Что показывает (по шагам урока):
//! 1. программа, строка меню и строка состояния;
//! 2. своё окно с таблицей контактов;
//! 3. диалог «Контакт»: строки ввода с проверкой, переключатели, флажки,
//!    обмен данными через `Value`;
//! 4. команды «Добавить», «Изменить», «Удалить», запрет недоступных команд;
//! 5. чтение и запись файла, диалог выбора файла, вопрос «Сохранить?»;
//! 6. тесты на «невидимом» терминале.

use std::path::{Path, PathBuf};

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// ----------------------------------------------------------------------
// Шаг 1. Команды приложения
// ----------------------------------------------------------------------

// Свои команды нумеруются от 100 (номера 0–99 заняты стандартными `cm::*`).

// ----------------------------------------------------------------------
// Данные
// ----------------------------------------------------------------------

/// Группы контакта — подписи переключателей и столбца «Группа».
const GROUPS: [&str; 3] = ["Семья", "Работа", "Друзья"];

/// Один контакт.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contact {
    pub name: String,
    pub phone: String,
    pub email: String,
    /// Номер группы (0 — «Семья», 1 — «Работа», 2 — «Друзья»).
    pub group: u32,
    pub favorite: bool,
}

impl Contact {
    fn new(name: &str, phone: &str, email: &str, group: u32, favorite: bool) -> Self {
        Contact { name: name.into(), phone: phone.into(), email: email.into(), group, favorite }
    }

    /// Ячейки строки таблицы.
    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.phone.clone(),
            self.email.clone(),
            GROUPS.get(self.group as usize).unwrap_or(&"?").to_string(),
            if self.favorite { "★".into() } else { String::new() },
        ]
    }

    // --- Шаг 3. Обмен данными с диалогом ---

    /// Данные для диалога: по одному значению на каждый элемент с данными,
    /// в порядке их вставки в диалог.
    fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()),
            Value::Str(self.phone.clone()),
            Value::Str(self.email.clone()),
            Value::U32(self.group),           // переключатели: номер выбранного
            Value::U32(self.favorite as u32), // флажки: битовая маска
        ])
    }

    fn from_value(v: &Value) -> Self {
        let l = v.as_list().unwrap_or(&[]);
        let s = |i: usize| l.get(i).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
        let n = |i: usize| l.get(i).and_then(|x| x.as_u32()).unwrap_or(0);
        Contact { name: s(0), phone: s(1), email: s(2), group: n(3), favorite: n(4) & 1 != 0 }
    }

    // --- Шаг 5. Формат файла: строка на контакт, поля через табуляцию ---

    fn to_line(&self) -> String {
        format!("{}\t{}\t{}\t{}\t{}", self.name, self.phone, self.email, self.group, self.favorite as u8)
    }

    fn from_line(line: &str) -> Option<Self> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            return None;
        }
        Some(Contact::new(f[0], f[1], f[2], f[3].parse().unwrap_or(0), f[4] == "1"))
    }
}

/// Образец данных для первого запуска.
fn sample_contacts() -> Vec<Contact> {
    vec![
        Contact::new("Анна Петрова", "+7 900 111-22-33", "anna@example.ru", 0, true),
        Contact::new("Борис Смирнов", "+7 900 222-33-44", "boris@example.ru", 1, false),
        Contact::new("Вера Иванова", "+7 900 333-44-55", "", 2, true),
        Contact::new("Глеб Орлов", "+7 900 444-55-66", "gleb@work.example", 1, false),
    ]
}

/// Прочитать контакты из файла.
pub fn load_contacts(path: &Path) -> Result<Vec<Contact>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Не удалось прочитать {}:\n{}", path.display(), e))?;
    Ok(text.lines().filter_map(Contact::from_line).collect())
}

/// Записать контакты в файл.
pub fn save_contacts(path: &Path, contacts: &[Contact]) -> Result<(), String> {
    let text: String = contacts.iter().map(|c| c.to_line() + "\n").collect();
    std::fs::write(path, text).map_err(|e| format!("Не удалось записать {}:\n{}", path.display(), e))
}

// ----------------------------------------------------------------------
// Шаг 3. Диалог «Контакт»
// ----------------------------------------------------------------------

/// Диалог правки контакта. Порядок `add` = порядок перехода по Tab =
/// порядок значений в `Contact::to_value`.
pub fn contact_dialog(title: &str, c: &Contact) -> Dialog {
    let mut d = Dialog::centered(52, 17, title);

    // Имя: не может быть пустым. Проверка — при нажатии OK.
    let name = InputLine::new(Rect::new(13, 2, 49, 3), 60)
        .with_validator(FnValidator { f: |s: &str| !s.trim().is_empty(), message: "Имя не может быть пустым.".into() });
    let name = d.add(name);
    d.add(Label::new(Rect::new(2, 2, 12, 3), "~И~мя:", Some(name)));

    // Телефон: только цифры и знаки + - ( ) и пробел — лишнее не наберётся.
    let phone = d.add(InputLine::new(Rect::new(13, 4, 49, 5), 30).with_validator(FilterValidator::new("0123456789+-() ")));
    d.add(Label::new(Rect::new(2, 4, 12, 5), "~Т~елефон:", Some(phone)));

    // Почта: пусто или с «@».
    let email = InputLine::new(Rect::new(13, 6, 49, 7), 60)
        .with_validator(FnValidator {
            f: |s: &str| s.trim().is_empty() || s.contains('@'), message: "В адресе почты нет «@».".into()
        });
    let email = d.add(email);
    d.add(Label::new(Rect::new(2, 6, 12, 7), "~П~очта:", Some(email)));

    let group = d.add(Cluster::radio_buttons(Rect::new(3, 9, 22, 12), &["~С~емья", "~Р~абота", "~Д~рузья"]));
    d.add(Label::new(Rect::new(2, 8, 12, 9), "~Г~руппа", Some(group)));

    d.add(Cluster::check_boxes(Rect::new(26, 9, 49, 10), &["Из~б~ранный"]));

    d.add(Button::new(Rect::new(12, 13, 24, 15), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(27, 13, 39, 15), "Отмена", cm::CANCEL, bf::NORMAL));

    d.set_data(&c.to_value());
    d.select(name);
    d
}

/// Показать диалог; `Some(контакт)` — нажали OK.
fn edit_contact(app: &mut Program, title: &str, c: &Contact) -> Option<Contact> {
    let (cmd, d) = app.exec_dialog(contact_dialog(title, c));
    (cmd == cm::OK).then(|| Contact::from_value(&d.get_data()))
}

// ----------------------------------------------------------------------
// Шаг 2. Окно с таблицей
// ----------------------------------------------------------------------

/// Окно «Контакты»: таблица и данные, которые она показывает.
pub struct ContactsWindow {
    pub win: Window,
    pub table: ViewId,
    pub contacts: Vec<Contact>,
    pub file: Option<PathBuf>,
    pub modified: bool,
}

impl ContactsWindow {
    pub fn new(bounds: Rect, contacts: Vec<Contact>, file: Option<PathBuf>) -> Self {
        let mut win = Window::new(bounds, "", 1);
        // Полосы прокрутки — на рамке окна.
        let v = win.standard_scroll_bar(true);
        let h = win.standard_scroll_bar(false);
        let model = VecTableModel::new(&["Имя", "Телефон", "Почта", "Группа", "★"], Vec::new());
        let cols =
            vec![TableColumn::new(22), TableColumn::new(18), TableColumn::new(22), TableColumn::new(8), TableColumn::new(3).align(Align::Center)];
        let mut table = Table::new(win.interior(), Box::new(model), Some(v), Some(h))
            .with_columns(cols)
            .selection(SelectionMode::Row)
            .striped(true)
            // Порядок строк задаём сами (команда «Сортировать»), чтобы номер
            // строки таблицы всегда совпадал с номером в `contacts`.
            .sortable(false);
        // Растягиваться вместе с окном (F5, мышь) — таблица сама этого не делает.
        table.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        let table = win.add(table);
        let mut w = ContactsWindow { win, table, contacts, file, modified: false };
        w.reload();
        w
    }

    fn table_mut(&mut self) -> &mut Table {
        let id = self.table;
        self.win.group.find_mut::<Table>(id).unwrap()
    }

    /// Номер контакта под курсором.
    pub fn current(&self) -> Option<usize> {
        let (row, _) = self.win.group.find::<Table>(self.table)?.cursor();
        (row < self.contacts.len()).then_some(row)
    }

    /// Показать `contacts` в таблице и обновить заголовок.
    pub fn reload(&mut self) {
        let rows: Vec<Vec<String>> = self.contacts.iter().map(|c| c.cells()).collect();
        let t = self.table_mut();
        let (row, _) = t.cursor();
        let cols = t.columns.clone();
        t.set_model(Box::new(VecTableModel::new(&["Имя", "Телефон", "Почта", "Группа", "★"], rows)));
        t.set_columns(cols); // `set_model` пересоздаёт столбцы — возвращаем свои
        t.set_cursor(row, 0);
        t.refresh();
        let name = self.file.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "без имени".into());
        self.win.title = format!("Контакты — {}{}", name, if self.modified { " *" } else { "" });
    }

    /// Поставить курсор на контакт `i`.
    pub fn select(&mut self, i: usize) {
        self.table_mut().set_cursor(i, 0);
    }
}

impl View for ContactsWindow {
    // Окно построено вокруг `Window`: группа, рамка, состояние — у него.
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        let double = matches!(ev, Event::MouseDown(m) if m.double);
        self.win.handle_window_event(ev, ctx);
        // Таблица не поглощает Enter, и он возвращается к окну: превращаем
        // его (и двойной щелчок) в команду «Изменить».
        if matches!(ev, Event::KeyDown(k) if k.key == Key::Enter && k.mods == Mods::NONE) || double {
            ctx.post_command(CM_EDIT, Info::None);
            ev.clear();
        }
    }
}

// ----------------------------------------------------------------------
// Шаги 1, 4, 5. Приложение
// ----------------------------------------------------------------------

struct App;

impl App {
    /// Окно контактов на рабочем столе (если открыто).
    fn window(app: &mut Program) -> Option<&mut ContactsWindow> {
        let desk = &mut app.desktop_mut().group;
        desk.children.iter_mut().find_map(|w| w.as_any_mut().downcast_mut::<ContactsWindow>())
    }

    fn open_window(app: &mut Program, contacts: Vec<Contact>, file: Option<PathBuf>) {
        // Старое окно закрываем: в этом приложении окно одно.
        let old: Vec<ViewId> = app.desktop().group.children.iter().filter(|w| w.as_any().is::<ContactsWindow>()).map(|w| w.vs().id).collect();
        for id in old {
            app.desktop_mut().group.remove(id);
        }
        let r = app.desktop().group.vs.extent();
        app.insert_window(ContactsWindow::new(r, contacts, file));
    }

    // --- Шаг 4. Правка ---

    fn add(&mut self, app: &mut Program) {
        if let Some(c) = edit_contact(app, "Новый контакт", &Contact::default()) {
            let w = Self::window(app).unwrap();
            w.contacts.push(c);
            w.modified = true;
            w.reload();
            let last = w.contacts.len() - 1;
            w.select(last);
        }
    }

    fn edit(&mut self, app: &mut Program) {
        // Данные копируем до диалога: пока он открыт, окно «взаймы» не держим.
        let Some((i, c)) = Self::window(app).and_then(|w| w.current().map(|i| (i, w.contacts[i].clone()))) else { return };
        if let Some(new) = edit_contact(app, "Контакт", &c) {
            let w = Self::window(app).unwrap();
            if w.contacts[i] != new {
                w.contacts[i] = new;
                w.modified = true;
                w.reload();
            }
        }
    }

    fn delete(&mut self, app: &mut Program) {
        let Some((i, name)) = Self::window(app).and_then(|w| w.current().map(|i| (i, w.contacts[i].name.clone()))) else { return };
        let q = format!("\x03Удалить «{}»?", name);
        if message_box(app, &q, mf::CONFIRMATION | mf::YES_BUTTON | mf::NO_BUTTON) == cm::YES {
            let w = Self::window(app).unwrap();
            w.contacts.remove(i);
            w.modified = true;
            w.reload();
        }
    }

    fn sort(&mut self, app: &mut Program) {
        if let Some(w) = Self::window(app) {
            w.contacts.sort_by_key(|c| c.name.to_lowercase());
            w.modified = true;
            w.reload();
        }
    }

    // --- Шаг 5. Файлы ---

    fn open(&mut self, app: &mut Program) {
        if !self.confirm_save(app) {
            return;
        }
        let d = FileDialog::new("*.tsv", "Открыть", "~И~мя", fd::OPEN_BUTTON, 100);
        let (cmd, d) = app.exec_dialog(d);
        if cmd == cm::CANCEL {
            return;
        }
        let path = d.file_name();
        match load_contacts(&path) {
            Ok(list) => Self::open_window(app, list, Some(path)),
            Err(e) => {
                message_box(app, &e, mf::ERROR | mf::OK_BUTTON);
            }
        }
    }

    /// Записать в файл окна; без имени — спросить имя. `false` — не записано.
    fn save(&mut self, app: &mut Program, ask_name: bool) -> bool {
        let Some(w) = Self::window(app) else { return true };
        let mut path = w.file.clone();
        if ask_name || path.is_none() {
            let d = FileDialog::new("*.tsv", "Сохранить как", "~И~мя", fd::OK_BUTTON, 100);
            let (cmd, d) = app.exec_dialog(d);
            if cmd == cm::CANCEL {
                return false;
            }
            path = Some(d.file_name());
        }
        let path = path.unwrap();
        let w = Self::window(app).unwrap();
        match save_contacts(&path, &w.contacts) {
            Ok(()) => {
                w.file = Some(path);
                w.modified = false;
                w.reload();
                true
            }
            Err(e) => {
                message_box(app, &e, mf::ERROR | mf::OK_BUTTON);
                false
            }
        }
    }

    /// Есть несохранённые изменения — спросить. `false` — пользователь передумал.
    fn confirm_save(&mut self, app: &mut Program) -> bool {
        if !Self::window(app).map(|w| w.modified).unwrap_or(false) {
            return true;
        }
        match message_box(app, "\x03Сохранить изменения?", mf::CONFIRMATION | mf::YES_NO_CANCEL) {
            cm::YES => self.save(app, false),
            cm::NO => true,
            _ => false,
        }
    }
}

impl AppHandler for App {
    /// Команды, которые не обработали окна, приходят сюда.
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        let Some(cmd) = ev.command() else { return };
        match cmd {
            CM_ADD => self.add(app),
            CM_EDIT => self.edit(app),
            CM_DELETE => self.delete(app),
            CM_SORT => self.sort(app),
            cm::NEW => {
                if self.confirm_save(app) {
                    Self::open_window(app, Vec::new(), None);
                }
            }
            cm::OPEN => self.open(app),
            cm::SAVE => {
                self.save(app, false);
            }
            cm::SAVE_AS => {
                self.save(app, true);
            }
            CM_EXIT => {
                if self.confirm_save(app) {
                    app.quit();
                }
            }
            _ => return,
        }
        ev.clear();
    }

    /// Когда событий нет: разрешить только то, что сейчас имеет смысл.
    /// Запрещённые пункты меню и строки состояния рисуются серым.
    fn idle(&mut self, app: &mut Program) {
        let (has_window, has_rows) = match Self::window(app) {
            Some(w) => (true, !w.contacts.is_empty()),
            None => (false, false),
        };
        for (cmd, on) in
            [(CM_ADD, has_window), (CM_EDIT, has_rows), (CM_DELETE, has_rows), (CM_SORT, has_rows), (cm::SAVE, has_window), (cm::SAVE_AS, has_window)]
        {
            if on {
                app.enable_command(cmd);
            } else {
                app.disable_command(cmd);
            }
        }
    }
}

// ----------------------------------------------------------------------
// Шаг 1. Меню и строка состояния
// ----------------------------------------------------------------------

fn menu() -> Menu {
    ui::menu_bar()
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    let mut app = Program::with_terminal(Some(menu()), Some(status()))?;
    let (contacts, file) = match std::env::args().nth(1).map(PathBuf::from) {
        Some(p) if p.exists() => match load_contacts(&p) {
            Ok(list) => (list, Some(p)),
            Err(e) => {
                eprintln!("{}", e);
                (Vec::new(), None)
            }
        },
        Some(p) => (Vec::new(), Some(p)), // новый файл
        None => (sample_contacts(), None),
    };
    App::open_window(&mut app, contacts, file);
    app.run(&mut App);
    Ok(())
}

// ----------------------------------------------------------------------
// Шаг 6. Тесты: cargo test --example guide_contacts
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::{HeadlessBackend, RawEvent};

    fn key(k: Key) -> RawEvent {
        RawEvent::Key(KeyEvent::new(k, Mods::NONE))
    }

    fn text(s: &str) -> Vec<RawEvent> {
        s.chars().map(|c| key(Key::Char(c))).collect()
    }

    /// Программа на «невидимом» терминале 100×30 с окном образца.
    fn program(input: Vec<RawEvent>) -> Program {
        let mut b = HeadlessBackend::new(100, 30);
        b.input.extend(input);
        let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
        App::open_window(&mut app, sample_contacts(), None);
        app
    }

    #[test]
    fn value_roundtrip() {
        let c = Contact::new("Аня", "123", "a@b", 2, true);
        assert_eq!(Contact::from_value(&c.to_value()), c);
        assert_eq!(Contact::from_line(&c.to_line()), Some(c));
    }

    #[test]
    fn window_shows_table() {
        let mut app = program(vec![]);
        app.redraw();
        let s = app.screen_text();
        assert!(s.contains("Контакты — без имени"), "{}", s);
        assert!(s.contains("Анна Петрова") && s.contains("Работа"), "{}", s);
    }

    #[test]
    fn dialog_validates_name() {
        // Пустое имя: OK не закрывает диалог, а показывает сообщение
        // (его закроет Esc тестового терминала), потом Esc закрывает диалог.
        let mut app = program(vec![key(Key::Enter)]);
        let (cmd, _) = app.exec_dialog(contact_dialog("Новый контакт", &Contact::default()));
        assert_eq!(cmd, cm::CANCEL, "без имени диалог OK не принимает");
    }

    #[test]
    fn add_contact_through_dialog() {
        // Ins → диалог; имя, Tab, телефон (буквы фильтр не пропустит), Enter — OK.
        let mut input = vec![key(Key::Ins)];
        input.extend(text("Дарья"));
        input.push(key(Key::Tab));
        input.extend(text("+7 (999) abc 12"));
        input.push(key(Key::Enter));
        let mut app = program(input);
        app.pump(&mut App);
        let w = App::window(&mut app).unwrap();
        assert_eq!(w.contacts.len(), 5);
        let c = w.contacts.last().unwrap();
        assert_eq!((c.name.as_str(), c.phone.as_str()), ("Дарья", "+7 (999)  12"));
        assert!(w.modified);
        assert_eq!(w.current(), Some(4), "курсор на новом контакте");
    }

    #[test]
    fn delete_asks_and_commands_follow_state() {
        // Del → вопрос, Enter — «Да» (кнопка по умолчанию).
        let mut app = program(vec![key(Key::Del), key(Key::Enter)]);
        app.pump(&mut App);
        assert_eq!(App::window(&mut app).unwrap().contacts.len(), 3);
        // Без строк «Изменить» и «Удалить» запрещены.
        App::window(&mut app).unwrap().contacts.clear();
        App.idle(&mut app);
        assert!(!app.command_enabled(CM_DELETE) && !app.command_enabled(CM_EDIT));
        assert!(app.command_enabled(CM_ADD));
    }

    #[test]
    fn save_and_load_file() {
        let path = std::env::temp_dir().join(format!("guide-contacts-{}.tsv", std::process::id()));
        save_contacts(&path, &sample_contacts()).unwrap();
        assert_eq!(load_contacts(&path).unwrap(), sample_contacts());
        let _ = std::fs::remove_file(&path);
    }
}
