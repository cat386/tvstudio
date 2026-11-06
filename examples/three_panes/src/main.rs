//! Окно, разделённое линиями на три части: дерево, таблица и строка ввода.
//!
//! Запуск: `cargo run --example three_panes`
//!
//! ```text
//! ╔═[■]═══════════════╤══════ Склад — Продукты (10) ═══════════1═[↑]═╗
//! ║└▾ Склад           │Артикул │     Наименование     │ Кол-во│ Цена ▲
//! ║  ├▾ Продукты      │────────┼──────────────────────┼───────┼──────■
//! ║  │ ├─ Овощи       │П-101   │Картофель             │    800│ 35,00▒
//! ║  │ └─ Фрукты      │П-201   │Яблоки «Антоновка»    │    120│ 89,00▼
//! ╟───────────────────┴───────────────────────────────────────────────╢
//! ║ Фильтр: груш█                                                     ║
//! └─══════════════════════════════════════════════════════════════════─┘
//! ```
//!
//! * часть 1 (слева) — `TreeView`: категории товаров;
//! * часть 2 (справа) — `Table`: товары выбранной категории (для узла
//!   с подкатегориями — товары всех подкатегорий);
//! * часть 3 (снизу) — `InputLine`: Enter применяет фильтр по артикулу
//!   и наименованию, пустая строка + Enter — сбросить фильтр.
//!
//! Tab / Shift+Tab — переход между частями, F5 — распахнуть окно,
//! Ctrl+F5 — размер/перемещение (линии-разделители следуют за размером),
//! Alt+X — выход.

use std::collections::HashMap;

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;

/// Ширина левой части (дерева) вместе с рамкой слева.
const TREE_WIDTH: i32 = 22;

// ======================================================================
// Данные
// ======================================================================

/// Товары по ключу категории (ключ хранится в `TreeNode::data` листа).
fn goods() -> HashMap<&'static str, Vec<[&'static str; 4]>> {
    let mut m = HashMap::new();
    m.insert(
        "veg",
        vec![
            ["П-101", "Картофель", "800", "35,00"],
            ["П-102", "Морковь", "260", "42,50"],
            ["П-103", "Свёкла", "190", "39,90"],
            ["П-104", "Лук репчатый", "310", "29,00"],
        ],
    );
    m.insert(
        "fruit",
        vec![["П-201", "Яблоки «Антоновка»", "120", "89,00"], ["П-202", "Груши", "45", "140,00"], ["П-203", "Апельсины", "75", "119,00"]],
    );
    m.insert("milk", vec![["П-301", "Молоко 3,2%", "64", "92,00"], ["П-302", "Кефир", "38", "85,00"], ["П-303", "Творог 9%", "22", "176,00"]]);
    m.insert("pc", vec![["Т-101", "Ноутбук 15\"", "4", "64 990,00"], ["Т-102", "Монитор 27\"", "7", "21 500,00"]]);
    m.insert("acc", vec![["Т-201", "Клавиатура", "18", "2 490,00"], ["Т-202", "Мышь", "33", "990,00"], ["Т-203", "Кабель USB-C", "120", "390,00"]]);
    m.insert("tools", vec![["Х-101", "Молоток", "15", "650,00"], ["Х-102", "Отвёртки, набор", "9", "1 290,00"]]);
    m
}

/// Дерево категорий. У листьев в `data` — ключ для [`goods`].
fn categories() -> Vec<TreeNode> {
    vec![TreeNode::new("Склад").expanded(true).children(vec![
        TreeNode::new("Продукты").expanded(true).children(vec![
            TreeNode::new("Овощи").data("veg"),
            TreeNode::new("Фрукты").data("fruit"),
            TreeNode::new("Молочное").data("milk"),
        ]),
        TreeNode::new("Техника").children(vec![TreeNode::new("Компьютеры").data("pc"), TreeNode::new("Аксессуары").data("acc")]),
        TreeNode::new("Хозтовары").data("tools"),
    ])]
}

/// Столбцы таблицы. `Table::set_model` пересоздаёт столбцы, поэтому
/// после каждой смены модели они задаются заново.
fn columns() -> Vec<TableColumn> {
    vec![TableColumn::new(8), TableColumn::new(22).limits(8, 60), TableColumn::new(7).align(Align::Right), TableColumn::new(11).align(Align::Right)]
}

/// Собрать ключи всех листьев под узлом (сам узел — тоже, если это лист).
fn leaf_keys(node: &TreeNode, out: &mut Vec<String>) {
    if !node.data.is_empty() {
        out.push(node.data.clone());
    }
    for c in &node.children {
        leaf_keys(c, out);
    }
}

// ======================================================================
// Окно из трёх частей
// ======================================================================

