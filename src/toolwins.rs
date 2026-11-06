//! Окна-инструменты среды: палитра компонентов, дерево проекта,
//! сообщения сборки — и маленький «ретранслятор» сообщений.

use std::path::{Path, PathBuf};

use tvision::*;

use crate::cmds::*;
use crate::inspector::tool_palette;
use crate::model::{Kind, Project};

// ======================================================================
// Ретранслятор
// ======================================================================

/// Невидимое представление: превращает широковещательное сообщение
/// `cm::LIST_ITEM_SELECTED` от списка/дерева `watch` в команду `command`.
///
/// Списки сообщают о выборе соседям по окну, а не самому окну и не
/// программе; ретранслятор, вставленный в то же окно, пересылает выбор
/// дальше — до обработчика среды.
pub struct Relay {
    pub vs: ViewState,
    pub watch: ViewId,
    pub command: u16,
}

impl Relay {
    pub fn new(watch: ViewId, command: u16) -> Self {
        let mut vs = ViewState::new(Rect::new(0, 0, 0, 0));
        vs.state &= !sf::VISIBLE;
        Relay { vs, watch, command }
    }
}

impl View for Relay {
    tvision::impl_view_basics!();

    fn draw(&self, _dc: &mut DrawCtx) {}

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        if let Event::Broadcast(m) = ev {
            if m.command == cm::LIST_ITEM_SELECTED && m.info == Info::Id(self.watch) {
                ctx.post_command(self.command, Info::Id(self.watch));
            }
        }
    }
}

/// Общие методы окон-инструментов (окно + группа).
macro_rules! tool_window_view {
    () => {
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

        fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
            self.win.group.valid(command, ctx)
        }

        fn size_limits(&self, owner_size: Point) -> (Point, Point) {
            (Point::new(16, 5), owner_size)
        }
    };
}

/// Если в окне `win` в фокусе представление `id` и нажат Enter — послать команду.
fn enter_to_command(win: &Window, id: ViewId, ev: &mut Event, ctx: &mut Ctx, command: u16) {
    if let Event::KeyDown(k) = ev {
        if k.key == Key::Enter && k.mods == Mods::NONE && win.group.current_view().map(|v| v.vs().id) == Some(id) {
            ctx.post_command(command, Info::Id(id));
            ev.clear();
        }
    }
}

// ======================================================================
// Палитра компонентов
// ======================================================================

/// Палитра компонентов: Enter или двойной щелчок добавляет элемент
/// на форму в верхнем окне дизайнера.
pub struct PaletteWindow {
    pub win: Window,
    pub list_id: ViewId,
}

impl PaletteWindow {
    pub fn new(bounds: Rect) -> Self {
        let mut win = Window::new(bounds, "Компоненты", 0);
        win.palette = tool_palette();
        win.group.vs.help_ctx = HC_PALETTE;
        let vsb = win.standard_scroll_bar(true);
        let mut list = ListBox::new(win.interior(), 1, Some(vsb));
        list.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        list.vs.help_ctx = HC_PALETTE;
        list.set_items(Kind::ALL.iter().map(|k| format!("{} {}", icon(*k), k.title())).collect());
        let list_id = win.add(list);
        win.add(Relay::new(list_id, CM_PAL_INSERT));
        PaletteWindow { win, list_id }
    }

    /// Выбранный в палитре вид элемента.
    pub fn selected_kind(&self) -> Kind {
        let i = self.win.group.find::<ListBox>(self.list_id).map(|l| l.focused).unwrap_or(0);
        Kind::ALL[i.min(Kind::ALL.len() - 1)]
    }
}

/// Значок вида элемента в палитре.
pub fn icon(k: Kind) -> &'static str {
    match k {
        Kind::StaticText => "Aa ",
        Kind::Label => "~~ ",
        Kind::Button => "[ ]",
        Kind::InputLine => "▒▒ ",
        Kind::CheckBoxes => "[X]",
        Kind::RadioButtons => "(•)",
        Kind::ListBox => "≡  ",
        Kind::Memo => "¶  ",
        Kind::ComboBox => "▒↓ ",
        Kind::ScrollBar => "↕  ",
        Kind::ProgressBar => "█░ ",
        Kind::Table => "▦  ",
        Kind::TreeView => "├─ ",
    }
}

