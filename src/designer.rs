//! Визуальный дизайнер форм.
//!
//! Форма редактируется «как есть»: окно дизайнера — это окно-диалог с
//! той же рамкой, заголовком, цветом и размером, что и будущий диалог, а
//! внутри рисуются настоящие виджеты tvision-rs (их строит `builder`).
//! Перетаскивание окна и его угла меняет положение и размер формы.
//!
//! Документ формы ([`FormDoc`]) общий у дизайнера и инспектора свойств
//! (`Rc<RefCell<…>>`): инспектор правит тот же объект, а дизайнер
//! перерисовывается по номеру ревизии.
//!
//! Мышь: щелчок — выбрать элемент, Ctrl+щелчок — добавить к выделению,
//! протяжка — переместить, протяжка за правый нижний угол — размер,
//! протяжка по пустому месту — выделить рамкой, двойной щелчок —
//! свойства, правая кнопка — контекстное меню.
//!
//! Клавиши: стрелки — сдвиг, Shift+стрелки — размер, Tab/Shift+Tab —
//! следующий/предыдущий элемент, Enter — свойства, Ins — добавить,
//! Del — удалить, Esc — снять выделение (свойства формы), Ctrl+C/X/V —
//! буфер обмена, Ctrl+Z/Ctrl+Y — отмена/повтор, Ctrl+A — всё, Ctrl+D —
//! дублировать, Ctrl+PgUp/PgDn — порядок (он же порядок Tab), F2 — сохранить.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use tvision::color::attr;
use tvision::event::MB_RIGHT;
use tvision::view::default_handle_event;
use tvision::*;

use crate::builder::build_dialog;
use crate::cmds::*;
use crate::dialogs::build_message_box;
use crate::json::{self, Json};
use crate::model::{build_order, Control, Form, Kind};

/// Общий документ формы.
pub type DocRef = Rc<RefCell<FormDoc>>;

/// Заголовок данных элементов в буфере обмена.
const CLIP_MARK: &str = "TVSTUDIO-CONTROLS";

/// Способ выравнивания.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Top,
    Bottom,
    CenterH,
    CenterV,
    SameWidth,
    SameHeight,
}

impl Align {
    pub const ALL: [Align; 8] =
        [Align::Left, Align::Right, Align::Top, Align::Bottom, Align::CenterH, Align::CenterV, Align::SameWidth, Align::SameHeight];

    pub fn title(self) -> &'static str {
        match self {
            Align::Left => "По ~л~евому краю",
            Align::Right => "По ~п~равому краю",
            Align::Top => "По ~в~ерхнему краю",
            Align::Bottom => "По ~н~ижнему краю",
            Align::CenterH => "По центру по ~г~оризонтали",
            Align::CenterV => "По центру по в~е~ртикали",
            Align::SameWidth => "Одинаковая ~ш~ирина",
            Align::SameHeight => "Одинаковая в~ы~сота",
        }
    }
}

/// Документ формы: сама форма, выделение, отмена, признак изменения.
pub struct FormDoc {
    pub form: Form,
    /// Выделенные элементы; первый — «основной» (его свойства в инспекторе).
    pub sel: Vec<usize>,
    undo: Vec<(Form, Vec<usize>)>,
    redo: Vec<(Form, Vec<usize>)>,
    pub modified: bool,
    /// Файл формы.
    pub path: PathBuf,
    /// Ревизия: растёт при любом изменении (формы или выделения).
    pub rev: u64,
    /// Ревизия самой формы (для кэша виджетов).
    pub form_rev: u64,
    /// Показывать сетку.
    pub grid: bool,
    /// Показывать номера порядка Tab.
    pub show_order: bool,
    /// Куда ставить следующий новый элемент.
    pub ins: Point,
    /// Ошибка, которую среда покажет пользователю.
    pub error: Option<String>,
}

impl FormDoc {
    pub fn new(form: Form, path: PathBuf) -> DocRef {
        Rc::new(RefCell::new(FormDoc {
            form,
            sel: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            modified: false,
            path,
            rev: 1,
            form_rev: 1,
            grid: true,
            show_order: false,
            ins: Point::new(2, 2),
            error: None,
        }))
    }

    /// Основной выделенный элемент.
    pub fn primary(&self) -> Option<usize> {
        self.sel.first().copied().filter(|&i| i < self.form.controls.len())
    }

    /// Сменилось выделение.
    pub fn touch(&mut self) {
        self.rev += 1;
    }

    /// Изменилась форма.
    pub fn changed(&mut self) {
        self.rev += 1;
        self.form_rev += 1;
        self.modified = true;
    }

