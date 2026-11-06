//! Построение «живого» диалога tvision-rs по модели формы.
//!
//! Тот же диалог, что получится в сгенерированном коде (`codegen`), но
//! собранный прямо во время работы среды. Нужен для двух вещей:
//! * дизайнер рисует настоящие виджеты — что видно, то и получится;
//! * «Проверить форму» запускает диалог модально, не выходя из среды.

use std::collections::HashMap;

use tvision::*;

use crate::model::{build_order, parse_tree, Control, Form, Kind, TreeItem};
use crate::props::parse_range;

/// Создать валидатор по описанию из свойства «Проверка».
pub fn make_validator(spec: &str) -> Option<Box<dyn Validator>> {
    let (kind, arg) = spec.split_once(':')?;
    match kind.trim() {
        "picture" => Some(Box::new(PictureValidator::new(arg, true))),
        "filter" => Some(Box::new(FilterValidator::new(arg))),
        "range" => parse_range(arg).ok().map(|(a, b)| Box::new(RangeValidator::new(a, b)) as Box<dyn Validator>),
        "lookup" => {
            let items: Vec<&str> = arg.split('|').map(|s| s.trim()).collect();
            Some(Box::new(StringLookupValidator::new(&items)))
        }
        _ => None,
    }
}

/// Ширина строки ввода с учётом значка истории / кнопки списка справа.
pub fn input_width(c: &Control) -> i32 {
    let extra = match c.kind {
        Kind::ComboBox => 3,
        Kind::InputLine if c.history > 0 => 3,
        _ => 0,
    };
    (c.w - extra).max(1)
}

/// Своя вертикальная полоса прокрутки — в последнем столбце элемента.
/// Возвращает границы самого элемента и id полосы.
fn with_scroll(d: &mut Dialog, c: &Control) -> (Rect, Option<ViewId>) {
    if c.scrollbar && c.w > 1 {
        let sb = d.add(ScrollBar::new(Rect::new(c.x + c.w - 1, c.y, c.x + c.w, c.y + c.h)));
        (Rect::new(c.x, c.y, c.x + c.w - 1, c.y + c.h), Some(sb))
    } else {
        (c.rect(), None)
    }
}

/// Узлы `TreeView` из разобранного дерева.
pub fn tree_nodes(items: &[TreeItem], expanded: bool) -> Vec<TreeNode> {
    items.iter().map(|t| TreeNode::new(&t.text).expanded(expanded && !t.children.is_empty()).children(tree_nodes(&t.children, expanded))).collect()
}

/// Общие флаги элемента: недоступность и контекст справки.
fn finish(vs: &mut ViewState, c: &Control) {
    if c.disabled {
        vs.state |= sf::DISABLED;
    }
    vs.help_ctx = c.help_ctx.clamp(0, 65535) as u16;
}