/// Окно «Склад». В C++ это был бы наследник `TWindow`; в Rust — структура,
/// которая содержит `Window` и сама реализует трейт `View`.
pub struct SkladWindow {
    win: Window,
    tree: ViewId,
    table: ViewId,
    input: ViewId,
    goods: HashMap<&'static str, Vec<[&'static str; 4]>>,
    /// Путь узла дерева, для которого сейчас заполнена таблица.
    shown: Option<TreePath>,
    /// Текущий фильтр (пустой — без фильтра).
    filter: String,
}

impl SkladWindow {
    pub fn new(bounds: Rect) -> Self {
        let mut win = Window::new(bounds, "Склад", 1);
        win.group.vs.options |= of::TILEABLE;
        let size = win.group.vs.size;
        // Строка горизонтальной линии: над строкой ввода и под таблицей.
        let hline = size.y - 3;

        // --- Часть 1: дерево (от рамки до вертикальной линии). ---
        let mut tree = TreeView::new(Rect::new(1, 1, TREE_WIDTH, hline), categories(), None, None).lines(true);
        // Дерево растёт только по высоте: ширина левой части постоянна.
        tree.vs.grow_mode = gf::GROW_HI_Y;
        let tree = win.add(tree);

        // --- Часть 2: таблица (правее линии) с полосой прокрутки на рамке. ---
        let mut sb = ScrollBar::new(Rect::new(size.x - 1, 1, size.x, hline));
        sb.vs.grow_mode = gf::GROW_LO_X | gf::GROW_HI_X | gf::GROW_HI_Y;
        let sb = win.add(sb);
        let model = VecTableModel::new(&["Артикул", "Наименование", "Кол-во", "Цена, ₽"], Vec::new());
        let mut table = Table::new(Rect::new(TREE_WIDTH + 1, 1, size.x - 1, hline), Box::new(model), Some(sb), None)
            .with_columns(columns())
            .selection(SelectionMode::Row)
            .sortable(true)
            .striped(true);
        table.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        let table = win.add(table);

        // --- Часть 3: строка ввода под горизонтальной линией. ---
        let input_y = size.y - 2;
        let mut input = InputLine::new(Rect::new(10, input_y, size.x - 1, input_y + 1), 80);
        input.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y | gf::GROW_HI_X;
        let input = win.add(input);
        let mut label = Label::new(Rect::new(1, input_y, 10, input_y + 1), "~Ф~ильтр:", Some(input));
        label.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y;
        win.add(label);

        let mut w = SkladWindow { win, tree, table, input, goods: goods(), shown: None, filter: String::new() };
        // Фокус — на дерево, таблицу заполняем для узла под курсором.
        w.win.group.select_id(tree, &mut Ctx::new());
        w.sync_table(true);
        w
    }

    fn tree(&self) -> &TreeView {
        self.win.group.find::<TreeView>(self.tree).expect("дерево")
    }

    /// Перезаполнить таблицу, если курсор дерева встал на другой узел
    /// (или `force` — например, после смены фильтра).
    fn sync_table(&mut self, force: bool) {
        let path = self.tree().cursor_path();
        if !force && path == self.shown {
            return;
        }
        // Ключи листьев под текущим узлом.
        let mut keys = Vec::new();
        let mut caption = String::from("Склад");
        if let Some(node) = self.tree().current() {
            leaf_keys(node, &mut keys);
            caption = format!("Склад — {}", node.text);
        }
        // Строки таблицы с учётом фильтра (без учёта регистра).
        let needle = self.filter.to_lowercase();
        let mut total = 0;
        let rows: Vec<Vec<String>> = keys
            .iter()
            .filter_map(|k| self.goods.get(k.as_str()))
            .flatten()
            .inspect(|_| total += 1)
            .filter(|r| needle.is_empty() || r[0].to_lowercase().contains(&needle) || r[1].to_lowercase().contains(&needle))
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect();

        // Заголовок окна показывает категорию и сколько строк прошло фильтр.
        self.win.title = if self.filter.is_empty() {
            format!("{} ({})", caption, rows.len())
        } else {
            format!("{} ({} из {}, фильтр «{}»)", caption, rows.len(), total, self.filter)
        };

        let model = VecTableModel::new(&["Артикул", "Наименование", "Кол-во", "Цена, ₽"], rows);
        if let Some(t) = self.win.group.find_mut::<Table>(self.table) {
            t.set_model(Box::new(model));
            t.set_columns(columns());
        }
        self.shown = path;
    }

    /// Enter в строке ввода — применить фильтр.
    fn apply_filter(&mut self) {
        let text = self.win.group.find::<InputLine>(self.input).map(|i| i.text()).unwrap_or_default();
        self.filter = text.trim().to_string();
        self.sync_table(true);
    }