    /// Запомнить состояние перед изменением (для отмены).
    pub fn checkpoint(&mut self) {
        let snap = (self.form.clone(), self.sel.clone());
        self.push_undo(snap);
    }

    /// Положить в журнал отмены состояние «до» (если оно отличается).
    pub fn push_undo(&mut self, before: (Form, Vec<usize>)) {
        self.undo.push(before);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Применить изменение формы с записью в журнал отмены.
    pub fn edit(&mut self, f: impl FnOnce(&mut Form, &mut Vec<usize>)) {
        let before = (self.form.clone(), self.sel.clone());
        f(&mut self.form, &mut self.sel);
        self.sel.retain(|&i| i < self.form.controls.len());
        if self.form != before.0 {
            self.push_undo(before);
            self.changed();
        } else if self.sel != before.1 {
            self.touch();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((f, s)) = self.undo.pop() {
            let cur = (std::mem::replace(&mut self.form, f), std::mem::replace(&mut self.sel, s));
            self.redo.push(cur);
            self.changed();
        }
    }

    pub fn redo(&mut self) {
        if let Some((f, s)) = self.redo.pop() {
            let cur = (std::mem::replace(&mut self.form, f), std::mem::replace(&mut self.sel, s));
            self.undo.push(cur);
            self.changed();
        }
    }

    /// Выделить ровно эти элементы.
    pub fn select(&mut self, v: Vec<usize>) {
        if self.sel != v {
            self.sel = v;
            self.touch();
        }
    }

    /// Ограничить позицию элемента размером `(w, h)` внутренностью формы.
    fn clamp_pos(&self, p: Point, w: i32, h: i32) -> Point {
        let f = &self.form;
        Point::new(p.x.clamp(1, (f.w - 1 - w).max(1)), p.y.clamp(1, (f.h - 1 - h).max(1)))
    }

    /// Добавить элемент вида `kind` в точку вставки. Возвращает его номер.
    pub fn insert(&mut self, kind: Kind) -> usize {
        let (w, h) = kind.default_size();
        let (w, h) = (w.min(self.form.w - 2), h.min(self.form.h - 2));
        let mut p = self.clamp_pos(self.ins, w, h);
        if self.overlaps(Rect::new(p.x, p.y, p.x + w, p.y + h)) {
            if let Some(free) = self.free_spot(w, h) {
                p = free;
            }
        }
        let mut c = self.form.new_control(kind, p.x, p.y);
        c.w = c.w.min(self.form.w - 1 - p.x).max(1);
        c.h = c.h.min(self.form.h - 1 - p.y).max(1);
        if kind == Kind::Label {
            // Метка сразу привязывается к выделенному элементу с данными.
            if let Some(t) = self.primary().map(|i| &self.form.controls[i]).filter(|t| t.kind.selectable()) {
                c.link = t.name.clone();
            }
        }
        let i = self.form.controls.len();
        let ch = c.h;
        self.edit(|f, s| {
            f.controls.push(c);
            *s = vec![i];
        });
        // Следующий элемент — ниже; у нижнего края — новая колонка.
        let mut next = Point::new(p.x, p.y + ch);
        if next.y >= self.form.h - 2 {
            next = Point::new(p.x + 16, 2);
        }
        if next.x >= self.form.w - 3 {
            next = Point::new(2, 2);
        }
        self.ins = next;
        i
    }

    /// Накрывает ли прямоугольник какой-нибудь элемент.
    fn overlaps(&self, r: Rect) -> bool {
        self.form.controls.iter().any(|c| !c.rect().intersect(&r).is_empty())
    }

    /// Первое свободное место для элемента `w`×`h` (сверху вниз, слева направо).
    fn free_spot(&self, w: i32, h: i32) -> Option<Point> {
        let f = &self.form;
        for y in 1..=(f.h - 1 - h).max(1) {
            for x in (2..=(f.w - 1 - w).max(2)).step_by(2) {
                if !self.overlaps(Rect::new(x, y, x + w, y + h)) {
                    return Some(Point::new(x, y));
                }
            }
        }
        None
    }

    /// Удалить выделенные элементы.
    pub fn delete_selected(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        let mut del = self.sel.clone();
        del.sort_unstable();
        self.edit(|f, s| {
            for &i in del.iter().rev() {
                let name = f.controls.remove(i).name;
                for c in f.controls.iter_mut() {
                    if c.link == name {
                        c.link.clear();
                    }
                }
            }
            s.clear();
        });
    }

    /// Сдвинуть выделенные элементы (без записи в журнал — это делает вызывающий).
    pub fn move_selected(&mut self, dx: i32, dy: i32) {
        for &i in &self.sel.clone() {
            let (w, h) = (self.form.controls[i].w, self.form.controls[i].h);
            let c = &self.form.controls[i];
            let p = self.clamp_pos(Point::new(c.x + dx, c.y + dy), w, h);
            let c = &mut self.form.controls[i];
            c.x = p.x;
            c.y = p.y;
        }
        self.changed();
    }

    /// Изменить размер выделенных элементов.
    pub fn resize_selected(&mut self, dw: i32, dh: i32) {
        let (fw, fh) = (self.form.w, self.form.h);
        for &i in &self.sel.clone() {
            let c = &mut self.form.controls[i];
            c.w = (c.w + dw).clamp(1, (fw - 1 - c.x).max(1));
            c.h = (c.h + dh).clamp(1, (fh - 1 - c.y).max(1));
        }
        self.changed();
    }

    /// Выделенные элементы в виде текста для буфера обмена.
    pub fn copy_text(&self) -> Option<String> {
        if self.sel.is_empty() {
            return None;
        }
        let mut idx = self.sel.clone();
        idx.sort_unstable();
        let arr = Json::Arr(idx.iter().map(|&i| self.form.controls[i].to_json()).collect());
        Some(format!("{}\n{}", CLIP_MARK, arr.to_pretty()))
    }

    /// Вставить элементы из буфера обмена (со сдвигом и новыми именами).
    pub fn paste_text(&mut self, text: &str) -> bool {
        let body = match text.strip_prefix(CLIP_MARK) {
            Some(b) => b,
            None => return false,
        };
        let arr = match json::parse(body.trim()) {
            Ok(a) => a,
            Err(_) => return false,
        };
        let mut ctrls: Vec<Control> = arr.as_arr().iter().filter_map(|j| Control::from_json(j).ok()).collect();
        if ctrls.is_empty() {
            return false;
        }
        self.edit(|f, s| {
            let mut renamed: Vec<(String, String)> = Vec::new();
            let start = f.controls.len();
            for c in ctrls.iter_mut() {
                if f.find(&c.name).is_some() {
                    let base: String = c.name.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string();
                    let nn = f.unique_name(if base.is_empty() { c.kind.prefix() } else { &base });
                    renamed.push((c.name.clone(), nn.clone()));
                    c.name = nn;
                }
                c.x = (c.x + 1).clamp(1, (f.w - 1 - c.w).max(1));
                c.y = (c.y + 1).clamp(1, (f.h - 1 - c.h).max(1));
                f.controls.push(c.clone());
            }
            // Метки внутри вставленного набора — на переименованные элементы.
            for c in f.controls[start..].iter_mut() {
                if let Some((_, nn)) = renamed.iter().find(|(o, _)| *o == c.link) {
                    c.link = nn.clone();
                }
            }
            *s = (start..f.controls.len()).collect();
        });
        true
    }

    /// Выровнять выделенные элементы по основному (или по форме для «центра»).
    pub fn align(&mut self, how: Align) {
        let Some(p) = self.primary() else { return };
        let r = self.form.controls[p].clone();
        let sel = self.sel.clone();
        self.edit(|f, _| {
            let (fw, fh) = (f.w, f.h);
            for &i in &sel {
                let c = &mut f.controls[i];
                match how {
                    Align::Left => c.x = r.x,
                    Align::Right => c.x = r.x + r.w - c.w,
                    Align::Top => c.y = r.y,
                    Align::Bottom => c.y = r.y + r.h - c.h,
                    Align::CenterH => c.x = (fw - c.w) / 2,
                    Align::CenterV => c.y = (fh - c.h) / 2,
                    Align::SameWidth => c.w = r.w,
                    Align::SameHeight => c.h = r.h,
                }
                c.x = c.x.max(1);
                c.y = c.y.max(1);
            }
        });
    }

    /// Изменить порядок основного элемента: `delta` = ±1 или в начало/конец.
    pub fn reorder(&mut self, to: Reorder) {
        let Some(p) = self.primary() else { return };
        let n = self.form.controls.len();
        let np = match to {
            Reorder::Forward => (p + 1).min(n - 1),
            Reorder::Backward => p.saturating_sub(1),
            Reorder::Front => n - 1,
            Reorder::Back => 0,
        };
        if np == p {
            return;
        }
        self.edit(|f, s| {
            let c = f.controls.remove(p);
            f.controls.insert(np, c);
            *s = vec![np];
        });
    }

    /// Дублировать выделенное.
    pub fn duplicate(&mut self) {
        if let Some(t) = self.copy_text() {
            self.paste_text(&t);
        }
    }

    /// Выбрать следующий/предыдущий элемент.
    pub fn select_next(&mut self, forward: bool) {
        let n = self.form.controls.len();
        if n == 0 {
            return;
        }
        let i = match (self.primary(), forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        self.select(vec![i]);
    }

    /// Записать форму в файл.
    pub fn save(&mut self) -> Result<(), String> {
        self.form.save(&self.path)?;
        self.modified = false;
        self.touch();
        Ok(())
    }
}

/// Куда переместить элемент в порядке Tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reorder {
    Forward,
    Backward,
    Front,
    Back,
}

// ======================================================================
// Холст: внутренность окна дизайнера
// ======================================================================

/// Состояние перетаскивания мышью.
enum Drag {
    /// Перемещение: точка начала и форма до начала.
    Move { start: Point, before: (Form, Vec<usize>), orig: Vec<(usize, Point)> },
    /// Изменение размера основного элемента.
    Resize { start: Point, before: (Form, Vec<usize>), orig: Vec<(usize, Point)> },
    /// Выделение рамкой.
    Band { start: Point, cur: Point, add: bool },
}

/// Метки модальных окон холста.
const TAG_SAVE_ASK: u16 = 1;

/// Холст дизайнера — занимает всю внутренность окна. Координаты мыши,
/// которые приходят в холст (в системе координат окна), совпадают с
/// координатами диалога: строка 0 окна — рамка, как и у диалога.
pub struct Canvas {
    pub vs: ViewState,
    pub doc: DocRef,
    /// id окна дизайнера (чтобы закрыть именно его после вопроса о сохранении).
    pub win_id: ViewId,
    drag: Option<Drag>,
    /// Последняя правка клавишами: `Some(false)` — сдвиг, `Some(true)` —
    /// размер. Подряд идущие одинаковые правки объединяются в одну отмену.
    key_op: Option<bool>,
    /// Построенный диалог для отрисовки (ключ — ревизия формы).
    cache: RefCell<Option<(u64, Dialog)>>,
    /// Команда, которую повторить после ответа «сохранить?».
    pending: Option<u16>,
}

impl Canvas {
    pub fn new(bounds: Rect, doc: DocRef, win_id: ViewId) -> Self {
        let mut vs = ViewState::new(bounds);
        vs.options |= of::SELECTABLE | of::FIRST_CLICK;
        vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        vs.help_ctx = HC_DESIGNER;
        Canvas { vs, doc, win_id, drag: None, key_op: None, cache: RefCell::new(None), pending: None }
    }

    /// Точка мыши (координаты окна) → координаты формы. Они совпадают.
    fn form_point(&self, p: Point) -> Point {
        p
    }

    /// Контекстное меню холста.
    fn context_menu() -> Menu {
        let mut add = Menu::new();
        for (i, k) in Kind::ALL.iter().enumerate() {
            add = add.item(k.title(), CM_INS_BASE + i as u16);
        }
        let mut al = Menu::new();
        for (i, a) in Align::ALL.iter().enumerate() {
            al = al.item(a.title(), CM_ALIGN_BASE + i as u16);
        }
        Menu::new()
            .item_key("~С~войства...", CM_DS_PROPS, kb::ENTER, "Enter")
            .submenu("~Д~обавить", add)
            .separator()
            .item_key("~В~ырезать", cm::CUT, kb::ctrl('X'), "Ctrl+X")
            .item_key("~К~опировать", cm::COPY, kb::ctrl('C'), "Ctrl+C")
            .item_key("Вст~а~вить", cm::PASTE, kb::ctrl('V'), "Ctrl+V")
            .item_key("Д~у~блировать", CM_DS_DUP, kb::ctrl('D'), "Ctrl+D")
            .item_key("~У~далить", cm::CLEAR, kb::key(Key::Del), "Del")
            .separator()
            .submenu("В~ы~ровнять", al)
            .item_key("Раньше в порядке Tab", CM_DS_BACKWARD, KeyEvent::new(Key::PgUp, Mods::CTRL), "Ctrl+PgUp")
            .item_key("Позже в порядке Tab", CM_DS_FORWARD, KeyEvent::new(Key::PgDn, Mods::CTRL), "Ctrl+PgDn")
            .separator()
            .item_key("~П~роверить форму", CM_FORM_TEST, kb::ctrl_f(9), "Ctrl+F9")
            .item_key("Ко~д~ формы", CM_FORM_CODE, kb::f(12), "F12")
    }

    /// Показать контекстное меню в точке `local` (координаты окна).
    fn popup(&self, local: Point, ctx: &mut Ctx) {
        let id = self.vs.id;
        let menu = Rc::new(Self::context_menu());
        ctx.with_program(move |app| {
            let o = app.group.owner_origin_of(id).unwrap_or_default();
            app.ctx.popup_menu(menu, o + local);
        });
    }

    fn handle_mouse(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        let id = self.vs.id;
        match ev.clone() {
            Event::MouseDown(m) => {
                let p = self.form_point(m.pos);
                let mut doc = self.doc.borrow_mut();
                let hit = doc.form.hit(p.x, p.y);
                self.key_op = None;
                if m.buttons & MB_RIGHT != 0 {
                    match hit {
                        Some(i) if !doc.sel.contains(&i) => doc.select(vec![i]),
                        None => {
                            doc.ins = p;
                        }
                        _ => {}
                    }
                    drop(doc);
                    self.popup(m.pos, ctx);
                    ev.clear();
                    return;
                }
                if let (true, Some(h)) = (m.double, hit) {
                    doc.select(vec![h]);
                    ctx.post_command(CM_DS_PROPS, Info::None);
                    ev.clear();
                    return;
                }
                match hit {
                    Some(i) => {
                        if m.mods.ctrl {
                            let mut s = doc.sel.clone();
                            if let Some(k) = s.iter().position(|&x| x == i) {
                                s.remove(k);
                            } else {
                                s.insert(0, i);
                            }
                            doc.select(s);
                            ev.clear();
                            return;
                        }
                        if !doc.sel.contains(&i) {
                            doc.select(vec![i]);
                        } else if doc.sel[0] != i {
                            // Щелчок по выделенному делает его основным.
                            let mut s = doc.sel.clone();
                            s.retain(|&x| x != i);
                            s.insert(0, i);
                            doc.select(s);
                        }
                        let c = &doc.form.controls[i];
                        let corner = Point::new(c.x + c.w - 1, c.y + c.h - 1);
                        let before = (doc.form.clone(), doc.sel.clone());
                        let orig: Vec<(usize, Point)> =
                            doc.sel.iter().map(|&k| (k, Point::new(doc.form.controls[k].x, doc.form.controls[k].y))).collect();
                        let sizes: Vec<(usize, Point)> =
                            doc.sel.iter().map(|&k| (k, Point::new(doc.form.controls[k].w, doc.form.controls[k].h))).collect();
                        self.drag = Some(if p == corner && (c.w > 1 || c.h > 1) {
                            Drag::Resize { start: p, before, orig: sizes }
                        } else {
                            Drag::Move { start: p, before, orig }
                        });
                    }
                    None => {
                        if !m.mods.ctrl {
                            doc.select(Vec::new());
                        }
                        doc.ins = p;
                        doc.touch();
                        self.drag = Some(Drag::Band { start: p, cur: p, add: m.mods.ctrl });
                    }
                }
                ctx.grab_mouse(id);
                ev.clear();
            }
            Event::MouseMove(m) | Event::MouseAuto(m) if ctx.mouse_grabbed_by(id) => {
                let p = self.form_point(m.pos);
                let mut doc = self.doc.borrow_mut();
                match &mut self.drag {
                    Some(Drag::Move { start, orig, .. }) => {
                        let d = p - *start;
                        let (fw, fh) = (doc.form.w, doc.form.h);
                        for &(i, o) in orig.iter() {
                            let c = &mut doc.form.controls[i];
                            let nx = (o.x + d.x).clamp(1, (fw - 1 - c.w).max(1));
                            let ny = (o.y + d.y).clamp(1, (fh - 1 - c.h).max(1));
                            if nx != c.x || ny != c.y {
                                c.x = nx;
                                c.y = ny;
                            }
                        }
                        doc.changed();
                    }
                    Some(Drag::Resize { start, orig, .. }) => {
                        let d = p - *start;
                        let (fw, fh) = (doc.form.w, doc.form.h);
                        for &(i, o) in orig.iter() {
                            let c = &mut doc.form.controls[i];
                            c.w = (o.x + d.x).clamp(1, (fw - 1 - c.x).max(1));
                            c.h = (o.y + d.y).clamp(1, (fh - 1 - c.y).max(1));
                        }
                        doc.changed();
                    }
                    Some(Drag::Band { cur, .. }) => {
                        *cur = p;
                        doc.touch();
                    }
                    None => {}
                }
                ev.clear();
            }
            Event::MouseUp(_) if ctx.mouse_grabbed_by(id) => {
                ctx.release_mouse();
                let mut doc = self.doc.borrow_mut();
                match self.drag.take() {
                    Some(Drag::Move { before, .. }) | Some(Drag::Resize { before, .. }) => {
                        if doc.form != before.0 {
                            doc.push_undo(before);
                            doc.changed();
                        }
                    }
                    Some(Drag::Band { start, cur, add }) => {
                        let r = Rect::new(start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x) + 1, start.y.max(cur.y) + 1);
                        let mut s: Vec<usize> = if add { doc.sel.clone() } else { Vec::new() };
                        if r.width() > 1 || r.height() > 1 {
                            for (i, c) in doc.form.controls.iter().enumerate() {
                                if !c.rect().intersect(&r).is_empty() && !s.contains(&i) {
                                    s.push(i);
                                }
                            }
                        }
                        doc.select(s);
                        doc.touch();
                    }
                    None => {}
                }
                ev.clear();
            }
            Event::MouseWheel(_) => ev.clear(),
            _ => {}
        }
    }

    /// Клавиши. Возвращает `true`, если клавиша обработана.
    fn handle_key(&mut self, k: KeyEvent, ctx: &mut Ctx) -> bool {
        let m = k.mods;
        let arrow = match k.key {
            Key::Left => Some((-1, 0)),
            Key::Right => Some((1, 0)),
            Key::Up => Some((0, -1)),
            Key::Down => Some((0, 1)),
            _ => None,
        };
        let mut doc = self.doc.borrow_mut();
        if let Some((dx, dy)) = arrow {
            if m.alt {
                return false;
            }
            if doc.sel.is_empty() {
                return true;
            }
            if self.key_op != Some(m.shift) {
                doc.checkpoint();
                self.key_op = Some(m.shift);
            }
            let step = if m.ctrl { 4 } else { 1 };
            if m.shift {
                doc.resize_selected(dx * step, dy * step);
            } else {
                doc.move_selected(dx * step, dy * step);
            }
            return true;
        }
        self.key_op = None;
        match (k.key, m.ctrl, m.alt, m.shift) {
            (Key::Tab, false, false, s) => doc.select_next(!s),
            (Key::BackTab, false, false, _) => doc.select_next(false),
            (Key::Del, false, false, false) => doc.delete_selected(),
            (Key::Del, false, false, true) => {
                drop(doc);
                self.command(cm::CUT, ctx);
            }
            (Key::Del, true, false, _) => doc.delete_selected(),
            (Key::Ins, false, false, false) => {
                // Меню «Добавить» рядом с выделением (или с точкой вставки).
                let p = doc.primary().map(|i| Point::new(doc.form.controls[i].x, doc.form.controls[i].y + 1)).unwrap_or(doc.ins);
                drop(doc);
                let mut add = Menu::new();
                for (i, kd) in Kind::ALL.iter().enumerate() {
                    add = add.item(kd.title(), CM_INS_BASE + i as u16);
                }
                let id = self.vs.id;
                let menu = Rc::new(add);
                ctx.with_program(move |app| {
                    let o = app.group.owner_origin_of(id).unwrap_or_default();
                    app.ctx.popup_menu(menu, o + p);
                });
            }
            (Key::Ins, true, false, _) => {
                drop(doc);
                self.command(cm::COPY, ctx);
            }
            (Key::Ins, false, false, true) => {
                drop(doc);
                self.command(cm::PASTE, ctx);
            }
            (Key::Enter, false, false, _) => ctx.post_command(CM_DS_PROPS, Info::None),
            (Key::Esc, false, false, _) => doc.select(Vec::new()),
            (Key::PgUp, true, false, _) => doc.reorder(Reorder::Backward),
            (Key::PgDn, true, false, _) => doc.reorder(Reorder::Forward),
            (Key::Home, true, false, _) => doc.reorder(Reorder::Back),
            (Key::End, true, false, _) => doc.reorder(Reorder::Front),
            (Key::Char(c), true, false, _) => {
                let cmd = match c.to_ascii_uppercase() {
                    'C' => cm::COPY,
                    'X' => cm::CUT,
                    'V' => cm::PASTE,
                    'Z' | 'U' => cm::UNDO,
                    'Y' => cm::REDO,
                    'A' => cm::SELECT_ALL,
                    'D' => CM_DS_DUP,
                    _ => return false,
                };
                drop(doc);
                return self.command(cmd, ctx);
            }
            _ => return false,
        }
        true
    }

    /// Команды правки. Возвращает `true`, если команда обработана.
    fn command(&mut self, c: u16, ctx: &mut Ctx) -> bool {
        let mut doc = self.doc.borrow_mut();
        match c {
            cm::UNDO => doc.undo(),
            cm::REDO => doc.redo(),
            cm::COPY => {
                if let Some(t) = doc.copy_text() {
                    ctx.clipboard.set(&t);
                }
            }
            cm::CUT => {
                if let Some(t) = doc.copy_text() {
                    ctx.clipboard.set(&t);
                    doc.delete_selected();
                }
            }
            cm::PASTE => {
                let t = ctx.clipboard.get();
                if !doc.paste_text(&t) {
                    doc.error = Some("В буфере обмена нет элементов формы.".into());
                }
            }
            cm::CLEAR => doc.delete_selected(),
            cm::SELECT_ALL => {
                let all = (0..doc.form.controls.len()).collect();
                doc.select(all);
            }
            CM_DS_DUP => doc.duplicate(),
            cm::SAVE => {
                if let Err(e) = doc.save() {
                    doc.error = Some(e);
                }
            }
            _ => return false,
        }
        true
    }
}

impl View for Canvas {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let doc = self.doc.borrow();
        let f = &doc.form;
        let bg = dc.color(1);
        dc.fill(self.vs.extent(), ' ', bg);
        // Сетка: точка в каждой второй клетке чётных строк.
        if doc.grid {
            let dot = tvision::color::with_fore(bg, 8);
            for y in 0..self.vs.size.y {
                for x in 0..self.vs.size.x {
                    if (x + 1) % 2 == 0 && (y + 1) % 2 == 0 {
                        dc.write_char(x, y, '·', dot, 1);
                    }
                }
            }
        }
        // Настоящие виджеты (диалог строится один раз на ревизию формы).
        {
            let mut cache = self.cache.borrow_mut();
            if cache.as_ref().map(|(r, _)| *r) != Some(doc.form_rev) {
                *cache = Some((doc.form_rev, build_dialog(f, &|_| 1000).0));
            }
            let dlg = &cache.as_ref().unwrap().1;
            let mut gdc = dc.sub(Rect::new(-1, -1, f.w - 1, f.h - 1), &[]);
            dlg.win.group.draw_children(&mut gdc);
        }
        // Выделение: перекрашиваем элементы, у основного — «уголок» размера.
        let (sel1, sel2) = if f.palette == crate::model::Pal::Cyan { (attr(15, 2), attr(0, 2)) } else { (attr(15, 3), attr(0, 3)) };
        for (n, &i) in doc.sel.iter().enumerate() {
            if let Some(c) = f.controls.get(i) {
                let r = c.rect().moved(-1, -1);
                dc.recolor(r, if n == 0 { sel1 } else { sel2 });
                if n == 0 && (c.w > 1 || c.h > 1) {
                    dc.write_char(r.b.x - 1, r.b.y - 1, '┘', attr(14, 4), 1);
                }
            }
        }
        // Номера порядка Tab.
        if doc.show_order {
            let mut n = 0;
            for i in build_order(f) {
                let c = &f.controls[i];
                if c.kind.selectable() {
                    n += 1;
                    dc.write_str(c.x - 1, c.y - 1, &n.to_string(), attr(14, 4));
                }
            }
        }
        // Рамка выделения.
        if let Some(Drag::Band { start, cur, .. }) = &self.drag {
            let r = Rect::new(start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x) + 1, start.y.max(cur.y) + 1).moved(-1, -1);
            dc.recolor(r, attr(15, 1));
        }
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        default_handle_event(&self.vs, ev, ctx);
        if ev.is_mouse() {
            self.handle_mouse(ev, ctx);
            return;
        }
        match ev.clone() {
            Event::KeyDown(k) if self.vs.is_focused() => {
                if self.handle_key(k, ctx) {
                    ev.clear();
                }
            }
            Event::Command(m)
                if self.vs.is_focused()
                    && matches!(
                        m.command,
                        cm::UNDO | cm::REDO | cm::COPY | cm::CUT | cm::PASTE | cm::CLEAR | cm::SELECT_ALL | CM_DS_DUP | cm::SAVE
                    )
                    && self.command(m.command, ctx) =>
            {
                ev.clear()
            }
            _ => {}
        }
    }

    fn update_commands(&self, ctx: &mut Ctx) {
        if !self.vs.is_focused() {
            return;
        }
        let doc = self.doc.borrow();
        let sel = !doc.sel.is_empty();
        ctx.set_command(cm::UNDO, doc.can_undo());
        ctx.set_command(cm::REDO, doc.can_redo());
        ctx.set_command(cm::CUT, sel);
        ctx.set_command(cm::COPY, sel);
        ctx.set_command(cm::CLEAR, sel);
        ctx.set_command(cm::PASTE, ctx.clipboard.has_text());
        ctx.set_command(cm::SAVE, true);
        ctx.set_command(cm::SAVE_AS, false);
    }

    /// Закрытие изменённой формы — вопрос о сохранении (как у `FileEditor`).
    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        if !matches!(command, cm::CLOSE | cm::QUIT) || !self.doc.borrow().modified {
            return true;
        }
        let name = self.doc.borrow().form.name.clone();
        let d = build_message_box(
            Rect::new(0, 0, 50, 9),
            &format!("\x03Форма «{}» изменена.\n\x03Сохранить изменения?", name),
            mf::INFORMATION | mf::YES_NO_CANCEL,
        );
        self.pending = Some(command);
        ctx.exec(Box::new(d), Some(self.vs.id), TAG_SAVE_ASK);
        false
    }

    fn exec_result(&mut self, tag: u16, command: u16, _view: Box<dyn View>, ctx: &mut Ctx) {
        if tag != TAG_SAVE_ASK {
            return;
        }
        let pending = self.pending.take();
        let go = match command {
            cm::YES => {
                let r = self.doc.borrow_mut().save();
                match r {
                    Ok(()) => true,
                    Err(e) => {
                        self.doc.borrow_mut().error = Some(e);
                        false
                    }
                }
            }
            cm::NO => {
                self.doc.borrow_mut().modified = false;
                true
            }
            _ => false,
        };
        if go {
            match pending {
                Some(cm::CLOSE) => ctx.post_command(cm::CLOSE, Info::Id(self.win_id)),
                Some(c) => ctx.post_command(c, Info::None),
                None => {}
            }
        }
    }
}

