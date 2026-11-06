//! Пример: виджет `TreeView` — дерево с ленивой загрузкой, флажками и правкой.
//!
//! Запуск:
//! ```text
//! cargo run --example treeview            # дерево текущего каталога
//! cargo run --example treeview -- /etc    # дерево заданного каталога
//! ```
//!
//! Окна:
//! * **Файлы** — дерево каталогов. Содержимое каталога читается только при
//!   первом раскрытии (ленивая загрузка), поэтому открывается мгновенно даже
//!   для `/`. Внизу — сведения об узле под курсором: путь, размер, время.
//! * **План** — дерево, которое можно править: F7 — новый вложенный узел,
//!   Ins — новый узел рядом, F2 — переименовать, Del — удалить.
//! * **JSON** (F3 — открыть файл, F4 — образец `books.json`) — объекты
//!   и массивы показаны узлами со значками `{` и `[`, значения — листьями.
//! * **Компоненты** (F9) — диалог с флажками: отметка раздела отмечает всё
//!   внутри, частично отмеченный раздел показан как `[-]`.
//!
//! Клавиши в дереве: ↑ ↓ PgUp PgDn Home End; → — раскрыть / к первому
//! ребёнку; ← — свернуть / к родителю; `+` `-` `*` — раскрыть, свернуть,
//! раскрыть всю ветку; Enter — раскрыть или выбрать; пробел — флажок;
//! буквы — быстрый поиск; Ctrl+←/→ — прокрутка вбок. Мышь: щелчок по
//! стрелке — раскрыть, по флажку — отметить, двойной щелчок — как Enter.

use std::path::{Path, PathBuf};

use tvision::*;

#[path = "json/mod.rs"]
mod json;
use json::{parse_json, Json};

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


// ======================================================================
// Файловая система: ленивая загрузка
// ======================================================================

/// Узел для пути: каталог — ленивый (дети при раскрытии), файл — лист.
/// Полный путь хранится в `data`.
pub fn fs_node(path: &Path, name: &str) -> TreeNode {
    let n = TreeNode::new(name).data(&path.to_string_lossy());
    if path.is_dir() {
        n.lazy()
    } else {
        n
    }
}

/// Сравнение имён «по-человечески»: группы цифр сравниваются как числа
/// (`file2` раньше `file10`), остальное — без учёта регистра.
pub fn name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(|c| c.is_ascii_digit()) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (m, n) = (take(&mut x), take(&mut y));
                let (m, n) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let o = m.len().cmp(&n.len()).then_with(|| m.cmp(n));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Дети каталога: сначала каталоги, потом файлы, по имени (см. [`name_cmp`]).
pub fn fs_children(dir: &Path) -> Vec<TreeNode> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => return vec![TreeNode::new(&format!("‹нет доступа: {}›", e.kind()))],
    };
    let mut items: Vec<(bool, String, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .map(|e| {
            let p = e.path();
            (p.is_dir(), e.file_name().to_string_lossy().to_string(), p)
        })
        .collect();
    items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| name_cmp(&a.1, &b.1)));
    items.iter().map(|(_, name, p)| fs_node(p, name)).collect()
}

/// Дерево каталога: корень раскрыт сразу.
pub fn fs_tree(bounds: Rect, root: &Path, v: Option<ViewId>, h: Option<ViewId>) -> TreeView {
    let name = root.to_string_lossy().to_string();
    let mut tree = TreeView::new(bounds, vec![fs_node(root, &name)], v, h)
        // Загрузчик получает путь в дереве и сам узел; путь файла — в `data`.
        .with_loader(|_, node| fs_children(Path::new(&node.data)));
    tree.expand(&[0]);
    tree
}