impl View for PaletteWindow {
    tool_window_view!();

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        enter_to_command(&self.win, self.list_id, ev, ctx, CM_PAL_INSERT);
        self.win.handle_window_event(ev, ctx);
    }
}

// ======================================================================
// Дерево проекта
// ======================================================================

/// Дерево проекта: формы, модель приложения, исходники, Cargo.toml.
/// Данные узла (`TreeNode::data`): `form:<имя>`, `model:<часть>`, `file:<путь>`.
pub struct ProjectWindow {
    pub win: Window,
    pub tree_id: ViewId,
}

impl ProjectWindow {
    pub fn new(bounds: Rect) -> Self {
        let mut win = Window::new(bounds, "Проект", 0);
        win.palette = tool_palette();
        win.group.vs.help_ctx = HC_PROJECT;
        let vsb = win.standard_scroll_bar(true);
        let mut tree = TreeView::new(win.interior(), vec![TreeNode::new("(нет проекта)")], Some(vsb), None);
        tree.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        tree.vs.help_ctx = HC_PROJECT;
        let tree_id = win.add(tree);
        win.add(Relay::new(tree_id, CM_PROJ_OPEN));
        ProjectWindow { win, tree_id }
    }

    /// Перестроить дерево (сохраняя раскрытые узлы и курсор, где возможно).
    pub fn set_project(&mut self, p: Option<&Project>) {
        let roots = match p {
            None => vec![TreeNode::new("(нет проекта)").data("")],
            Some(p) => {
                let mut forms = TreeNode::new("Формы").icon('▣').expanded(true).data("forms");
                for f in &p.forms {
                    forms = forms.child(TreeNode::new(f).icon('□').data(&format!("form:{}", f)));
                }
                let model = TreeNode::new("Модель приложения")
                    .icon('◆')
                    .expanded(true)
                    .data("model")
                    .child(TreeNode::new("Меню").data("model:menu"))
                    .child(TreeNode::new("Строка состояния").data("model:status"))
                    .child(TreeNode::new("Команды").data("model:commands"))
                    .child(TreeNode::new("Параметры").data("model:options"));
                let src = dir_node("src", &p.dir.join("src"));
                let cargo = TreeNode::new("Cargo.toml").data(&format!("file:{}", p.dir.join("Cargo.toml").display()));
                vec![TreeNode::new(&p.name).icon('◘').expanded(true).data("project").child(forms).child(model).child(src).child(cargo)]
            }
        };
        if let Some(t) = self.win.group.find_mut::<TreeView>(self.tree_id) {
            // Запоминаем раскрытые узлы по тексту пути.
            let mut open: Vec<Vec<usize>> = Vec::new();
            t.for_each(|path, n| {
                if n.expanded {
                    open.push(Vec::from(path));
                }
            });
            let open_texts: Vec<Vec<String>> = open.iter().map(|p| t.path_texts(p)).collect();
            let cur = t.cursor_path().map(|p| t.path_texts(&p));
            t.set_roots(roots);
            let mut expand: Vec<Vec<usize>> = Vec::new();
            t.for_each(|path, _| expand.push(Vec::from(path)));
            for p in expand {
                if open_texts.contains(&t.path_texts(&p)) {
                    t.expand(&p);
                }
            }
            if let Some(cur) = cur {
                let mut all: Vec<Vec<usize>> = Vec::new();
                t.for_each(|path, _| all.push(path.to_vec()));
                if let Some(p) = all.into_iter().find(|p| t.path_texts(p) == cur) {
                    t.select_path(&p);
                }
            }
        }
    }