    /// Нарисовать линии, делящие окно на три части. Линии рисуются тем же
    /// цветом и тем же стилем, что и рамка, а в местах стыка с рамкой
    /// ставятся «тройники» (╤ ╧ ╟ ╢ у двойной рамки, ┬ ├ ┤ у одинарной).
    fn draw_splitters(&self, dc: &mut DrawCtx) {
        let vs = &self.win.group.vs;
        let (w, h) = (vs.size.x, vs.size.y);
        let hline = h - 3;
        let active = vs.is_active() && vs.state & sf::DRAGGING == 0;
        // Номера цветов в палитре рамки: 1 — пассивная, 3 — активная.
        let mut fdc = dc.sub(vs.extent(), Frame::palette());
        let attr = fdc.color(if active { 3 } else { 1 });
        let (top, left, right) = if active { ('╤', '╟', '╢') } else { ('┬', '├', '┤') };
        let x = TREE_WIDTH;

        // Вертикальная линия между деревом и таблицей.
        fdc.write_char(x, 0, top, attr, 1);
        for y in 1..hline {
            fdc.write_char(x, y, '│', attr, 1);
        }
        // Горизонтальная линия над строкой ввода.
        fdc.write_char(0, hline, left, attr, 1);
        fdc.write_char(1, hline, '─', attr, w - 2);
        fdc.write_char(x, hline, '┴', attr, 1);
        fdc.write_char(w - 1, hline, right, attr, 1);
    }
}

impl View for SkladWindow {
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        // Рамка и дети, затем линии-разделители поверх рамки.
        self.win.draw_window(dc);
        self.draw_splitters(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn size_limits(&self, owner: Point) -> (Point, Point) {
        // Не даём окну стать уже левой части + места под таблицу.
        let (_, max) = self.win.size_limits(owner);
        (Point::new(TREE_WIDTH + 16, 8), max)
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        // Enter в строке ввода: строка сама Enter не обрабатывает.
        if let Event::KeyDown(k) = ev {
            let input_focused = self.win.group.current_view().map(|v| v.vs().id) == Some(self.input);
            if k.key == Key::Enter && k.mods == Mods::NONE && input_focused {
                self.apply_filter();
                ev.clear();
                return;
            }
        }
        self.win.handle_window_event(ev, ctx);
        // `cm::TREE_CURSOR_MOVED` дерево шлёт соседям, а не окну, поэтому
        // после каждого события просто сверяем, не сдвинулся ли курсор.
        self.sync_table(false);
    }
}

// ======================================================================
// Приложение
// ======================================================================

struct App;

impl AppHandler for App {}

fn main() -> std::io::Result<()> {
    let menu = ui::menu_bar();
    let status = ui::status_line();

    let mut app = Program::with_terminal(Some(menu), Some(status))?;
    let size = app.desktop().vs().size;
    app.insert_window(SkladWindow::new(Rect::new(0, 0, size.x, size.y)));
    app.run(&mut App);
    Ok(())
}

// ======================================================================
// Проверка на «невидимом» терминале: `cargo test --example three_panes`
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    fn key(k: Key) -> KeyEvent {
        KeyEvent::new(k, Mods::NONE)
    }

    #[test]
    fn three_parts_and_filter() {
        let mut b = HeadlessBackend::new(80, 25);
        b.esc_budget = 0;
        let mut app = Program::new(Box::new(b), None, None);
        let size = app.desktop().vs().size;
        let id = app.insert_window(SkladWindow::new(Rect::new(0, 0, size.x, size.y)));
        app.redraw();
        let s = app.screen_text();
        println!("{}", s);
        // Линии-разделители и стыки с рамкой.
        assert!(s.contains('╤') && s.contains('┴') && s.contains('╟') && s.contains('╢'));
        // Под курсором дерева корень «Склад» — в таблице все 17 товаров.
        assert!(s.contains("Склад — Склад (17)"));

        // Фильтр «мо»: Молоко, Монитор, Молоток.
        let w = app.find_mut::<SkladWindow>(id).unwrap();
        let input = w.input;
        w.win.group.find_mut::<InputLine>(input).unwrap().set_text("мо");
        w.apply_filter();
        assert_eq!(w.win.group.find::<Table>(w.table).unwrap().model().row_count(), 4); // Морковь, Молоко, Монитор, Молоток
        app.redraw();
        assert!(app.screen_text().contains("4 из 17"));

        // То же через клавиатуру: Tab, Tab — в строку ввода, сбросить
        // фильтр пустой строкой и Enter.
        let w = app.find_mut::<SkladWindow>(id).unwrap();
        w.win.group.find_mut::<InputLine>(input).unwrap().set_text("");
        app.put_event(Event::KeyDown(key(Key::Tab)));
        app.put_event(Event::KeyDown(key(Key::Tab)));
        app.put_event(Event::KeyDown(key(Key::Enter)));
        app.pump(&mut App);
        app.redraw();
        let w = app.find::<SkladWindow>(id).unwrap();
        assert!(w.filter.is_empty());
        assert_eq!(w.win.group.find::<Table>(w.table).unwrap().model().row_count(), 17);
    }
}