/// Текст сведений об узле файлового дерева: в первой строке — имя, размер
/// и время (они видны всегда), во второй — полный путь.
pub fn fs_info(node: Option<&TreeNode>) -> String {
    let n = match node {
        Some(n) if !n.data.is_empty() => n,
        _ => return String::new(),
    };
    let p = Path::new(&n.data);
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) => return format!("{}\n{}", e, n.data),
    };
    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| n.data.clone());
    let time = format_time_short(meta.modified().ok());
    if meta.is_dir() {
        let inside = if n.lazy { "не прочитан".to_string() } else { format!("элементов: {}", n.children.len()) };
        format!("{}/ — {}, {}\n{}", name, inside, time, n.data)
    } else {
        format!("{} — {} байт, {}\n{}", name, format_size_full(meta.len()), time, n.data)
    }
}

// ======================================================================
// JSON → дерево
// ======================================================================

/// Узел для значения JSON: объект и массив — ветви с числом элементов,
/// остальное — лист «ключ: значение».
pub fn json_node(key: &str, v: &Json) -> TreeNode {
    match v {
        Json::Obj(fields) => {
            TreeNode::new(&format!("{}  ({})", key, fields.len())).icon('{').children(fields.iter().map(|(k, x)| json_node(k, x)).collect())
        }
        Json::Arr(items) => TreeNode::new(&format!("{}  ({})", key, items.len()))
            .icon('[')
            .children(items.iter().enumerate().map(|(i, x)| json_node(&format!("[{}]", i), x)).collect()),
        other => TreeNode::new(&format!("{}: {}", key, other.compact())),
    }
}

// ======================================================================
// План: дерево для правки
// ======================================================================

fn plan_nodes() -> Vec<TreeNode> {
    let t = TreeNode::new;
    vec![t("Переезд в новый офис")
        .expanded(true)
        .child(t("Подготовка").expanded(true).child(t("Выбрать помещение")).child(t("Подписать договор")).child(t("Составить смету")))
        .child(t("Переезд").child(t("Заказать грузчиков")).child(t("Упаковать технику").child(t("Серверы")).child(t("Рабочие места"))))
        .child(t("После переезда").child(t("Настроить сеть")).child(t("Новоселье")))]
}

// ======================================================================
// Окно с деревом
// ======================================================================

/// Окно с деревом, полосами прокрутки и (у файлового дерева) строками
/// сведений внизу.
pub struct TreeWindow {
    pub win: Window,
    pub tree: ViewId,
    /// Строки сведений (у файлового дерева).
    pub info: Option<ViewId>,
    /// Узлы можно править (окно «План»).
    pub editable: bool,
    /// Путь под курсором при последнем обновлении сведений.
    last: Option<TreePath>,
}

impl TreeWindow {
    /// Окно; `make` строит дерево по прямоугольнику и полосам прокрутки.
    fn build(bounds: Rect, title: &str, number: i16, info: bool, make: impl FnOnce(Rect, ViewId, ViewId) -> TreeView) -> Self {
        let mut win = Window::new(bounds, title, number);
        win.group.vs.options |= of::TILEABLE;
        let v = win.standard_scroll_bar(true);
        let h = win.standard_scroll_bar(false);
        let r = win.interior();
        let (tree_r, info_r) =
            if info { (Rect::new(r.a.x, r.a.y, r.b.x, r.b.y - 3), Some(Rect::new(r.a.x, r.b.y - 2, r.b.x, r.b.y))) } else { (r, None) };
        let tree = win.add(make(tree_r, v, h));
        let info = info_r.map(|ir| {
            // Линия-разделитель и две строки сведений; при изменении размера
            // окна они остаются внизу.
            let mut sep = StaticText::new(Rect::new(ir.a.x, ir.a.y - 1, ir.b.x, ir.a.y), &"─".repeat(ir.width() as usize));
            sep.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y | gf::GROW_HI_X;
            win.add(sep);
            let mut st = StaticText::new(ir, "");
            st.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y | gf::GROW_HI_X;
            win.add(st)
        });
        let mut w = TreeWindow { win, tree, info, editable: false, last: None };
        w.update_info();
        w
    }

    /// Окно «Файлы» для каталога.
    pub fn files(bounds: Rect, root: &Path, number: i16) -> Self {
        let title = format!("Файлы: {}", root.display());
        Self::build(bounds, &title, number, true, |r, v, h| fs_tree(r, root, Some(v), Some(h)))
    }