    /// Данные узла под курсором.
    pub fn current_data(&self) -> Option<String> {
        self.win.group.find::<TreeView>(self.tree_id).and_then(|t| t.current()).map(|n| n.data.clone())
    }
}

/// Узел каталога с файлами `.rs` (рекурсивно, каталоги вперёд).
fn dir_node(name: &str, dir: &Path) -> TreeNode {
    let mut node = TreeNode::new(name).icon('▸').expanded(true).data(&format!("dir:{}", dir.display()));
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                files.push(p);
            }
        }
    }
    dirs.sort();
    files.sort();
    for d in dirs {
        let n = d.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        node = node.child(dir_node(&n, &d));
    }
    for f in files {
        let n = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let generated = std::fs::read_to_string(&f).map(|t| t.starts_with(crate::codegen::GENERATED_MARK)).unwrap_or(false);
        let mut leaf = TreeNode::new(&n).data(&format!("file:{}", f.display()));
        if generated {
            leaf = leaf.icon('⚙');
        }
        node = node.child(leaf);
    }
    node
}

impl View for ProjectWindow {
    tool_window_view!();

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        self.win.handle_window_event(ev, ctx);
    }
}

// ======================================================================
// Сообщения
// ======================================================================

/// Вид сообщения.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    Info,
    Error,
    Warning,
    Note,
}

/// Строка в окне сообщений.
#[derive(Clone, Debug, PartialEq)]
pub struct MsgEntry {
    pub text: String,
    pub kind: MsgKind,
    /// Место: файл (как напечатал cargo — относительно проекта), строка, столбец (с 1).
    pub loc: Option<(String, usize, usize)>,
}

/// Разбор строки cargo в формате `--message-format=short`:
/// `src/main.rs:12:5: error[E0425]: cannot find value`.
pub fn parse_cargo_line(line: &str) -> MsgEntry {
    let t = line.trim_end();
    let lower = t.trim_start().to_lowercase();
    let kind_of = |s: &str| {
        let s = s.trim_start();
        if s.starts_with("error") {
            MsgKind::Error
        } else if s.starts_with("warning") {
            MsgKind::Warning
        } else if s.starts_with("note") || s.starts_with("help") {
            MsgKind::Note
        } else {
            MsgKind::Info
        }
    };
    // Ищем «:строка:столбец:» — путь может содержать «C:\».
    let b: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == ':' {
            let mut j = i + 1;
            let ls = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > ls && j < b.len() && b[j] == ':' {
                let cs = j + 1;
                let mut k = cs;
                while k < b.len() && b[k].is_ascii_digit() {
                    k += 1;
                }
                if k > cs && k < b.len() && b[k] == ':' {
                    let file: String = b[..i].iter().collect();
                    let line_no: usize = b[ls..j].iter().collect::<String>().parse().unwrap_or(1);
                    let col: usize = b[cs..k].iter().collect::<String>().parse().unwrap_or(1);
                    let rest: String = b[k + 1..].iter().collect();
                    let file = file.trim().trim_start_matches("--> ").to_string();
                    if !file.is_empty() && !file.contains(' ') {
                        return MsgEntry { text: t.to_string(), kind: kind_of(&rest), loc: Some((file, line_no, col)) };
                    }
                }
            }
        }
        i += 1;
    }
    let kind = if lower.starts_with("error") {
        MsgKind::Error
    } else if lower.starts_with("warning") {
        MsgKind::Warning
    } else {
        MsgKind::Info
    };
    MsgEntry { text: t.to_string(), kind, loc: None }
}

/// Окно сообщений: вывод cargo, найденное «поиском в файлах», события среды.
/// Enter или двойной щелчок по строке с местом — открыть файл на этой строке.
pub struct MessagesWindow {
    pub win: Window,
    pub list_id: ViewId,
    pub entries: Vec<MsgEntry>,
}