// ======================================================================
// Окно дизайнера
// ======================================================================

/// Окно дизайнера формы: рамка = рамка будущего диалога.
pub struct DesignWindow {
    pub win: Window,
    pub doc: DocRef,
    pub canvas_id: ViewId,
}

impl DesignWindow {
    pub fn new(doc: DocRef) -> Self {
        let f = doc.borrow().form.clone();
        let mut win = Window::new(Rect::new(f.x, f.y, f.x + f.w, f.y + f.h), &f.title, 0);
        win.flags = wf::MOVE | wf::GROW | wf::CLOSE;
        win.palette = f.palette.palette();
        win.group.vs.grow_mode = 0;
        let wid = win.id();
        let canvas = Canvas::new(win.interior(), doc.clone(), wid);
        let canvas_id = win.add(canvas);
        DesignWindow { win, doc, canvas_id }
    }

    /// Привести окно к форме (после правки в инспекторе или отмены).
    pub fn sync_from_doc(&mut self) {
        let doc = self.doc.borrow();
        let f = &doc.form;
        let r = Rect::new(f.x, f.y, f.x + f.w, f.y + f.h);
        if self.win.group.vs.bounds() != r {
            self.win.group.change_bounds(r);
        }
        if self.win.title != f.title {
            self.win.title = f.title.clone();
        }
        self.win.palette = f.palette.palette();
        // Номер окна-формы виден в рамке дизайнера (как будет в программе).
        let n = if f.class == crate::model::FormClass::Window { f.number } else { 0 };
        if self.win.number != n {
            self.win.number = n;
        }
    }