    /// Окно «План» (правка).
    pub fn plan(bounds: Rect, number: i16) -> Self {
        let mut w = Self::build(bounds, "План", number, false, |r, v, h| TreeView::new(r, plan_nodes(), Some(v), Some(h)));
        w.editable = true;
        w
    }

    /// Окно с файлом JSON.
    pub fn json(bounds: Rect, path: &Path, number: i16) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Не удалось прочитать файл\n{}\n{}", path.display(), e))?;
        let j = parse_json(&text).map_err(|e| format!("Ошибка в JSON:\n{}", e))?;
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let root = json_node(&name, &j).expanded(true);
        Ok(Self::build(bounds, &format!("JSON: {}", name), number, false, |r, v, h| TreeView::new(r, vec![root], Some(v), Some(h))))
    }

    pub fn tree(&self) -> &TreeView {
        self.win.group.find::<TreeView>(self.tree).unwrap()
    }

    pub fn tree_mut(&mut self) -> &mut TreeView {
        let id = self.tree;
        self.win.group.find_mut::<TreeView>(id).unwrap()
    }

    /// Обновить сведения, если курсор дерева сдвинулся (или узел раскрыли).
    fn update_info(&mut self) {
        let Some(info) = self.info else { return };
        let tree = self.tree();
        let path = tree.cursor_path();
        let text = fs_info(tree.current());
        // Сравниваем и путь, и текст: раскрытие каталога меняет «элементов: N».
        let st = self.win.group.find_mut::<StaticText>(info).unwrap();
        if path != self.last || st.text != text {
            st.text = text;
            self.last = path;
        }
    }
}

impl View for TreeWindow {
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
        self.win.handle_window_event(ev, ctx);
        // Сообщение `cm::TREE_CURSOR_MOVED` дерево шлёт своим соседям по
        // окну, а не самому окну, — поэтому окно просто сверяет курсор после
        // каждого события.
        self.update_info();
    }
}

// ======================================================================
// Диалог «Компоненты» с флажками
// ======================================================================

fn components() -> Vec<TreeNode> {
    let t = TreeNode::new;
    vec![
        t("Библиотека").expanded(true).child(t("Ядро").checked(true)).child(t("Виджеты").checked(true)).child(t("Справка")),
        t("Примеры").expanded(true).child(t("hello")).child(t("tvdemo").checked(true)).child(t("treeview").checked(true)).child(t("lfm")),
        t("Документация").child(t("Руководство")).child(t("Справочник")),
        t("Исходники C++ для сравнения"),
    ]
}

/// Диалог «Компоненты». Возвращает диалог и id дерева в нём.
pub fn components_dialog() -> (Dialog, ViewId) {
    let mut d = Dialog::centered(52, 18, "Компоненты");
    d.add(StaticText::new(Rect::new(3, 2, 49, 3), "Пробел — отметить, Enter на листе — OK"));
    let sb = d.add(ScrollBar::new(Rect::new(47, 4, 48, 13)));
    let tree = TreeView::new(Rect::new(3, 4, 47, 13), components(), Some(sb), None).checkboxes(true);
    let tid = d.add(tree);
    d.add(Button::new(Rect::new(12, 14, 24, 16), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(27, 14, 39, 16), "Отмена", cm::CANCEL, bf::NORMAL));
    d.select(tid);
    (d, tid)
}

/// Выбранные компоненты: «Раздел / элемент» для каждого отмеченного листа.
pub fn chosen(tree: &TreeView) -> Vec<String> {
    tree.checked_paths().iter().map(|p| tree.path_texts(p).join(" / ")).collect()
}

// ======================================================================
// Приложение
// ======================================================================

struct App {
    windows: i16,
    root: PathBuf,
}

impl App {
    fn next_number(&mut self) -> i16 {
        self.windows += 1;
        if self.windows < 10 {
            self.windows
        } else {
            0
        }
    }

