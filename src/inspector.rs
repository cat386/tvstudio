//! Окно «Инспектор» справа от рабочего стола: сверху — компоненты
//! (Enter или двойной щелчок добавляет элемент на форму в верхнем окне
//! дизайнера), под ними — таблица «свойство — значение» для выделенного
//! элемента формы (или самой формы, если ничего не выделено). Tab
//! переходит между компонентами и свойствами.
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
use crate::model::Kind;
use crate::props::{self, Prop};
use crate::toolwins::{enter_to_command, icon, Relay};

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

/// Окно инспектора: компоненты и свойства.
pub struct InspectorWindow {
    pub win: Window,
    /// Список компонентов (верхняя часть).
    pub list_id: ViewId,
    pub table_id: ViewId,
    head_id: ViewId,
    hint_id: ViewId,
    target: Rc<RefCell<Option<DocRef>>>,
    /// Что показано сейчас: (адрес документа, ревизия, основной элемент).
    shown: (usize, u64, Option<usize>),
    pending_choice: Option<ChoiceEdit>,
}

struct ChoiceEdit {
    doc: DocRef,
    selection: Option<usize>,
    prop: Prop,
    values: Vec<String>,
    list: ViewId,
}

const CHOOSE_PROPERTY: u16 = 1362;

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
        // Компоненты: заголовок и список фиксированной высоты (все виды, если
        // помещаются, но не больше ~40% окна — остальное свойствам).
        let lh = Self::palette_rows(s.y);
        let mut ph = StaticText::new(Rect::new(1, 1, s.x - 1, 2), &format!("Компоненты ({})", Kind::ALL.len()));
        ph.vs.grow_mode = gf::GROW_HI_X;
        win.add(ph);
        let palette_scroll = win.add(ScrollBar::new(Rect::new(s.x - 1, 2, s.x, 2 + lh)));
        win.group.view_mut(palette_scroll).unwrap().vs_mut().grow_mode = gf::GROW_LO_X | gf::GROW_HI_X;
        let mut list = ListBox::new(Rect::new(1, 2, s.x - 1, 2 + lh), 1, Some(palette_scroll));
        list.vs.grow_mode = gf::GROW_HI_X;
        list.vs.help_ctx = HC_PALETTE;
        list.set_items(Kind::ALL.iter().map(|k| format!("{} {}", icon(*k), k.title())).collect());
        let list_id = win.add(list);
        win.add(Relay::new(list_id, CM_PAL_INSERT));
        // Свойства: строка «что выделено», таблица, подсказка свойства.
        let top = 2 + lh;
        let mut head = StaticText::new(Rect::new(1, top, s.x - 1, top + 1), "Нет формы");
        head.vs.grow_mode = gf::GROW_HI_X;
        let head_id = win.add(head);
        let vsb = win.add(ScrollBar::new(Rect::new(s.x - 1, top + 1, s.x, s.y - 1)));
        let target: Rc<RefCell<Option<DocRef>>> = Rc::new(RefCell::new(None));
        let model = PropModel { target: target.clone() };
        let w = (s.x - 2).max(10);
        let mut table = Table::new(Rect::new(1, top + 1, s.x - 1, s.y - 2), Box::new(model), Some(vsb), None)
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
        win.select(table_id);
        InspectorWindow { win, list_id, table_id, head_id, hint_id, target, shown: (0, 0, None), pending_choice: None }
    }

    /// Высота списка компонентов для окна высотой `h`.
    fn palette_rows(h: i32) -> i32 {
        let all = Kind::ALL.len() as i32;
        ((h - 2) * 2 / 5).clamp(3, all)
    }

    /// Выбранный в списке компонентов вид элемента.
    pub fn selected_kind(&self) -> Kind {
        let i = self.win.group.find::<ListBox>(self.list_id).map(|l| l.focused).unwrap_or(0);
        Kind::ALL[i.min(Kind::ALL.len() - 1)]
    }

    /// Перевести фокус на компоненты (`true`) или на свойства.
    pub fn focus_part(&mut self, components: bool) {
        self.win.select(if components { self.list_id } else { self.table_id });
    }

    /// В фокусе список компонентов.
    pub fn components_focused(&self) -> bool {
        self.win.group.current_view().map(|v| v.vs().id) == Some(self.list_id)
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
        if !p.is_bool() && !matches!(p, Prop::FormClass | Prop::FormPalette) {
            return false;
        }
        let Some(doc) = self.target.borrow().clone() else { return false };
        let mut d = doc.borrow_mut();
        let sel = d.primary();
        let now = props::get(&d.form, sel, p);
        let choices = props::choices(&d.form, p);
        let next = if !choices.is_empty() {
            choices[(choices.iter().position(|value| *value == now).unwrap_or(0) + 1) % choices.len()].as_str()
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

    fn choose_value(&mut self, ctx: &mut Ctx) -> bool {
        let Some(prop) = self.current_prop() else { return false };
        if prop.is_bool() {
            return self.toggle_bool();
        }
        let Some(doc) = self.target.borrow().clone() else { return false };
        let d = doc.borrow();
        let selection = d.primary();
        let mut values = props::choices(&d.form, prop);
        if values.is_empty() {
            return false;
        }
        let current = props::get(&d.form, selection, prop);
        if !values.contains(&current) {
            values.push(current.clone());
        }
        let rows = (values.len() as i32).clamp(3, 12);
        let mut dialog = Dialog::centered(54, rows + 7, "Выбрать значение свойства");
        let scroll = dialog.add(ScrollBar::new(Rect::new(51, 2, 52, 2 + rows)));
        let mut list = ListBox::new(Rect::new(2, 2, 51, 2 + rows), 1, Some(scroll));
        list.set_items(values.iter().map(|v| if v.is_empty() { "(нет)".into() } else { v.clone() }).collect());
        list.focus_item(values.iter().position(|v| *v == current).unwrap_or(0));
        let list = dialog.add(list);
        dialog.add(Relay::new(list, cm::OK));
        crate::dialogs::action_cancel(&mut dialog, rows + 4, "~В~ыбрать");
        dialog.select(list);
        drop(d);
        self.pending_choice = Some(ChoiceEdit { doc, selection, prop, values, list });
        ctx.exec(Box::new(crate::dialogs::flat(dialog)), Some(self.win.id()), CHOOSE_PROPERTY);
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
        // Enter в списке компонентов — добавить элемент на форму.
        enter_to_command(&self.win, self.list_id, ev, ctx, CM_PAL_INSERT);
        if let Event::KeyDown(k) = ev {
            let editing = self.win.group.find::<Table>(self.table_id).map(|t| t.is_editing()).unwrap_or(false);
            let table_focused = self.win.group.current_view().map(|v| v.vs().id) == Some(self.table_id);
            if table_focused && !editing && k.mods == Mods::NONE {
                if k.key == Key::Enter {
                    if self.choose_value(ctx) {
                        ev.clear();
                        return;
                    }
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

    fn exec_result(&mut self, tag: u16, command: u16, view: Box<dyn View>, _ctx: &mut Ctx) {
        if tag != CHOOSE_PROPERTY {
            return;
        }
        let Some(pending) = self.pending_choice.take() else { return };
        if command != cm::OK {
            return;
        }
        let Some(dialog) = view.as_any().downcast_ref::<Dialog>() else { return };
        let Some(value) = dialog.win.group.find::<ListBox>(pending.list).and_then(|l| pending.values.get(l.focused)) else { return };
        let mut doc = pending.doc.borrow_mut();
        let before = (doc.form.clone(), doc.sel.clone());
        match props::set(&mut doc.form, pending.selection, pending.prop, value) {
            Ok(()) if doc.form != before.0 => {
                doc.push_undo(before);
                doc.changed();
            }
            Err(error) => doc.error = Some(error),
            _ => {}
        }
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        (Point::new(20, 8), owner_size)
    }
}