impl MessagesWindow {
    pub fn new(bounds: Rect) -> Self {
        let mut win = Window::new(bounds, "Сообщения", 0);
        win.palette = tool_palette();
        win.group.vs.help_ctx = HC_MESSAGES;
        let vsb = win.standard_scroll_bar(true);
        let mut list = ListBox::new(win.interior(), 1, Some(vsb));
        list.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        list.vs.help_ctx = HC_MESSAGES;
        let list_id = win.add(list);
        win.add(Relay::new(list_id, CM_MSG_GOTO));
        MessagesWindow { win, list_id, entries: Vec::new() }
    }

    fn list_mut(&mut self) -> Option<&mut ListBox> {
        let id = self.list_id;
        self.win.group.find_mut::<ListBox>(id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        if let Some(l) = self.list_mut() {
            l.set_items(Vec::new());
        }
    }

    /// Добавить строку (и показать её — список прокручивается вниз).
    pub fn push(&mut self, e: MsgEntry) {
        let shown = match e.kind {
            MsgKind::Error => format!("■ {}", e.text),
            MsgKind::Warning => format!("▲ {}", e.text),
            MsgKind::Note => format!("  {}", e.text),
            MsgKind::Info => format!("  {}", e.text),
        };
        self.entries.push(e);
        let n = self.entries.len();
        if let Some(l) = self.list_mut() {
            let mut items = std::mem::take(&mut l.items);
            items.push(shown);
            let focused = l.focused;
            let follow = focused + 2 >= n || n == 1;
            l.items = items;
            if follow {
                l.focus_item(n - 1);
            }
        }
    }

    pub fn info(&mut self, text: &str) {
        for line in text.lines() {
            self.push(MsgEntry { text: line.to_string(), kind: MsgKind::Info, loc: None });
        }
    }

    /// Номер строки под курсором.
    pub fn focused(&self) -> usize {
        self.win.group.find::<ListBox>(self.list_id).map(|l| l.focused).unwrap_or(0)
    }

    /// Поставить курсор на строку.
    pub fn focus(&mut self, i: usize) {
        if let Some(l) = self.list_mut() {
            l.focus_item(i);
        }
    }

    /// Следующая (или предыдущая) строка с местом, начиная от курсора.
    pub fn next_location(&self, forward: bool) -> Option<usize> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        let cur = self.focused();
        let is_target = |e: &MsgEntry| e.loc.is_some() && matches!(e.kind, MsgKind::Error | MsgKind::Warning);
        if forward {
            ((cur + 1)..n).chain(0..=cur).find(|&i| is_target(&self.entries[i]))
        } else {
            (0..cur).rev().chain((cur..n).rev()).find(|&i| is_target(&self.entries[i]))
        }
    }

    /// Число ошибок и предупреждений.
    pub fn counts(&self) -> (usize, usize) {
        let e = self.entries.iter().filter(|m| m.kind == MsgKind::Error && m.loc.is_some()).count();
        let w = self.entries.iter().filter(|m| m.kind == MsgKind::Warning && m.loc.is_some()).count();
        (e, w)
    }
}

impl View for MessagesWindow {
    tool_window_view!();

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        enter_to_command(&self.win, self.list_id, ev, ctx, CM_MSG_GOTO);
        self.win.handle_window_event(ev, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_lines() {
        let e = parse_cargo_line("src/main.rs:12:5: error[E0425]: cannot find value `x`");
        assert_eq!(e.kind, MsgKind::Error);
        assert_eq!(e.loc, Some(("src/main.rs".into(), 12, 5)));
        let e = parse_cargo_line("C:\\work\\app\\src\\ui.rs:3:1: warning: unused import");
        assert_eq!(e.kind, MsgKind::Warning);
        assert_eq!(e.loc.unwrap().0, "C:\\work\\app\\src\\ui.rs");
        let e = parse_cargo_line("error: could not compile `app` (bin \"app\") due to 1 previous error");
        assert_eq!(e.kind, MsgKind::Error);
        assert!(e.loc.is_none());
        let e = parse_cargo_line("   Compiling app v0.1.0 (/x)");
        assert_eq!(e.kind, MsgKind::Info);
    }
}