    /// Активное окно с деревом.
    fn active(app: &mut Program) -> Option<&mut TreeWindow> {
        app.desktop_mut().group.current_view_mut()?.as_any_mut().downcast_mut::<TreeWindow>()
    }

    fn open_files(&mut self, app: &mut Program, r: Rect) {
        let n = self.next_number();
        let root = self.root.clone();
        app.insert_window(TreeWindow::files(r, &root, n));
    }

    fn open_plan(&mut self, app: &mut Program, r: Rect) {
        let n = self.next_number();
        app.insert_window(TreeWindow::plan(r, n));
    }

    fn open_json(&mut self, app: &mut Program, path: &Path) {
        let r = app.desktop().group.vs.extent();
        let n = self.next_number();
        match TreeWindow::json(r, path, n) {
            Ok(w) => {
                app.insert_window(w);
            }
            Err(e) => {
                message_box_rect(app, Rect::new(0, 0, 60, 11), &e, mf::ERROR | mf::OK_BUTTON, true);
            }
        }
    }

    fn json_dialog(&mut self, app: &mut Program) {
        let d = FileDialog::new("*.json", "Открыть JSON", "~И~мя", fd::OPEN_BUTTON, 100);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            let p = d.file_name();
            self.open_json(app, &p);
        }
    }

    fn components(&mut self, app: &mut Program) {
        let (d, tid) = components_dialog();
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::OK {
            return;
        }
        let list = chosen(d.win.group.find::<TreeView>(tid).unwrap());
        let text = if list.is_empty() {
            "Ничего не выбрано.".to_string()
        } else {
            format!("Будут установлены:\n{}", list.join("\n"))
        };
        message_box_rect(app, Rect::new(0, 0, 54, 16), &text, mf::INFORMATION | mf::OK_BUTTON, true);
    }

    /// Правка дерева «План»: добавить узел, переименовать, удалить.
    fn edit(&mut self, app: &mut Program, cmd: u16) {
        // Сначала читаем всё нужное, потом показываем диалоги: окно нельзя
        // держать «взаймы», пока работает модальный диалог.
        let (path, text, has_children) = match Self::active(app) {
            Some(w) if w.editable => {
                let t = w.tree();
                match (t.cursor_path(), t.current()) {
                    (Some(p), Some(n)) => (p, n.text.clone(), !n.children.is_empty()),
                    _ => return,
                }
            }
            _ => {
                message_box(app, "\x03Править можно только дерево «План».", mf::INFORMATION | mf::OK_BUTTON);
                return;
            }
        };
        match cmd {
            CM_ADD_CHILD | CM_ADD_SIBLING => {
                let Some(name) = input_box(app, "Новый узел", "~Н~азвание:", "", 60) else { return };
                if name.trim().is_empty() {
                    return;
                }
                let tree = Self::active(app).unwrap().tree_mut();
                let node = TreeNode::new(name.trim());
                let new_path = if cmd == CM_ADD_CHILD {
                    tree.push(Some(&path), node)
                } else {
                    let (last, parent) = path.split_last().unwrap();
                    tree.insert(if parent.is_empty() { None } else { Some(parent) }, last + 1, node)
                };
                if let Some(p) = new_path {
                    tree.select_path(&p);
                }
            }
            CM_RENAME => {
                let Some(name) = input_box(app, "Переименовать", "~Н~азвание:", &text, 60) else { return };
                if name.trim().is_empty() {
                    return;
                }
                let tree = Self::active(app).unwrap().tree_mut();
                tree.node_mut(&path).unwrap().text = name.trim().to_string();
                tree.refresh();
            }
            CM_DELETE => {
                if has_children {
                    let q = format!("Удалить «{}» вместе со всем, что внутри?", text);
                    if message_box(app, &q, mf::CONFIRMATION | mf::YES_BUTTON | mf::NO_BUTTON) != cm::YES {
                        return;
                    }
                }
                Self::active(app).unwrap().tree_mut().remove(&path);
            }
            _ => {}
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        let Some(c) = ev.command() else { return };
        match c {
            CM_FILES => {
                let r = app.desktop().group.vs.extent();
                self.open_files(app, r);
            }
            CM_PLAN => {
                let r = app.desktop().group.vs.extent();
                self.open_plan(app, r);
            }
            CM_OPEN_JSON => self.json_dialog(app),
            CM_SAMPLE_JSON => {
                let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/data/books.json");
                self.open_json(app, &p);
            }
            CM_COMPONENTS => self.components(app),
            CM_ADD_CHILD | CM_ADD_SIBLING | CM_RENAME | CM_DELETE => self.edit(app, c),
            CM_EXPAND_ALL | CM_COLLAPSE_ALL => {
                if let Some(w) = Self::active(app) {
                    let t = w.tree_mut();
                    if c == CM_EXPAND_ALL {
                        // Ленивые каталоги при этом не читаются — раскрывается то, что уже загружено.
                        t.expand_all();
                    } else {
                        t.collapse_all();
                    }
                }
            }
            _ => return,
        }
        ev.clear();
    }
}

