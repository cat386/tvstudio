//! Инспектор свойств: таблица «свойство — значение» для выделенного
//! элемента формы (или самой формы, если ничего не выделено).
//!
//! Правка: Enter или F2 (или просто набор текста) — изменить значение,
//! Enter — применить, Esc — отменить; пробел на логическом свойстве —
//! переключить «да/нет». Ошибочное значение не принимается, среда
//! показывает, что не так.

use std::cell::RefCell;
use std::rc::Rc;

use tvision::color::palettes;
use tvision::*;

use crate::cmds::*;
use crate::designer::DocRef;
use crate::props::{self, Prop};

/// Модель таблицы: смотрит в документ формы, который сейчас «под инспектором».
pub struct PropModel {
    pub target: Rc<RefCell<Option<DocRef>>>,
}

impl PropModel {
    fn props(&self) -> Vec<Prop> {
        match &*self.target.borrow() {
            Some(d) => {
                let d = d.borrow();
                props::props_for(&d.form, d.primary())
            }
            None => Vec::new(),
        }
    }
}

impl TableModel for PropModel {
    fn row_count(&self) -> usize {
        self.props().len()
    }

    fn col_count(&self) -> usize {
        2
    }

    fn cell(&self, row: usize, col: usize) -> String {
        let t = self.target.borrow();
        let Some(d) = &*t else { return String::new() };
        let d = d.borrow();
        let sel = d.primary();
        let kind = sel.map(|i| d.form.controls[i].kind);
        let list = props::props_for(&d.form, sel);
        match (list.get(row), col) {
            (Some(p), 0) => p.label(kind).to_string(),
            (Some(p), _) => props::get(&d.form, sel, *p),
            _ => String::new(),
        }
    }

    fn header(&self, col: usize) -> String {
        if col == 0 { "Свойство" } else { "Значение" }.into()
    }

    fn editable(&self, _row: usize, col: usize) -> bool {
        col == 1
    }

    fn set_cell(&mut self, row: usize, col: usize, value: &str) -> bool {
        if col != 1 {
            return false;
        }
        let t = self.target.borrow();
        let Some(d) = &*t else { return false };
        let mut d = d.borrow_mut();
        let sel = d.primary();
        let Some(&p) = props::props_for(&d.form, sel).get(row) else { return false };
        let before = (d.form.clone(), d.sel.clone());
        match props::set(&mut d.form, sel, p, value) {
            Ok(()) => {
                if d.form != before.0 {
                    d.push_undo(before);
                    d.changed();
                }
                true
            }
            Err(e) => {
                d.error = Some(e);
                false
            }
        }
    }
}

/// Окно инспектора.
pub struct InspectorWindow {
    pub win: Window,
    pub table_id: ViewId,
    head_id: ViewId,
    hint_id: ViewId,
    target: Rc<RefCell<Option<DocRef>>>,
    /// Что показано сейчас: (адрес документа, ревизия, основной элемент).
    shown: (usize, u64, Option<usize>),
}

/// Палитра окон-инструментов (бирюзовые, как окно Message в Turbo Pascal):
/// внутри работают элементы диалогов.
pub fn tool_palette() -> &'static [u8] {
    palettes::CYAN_DIALOG
}

impl InspectorWindow {
    pub fn new(bounds: Rect) -> Self {
        let mut win = Window::new(bounds, "Инспектор", 0);
        win.palette = tool_palette();
        let s = win.group.vs.size;
        let mut head = StaticText::new(Rect::new(1, 1, s.x - 1, 2), "Нет формы");
        head.vs.grow_mode = gf::GROW_HI_X;
        let head_id = win.add(head);
        let vsb = win.standard_scroll_bar(true);
        let target: Rc<RefCell<Option<DocRef>>> = Rc::new(RefCell::new(None));
        let model = PropModel { target: target.clone() };
        let w = (s.x - 2).max(10);
        let mut table = Table::new(Rect::new(1, 2, s.x - 1, s.y - 2), Box::new(model), Some(vsb), None)
            .with_columns(vec![TableColumn::new(12).title("Свойство"), TableColumn::new((w - 13).max(8)).title("Значение")])
            .header(true)
            .editable(true)
            .sortable(false);
        table.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        table.vs.help_ctx = HC_INSPECTOR;
        let table_id = win.add(table);
        let mut hint = StaticText::new(Rect::new(1, s.y - 2, s.x - 1, s.y - 1), "");
        hint.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y | gf::GROW_HI_X;
        let hint_id = win.add(hint);
        win.group.vs.help_ctx = HC_INSPECTOR;
        InspectorWindow { win, table_id, head_id, hint_id, target, shown: (0, 0, None) }
    }