/// Собрать диалог. `resolve` переводит имя команды кнопки в номер.
/// Возвращает диалог и id элементов по именам.
pub fn build_dialog(form: &Form, resolve: &dyn Fn(&str) -> u16) -> (Dialog, HashMap<String, ViewId>) {
    let mut d = Dialog::new(Rect::new(form.x, form.y, form.x + form.w, form.y + form.h), &form.title);
    if form.centered {
        d.win.group.vs.options |= of::CENTERED;
    }
    d.set_palette(match form.palette {
        crate::model::Pal::Gray => DialogPalette::Gray,
        crate::model::Pal::Blue => DialogPalette::Blue,
        crate::model::Pal::Cyan => DialogPalette::Cyan,
    });
    d.win.group.vs.help_ctx = form.help_ctx.clamp(0, 65535) as u16;
    let mut ids: HashMap<String, ViewId> = HashMap::new();
    for i in build_order(form) {
        let c = &form.controls[i];
        let r = c.rect();
        let id = match c.kind {
            Kind::StaticText => {
                let mut v = StaticText::new(r, &c.text);
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::Label => {
                let link = ids.get(&c.link).copied();
                let mut v = Label::new(r, &c.text, link);
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::Button => {
                let mut v = Button::new(r, &c.text, resolve(&c.command), if c.default { bf::DEFAULT } else { bf::NORMAL });
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::InputLine | Kind::ComboBox => {
                let iw = input_width(c);
                let mut v = InputLine::new(Rect::new(c.x, c.y, c.x + iw, c.y + 1), c.max_len.max(1) as usize);
                v.set_text(&c.text);
                if c.kind == Kind::InputLine {
                    v.validator = make_validator(&c.validator);
                }
                finish(&mut v.vs, c);
                let id = d.add(v);
                let side = Rect::new(c.x + iw, c.y, c.x + c.w, c.y + 1);
                if c.kind == Kind::ComboBox {
                    let items: Vec<&str> = c.items.iter().map(|s| s.as_str()).collect();
                    d.add(Combo::new(side, id, cbx::NO_TRANSFER, &items));
                } else if c.history > 0 {
                    d.add(History::new(side, id, c.history as u16));
                }
                id
            }
            Kind::CheckBoxes | Kind::RadioButtons => {
                let items: Vec<&str> = c.items.iter().map(|s| s.as_str()).collect();
                let mut v = if c.kind == Kind::CheckBoxes { Cluster::check_boxes(r, &items) } else { Cluster::radio_buttons(r, &items) };
                v.value = c.value.max(0) as u32;
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::ListBox => {
                let (lr, sb) = if c.scrollbar && c.w > 1 {
                    let sb = d.add(ScrollBar::new(Rect::new(c.x + c.w - 1, c.y, c.x + c.w, c.y + c.h)));
                    (Rect::new(c.x, c.y, c.x + c.w - 1, c.y + c.h), Some(sb))
                } else {
                    (r, None)
                };
                let mut v = ListBox::new(lr, c.columns.max(1) as usize, sb);
                v.set_items(c.items.clone());
                if c.value > 0 && (c.value as usize) < c.items.len() {
                    v.focus_item(c.value as usize);
                }
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::Memo => {
                let (mr, sb) = if c.scrollbar && c.w > 1 {
                    let sb = d.add(ScrollBar::new(Rect::new(c.x + c.w - 1, c.y, c.x + c.w, c.y + c.h)));
                    (Rect::new(c.x, c.y, c.x + c.w - 1, c.y + c.h), Some(sb))
                } else {
                    (r, None)
                };
                let mut v = Memo::new(mr, None, sb);
                v.editor.set_text(&c.text);
                v.editor.modified = false;
                finish(&mut v.editor.vs, c);
                d.add(v)
            }
            Kind::ScrollBar => {
                let mut v = ScrollBar::new(r);
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::ProgressBar => {
                let mut v = ProgressBar::new(r, c.max.max(1));
                v.set_value(c.value);
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::Table => {
                let (tr, sb) = with_scroll(&mut d, c);
                let cols = c.table_columns();
                let headers: Vec<&str> = cols.iter().map(|(t, _)| t.as_str()).collect();
                let model = VecTableModel::new(&headers, c.table_rows()).editable(c.editable);
                let mut v = Table::new(tr, Box::new(model), sb, None)
                    .with_columns(cols.iter().map(|(t, w)| TableColumn::new(*w).title(t)).collect())
                    .header(c.header)
                    .grid(c.grid)
                    .editable(c.editable)
                    .row_numbers(c.row_numbers);
                if c.value > 0 {
                    v.set_cursor(c.value as usize, 0);
                }
                finish(&mut v.vs, c);
                d.add(v)
            }
            Kind::TreeView => {
                let (tr, sb) = with_scroll(&mut d, c);
                let mut v = TreeView::new(tr, tree_nodes(&parse_tree(&c.items), c.expanded), sb, None).lines(c.lines).checkboxes(c.checks);
                v.vs.grow_mode = 0;
                finish(&mut v.vs, c);
                d.add(v)
            }
        };
        ids.insert(c.name.clone(), id);
    }
    (d, ids)
}

/// Данные диалога в виде текста (для окна «Результат проверки формы»).
pub fn describe_data(form: &Form, v: &Value) -> String {
    let fields = form.data_controls();
    let list = v.as_list().unwrap_or(&[]);
    if fields.is_empty() {
        return "У формы нет элементов с данными.".into();
    }
    let mut out = String::new();
    for (c, val) in fields.iter().zip(list.iter()) {
        let shown = match val {
            Value::Str(s) => format!("{:?}", s),
            Value::U32(n) => format!("{} (0b{:b})", n, n),
            Value::Int(n) if c.kind == Kind::Table => match c.table_rows().get(*n as usize) {
                Some(r) => format!("строка {} «{}»", n, r.join(" | ")),
                None => format!("строка {}", n),
            },
            Value::Int(n) => match c.items.get(*n as usize) {
                Some(it) => format!("{} «{}»", n, it),
                None => n.to_string(),
            },
            other => format!("{:?}", other),
        };
        out.push_str(&format!("{}: {}\n", c.name, shown));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{about_form, Control};

    #[test]
    fn builds_all_kinds() {
        let mut f = about_form("x");
        f.w = 70;
        f.h = 24;
        let mut y = 1;
        for k in Kind::ALL {
            let mut c = f.new_control(k, 2, y);
            if k == Kind::InputLine {
                c.history = 5;
                c.validator = "range:1..10".into();
            }
            y += c.h;
            f.controls.push(c);
        }
        let mut l = Control::new(Kind::Label, "lbl", 40, 1);
        l.link = "input1".into();
        f.controls.push(l);
        let (d, ids) = build_dialog(&f, &|_| 1000);
        assert_eq!(ids.len(), f.controls.len());
        // Данные: строка ввода, флажки, переключатели, список, поле, комбо.
        let data = d.get_data();
        assert_eq!(data.as_list().unwrap().len(), f.data_controls().len());
        let text = describe_data(&f, &data);
        assert!(text.contains("input1"), "{}", text);
    }
}