fn menu() -> Menu {
    ui::menu_bar()
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

/// Окна при запуске: «План» слева, «Файлы» справа.
fn open_start_windows(app: &mut Program, h: &mut App) {
    let r = app.desktop().group.vs.extent();
    let mid = r.b.x * 2 / 5;
    h.open_plan(app, Rect::new(0, 0, mid, r.b.y));
    h.open_files(app, Rect::new(mid, 0, r.b.x, r.b.y));
}

fn main() -> std::io::Result<()> {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let mut app = Program::with_terminal(Some(menu()), Some(status()))?;
    let mut h = App { windows: 0, root };
    open_start_windows(&mut app, &mut h);
    app.run(&mut h);
    Ok(())
}

// ======================================================================
// Тесты: cargo test --example treeview
// ======================================================================

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

    fn program(input: Vec<RawEvent>) -> Program {
        let mut b = HeadlessBackend::new(100, 30);
        b.input.extend(input);
        Program::new(Box::new(b), Some(menu()), Some(status()))
    }

    /// Каталог с файлами для теста.
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("treeview-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("каталог/вложенный")).unwrap();
        std::fs::write(d.join("file10.txt"), "10").unwrap();
        std::fs::write(d.join("file2.txt"), "2").unwrap();
        std::fs::write(d.join("каталог/внутри.txt"), "привет").unwrap();
        d
    }

    fn row_texts(t: &TreeView) -> Vec<String> {
        (0..t.row_count()).map(|i| t.node(t.row_path(i).unwrap()).unwrap().text.clone()).collect()
    }

    #[test]
    fn fs_tree_is_lazy_and_sorted() {
        let d = tmp("fs");
        let mut t = fs_tree(Rect::new(0, 0, 60, 20), &d, None, None);
        // Каталоги первыми, имена — в естественном порядке.
        assert_eq!(row_texts(&t)[1..], ["каталог", "file2.txt", "file10.txt"]);
        let dir = t.find_text("каталог").unwrap();
        assert!(t.node(&dir).unwrap().lazy, "каталог ещё не прочитан");
        t.expand(&dir);
        assert_eq!(t.node(&dir).unwrap().children.len(), 2);
        assert!(t.find_text("внутри.txt").is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn names_compare_naturally() {
        let mut v = vec!["file10.txt", "File2.txt", "file1.txt", "a", "file02b"];
        v.sort_by(|a, b| name_cmp(a, b));
        assert_eq!(v, ["a", "file1.txt", "File2.txt", "file02b", "file10.txt"]);
    }

    #[test]
    fn json_tree() {
        let j = parse_json(r#"{"имя": "Аня", "теги": ["a", "b"], "адрес": {"город": "Тула"}}"#).unwrap();
        let n = json_node("x.json", &j);
        assert_eq!(n.text, "x.json  (3)");
        assert_eq!(n.icon, Some('{'));
        assert_eq!(n.children[0].text, "имя: \"Аня\"");
        assert_eq!(n.children[1].icon, Some('['));
        assert_eq!(n.children[1].children[1].text, "[1]: \"b\"");
        assert_eq!(n.count(), 7);
    }

    #[test]
    fn components_choice() {
        let (mut d, tid) = components_dialog();
        let t = d.win.group.find_mut::<TreeView>(tid).unwrap();
        assert_eq!(t.check_state(&[0]), CheckState::Partial);
        // Отметить «Документацию» целиком и снять «Примеры».
        t.toggle_check(&[2]);
        t.set_checked(&[1], false);
        assert_eq!(chosen(t), ["Библиотека / Ядро", "Библиотека / Виджеты", "Документация / Руководство", "Документация / Справочник"]);
    }

    #[test]
    fn components_dialog_keys() {
        // Пробел снимает «Библиотеку» (она частично отмечена → отмечается целиком),
        // ↓ на «Ядро», пробел снимает его, Esc тестового терминала закрывает диалог.
        let (d, tid) = components_dialog();
        let mut app = program(vec![key(Key::Char(' ')), key(Key::Down), key(Key::Char(' '))]);
        let (_, d) = app.exec_dialog(d);
        let t = d.win.group.find::<TreeView>(tid).unwrap();
        assert_eq!(t.check_state(&[0, 0]), CheckState::Unchecked);
        assert_eq!(t.check_state(&[0, 2]), CheckState::Checked, "Справка отмечена вместе с разделом");
        assert_eq!(t.check_state(&[0]), CheckState::Partial);
    }

    #[test]
    fn plan_editing() {
        // F7 на корне: новый вложенный узел «Итоги»; потом переименовать в «Отчёт».
        let mut input = text("Итоги");
        input.push(key(Key::Enter));
        let mut app = program(input);
        let mut h = App { windows: 0, root: PathBuf::from(".") };
        let r = app.desktop().group.vs.extent();
        h.open_plan(&mut app, r);
        app.put_event(Event::command_event(CM_ADD_CHILD));
        app.step(&mut h);
        let w = App::active(&mut app).unwrap();
        assert_eq!(w.tree().current().unwrap().text, "Итоги");
        assert_eq!(w.tree().cursor_path(), Some(vec![0, 3]));

        // Переименование. Ввод для окна input_box кладём в очередь событий
        // программы после команды; текст поля выделен целиком — набор заменяет его.
        app.put_event(Event::command_event(CM_RENAME));
        for e in text("Отчёт").into_iter().chain([key(Key::Enter)]) {
            if let RawEvent::Key(k) = e {
                app.put_event(Event::KeyDown(k));
            }
        }
        app.step(&mut h);
        let w = App::active(&mut app).unwrap();
        assert_eq!(w.tree().current().unwrap().text, "Отчёт");

        // Удаление листа — без вопроса.
        app.put_event(Event::command_event(CM_DELETE));
        app.step(&mut h);
        let w = App::active(&mut app).unwrap();
        assert!(w.tree().find_text("Отчёт").is_none());
        assert_eq!(w.tree().current().unwrap().text, "После переезда");
    }

    #[test]
    fn files_window_info_follows_cursor() {
        let d = tmp("info");
        let mut app = program(vec![key(Key::End)]);
        let mut h = App { windows: 0, root: d.clone() };
        let r = app.desktop().group.vs.extent();
        h.open_files(&mut app, r);
        app.step(&mut h);
        app.redraw();
        let screen = app.screen_text();
        assert!(screen.contains("file10.txt"), "{}", screen);
        assert!(screen.contains("file10.txt\n") || screen.contains("/file10.txt"), "{}", screen);
        assert!(screen.contains("file10.txt — 2 байт"), "сведения о последнем файле:\n{}", screen);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn start_screen() {
        let mut app = program(vec![]);
        let mut h = App { windows: 0, root: tmp("start") };
        open_start_windows(&mut app, &mut h);
        app.redraw();
        let s = app.screen_text();
        assert!(s.contains("План") && s.contains("Файлы"), "{}", s);
        assert!(s.contains("▾ Переезд в новый офис"), "{}", s);
        assert!(s.contains("├▸ Переезд"), "{}", s);
    }
}