    /// Перенести в форму положение и размер окна (после перетаскивания).
    fn sync_to_doc(&mut self) {
        let b = self.win.group.vs.bounds();
        let mut doc = self.doc.borrow_mut();
        let f = &doc.form;
        if b != Rect::new(f.x, f.y, f.x + f.w, f.y + f.h) {
            let f = &mut doc.form;
            f.x = b.a.x;
            f.y = b.a.y;
            f.w = b.width();
            f.h = b.height();
            doc.changed();
        }
    }
}

impl View for DesignWindow {
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
        self.sync_to_doc();
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.win.group.valid(command, ctx)
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        (Point::new(10, 4), Point::new(owner_size.x.max(250), owner_size.y.max(100)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::about_form;

    #[test]
    fn doc_edit_undo_paste() {
        let d = FormDoc::new(about_form("x"), PathBuf::from("/tmp/x.tvf"));
        let mut doc = d.borrow_mut();
        let i = doc.insert(Kind::InputLine);
        assert_eq!(doc.sel, vec![i]);
        assert!(doc.modified);
        let t = doc.copy_text().unwrap();
        assert!(doc.paste_text(&t));
        assert_eq!(doc.form.controls.len(), 4);
        assert_eq!(doc.form.controls[3].name, "input2");
        doc.undo();
        assert_eq!(doc.form.controls.len(), 3);
        doc.redo();
        assert_eq!(doc.form.controls.len(), 4);
        doc.select(vec![0, 1]);
        doc.align(Align::Left);
        assert_eq!(doc.form.controls[1].x, doc.form.controls[0].x);
        doc.select(vec![0]);
        doc.reorder(Reorder::Front);
        assert_eq!(doc.form.controls[3].name, "info");
        doc.select(vec![3]);
        doc.delete_selected();
        assert!(doc.form.find("info").is_none());
    }
}