    /// Показать свойства документа `doc` (вызывается средой в простое).
    pub fn attach(&mut self, doc: Option<DocRef>) {
        let key = match &doc {
            Some(d) => {
                let b = d.borrow();
                (Rc::as_ptr(d) as usize, b.rev, b.primary())
            }
            None => (0, 0, None),
        };
        if key == self.shown {
            return;
        }
        let sel_changed = key.0 != self.shown.0 || key.2 != self.shown.2;
        self.shown = key;
        *self.target.borrow_mut() = doc.clone();
        let head = match &doc {
            Some(d) => {
                let b = d.borrow();
                match b.primary() {
                    Some(i) => {
                        let c = &b.form.controls[i];
                        let more = if b.sel.len() > 1 { format!(" (+{})", b.sel.len() - 1) } else { String::new() };
                        format!("{}: {}{}", c.name, c.kind.title(), more)
                    }
                    None => format!("Форма {}", b.form.name),
                }
            }
            None => "Нет формы".into(),
        };
        if let Some(h) = self.win.group.find_mut::<StaticText>(self.head_id) {
            h.text = head;
        }
        if let Some(t) = self.win.group.find_mut::<Table>(self.table_id) {
            t.refresh();
            if sel_changed {
                t.set_cursor(0, 1);
            }
        }
        self.update_hint();
    }

    fn current_prop(&self) -> Option<Prop> {
        let t = self.win.group.find::<Table>(self.table_id)?;
        let row = t.cursor().0;
        let doc = self.target.borrow().clone()?;
        let d = doc.borrow();
        props::props_for(&d.form, d.primary()).get(row).copied()
    }

    fn update_hint(&mut self) {
        let hint = self.current_prop().map(|p| p.hint().to_string()).unwrap_or_default();
        if let Some(h) = self.win.group.find_mut::<StaticText>(self.hint_id) {
            if h.text != hint {
                h.text = hint;
            }
        }
    }

    /// Пробел на логическом свойстве (или классе формы) — переключить значение.
    fn toggle_bool(&mut self) -> bool {
        let Some(p) = self.current_prop() else { return false };
        // Класс формы (Dialog ↔ Window) тоже переключается пробелом.
        if !p.is_bool() && p != Prop::FormClass {
            return false;
        }
        let Some(doc) = self.target.borrow().clone() else { return false };
        let mut d = doc.borrow_mut();
        let sel = d.primary();
        let now = props::get(&d.form, sel, p);
        let next = if p == Prop::FormClass {
            if now == "Dialog" {
                "Window"
            } else {
                "Dialog"
            }
        } else if props::parse_bool(&now).unwrap_or(false) {
            "нет"
        } else {
            "да"
        };
        let before = (d.form.clone(), d.sel.clone());
        if props::set(&mut d.form, sel, p, next).is_ok() && d.form != before.0 {
            d.push_undo(before);
            d.changed();
        }
        true
    }
}

impl View for InspectorWindow {
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
        if let Event::KeyDown(k) = ev {
            let editing = self.win.group.find::<Table>(self.table_id).map(|t| t.is_editing()).unwrap_or(false);
            let table_focused = self.win.group.current_view().map(|v| v.vs().id) == Some(self.table_id);
            if table_focused && !editing && k.mods == Mods::NONE {
                if k.key == Key::Enter {
                    // Enter начинает правку значения (как F2).
                    *ev = Event::KeyDown(kb::f(2));
                    if let Some(t) = self.win.group.find_mut::<Table>(self.table_id) {
                        let r = t.cursor().0;
                        t.set_cursor(r, 1);
                    }
                } else if k.key == Key::Char(' ') && self.toggle_bool() {
                    ev.clear();
                    return;
                }
            }
        }
        self.win.handle_window_event(ev, ctx);
        self.update_hint();
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.win.group.valid(command, ctx)
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        (Point::new(20, 8), owner_size)
    }
}
