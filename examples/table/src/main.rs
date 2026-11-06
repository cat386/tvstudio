//! Пример таблицы (`Table`).
//!
//! Запуск: `cargo run --example table`
//!
//! * F2 — «Большая таблица»: миллион строк × тысяча столбцов. Данные не
//!   хранятся, а вычисляются по запросу (`FnTableModel`); номера строк,
//!   закреплённый слева столбец, прокрутка мышью и полосами.
//! * F3 — «Склад»: своя модель данных (трейт `TableModel`) с вычисляемым
//!   столбцом «Сумма», проверкой ввода, правкой (F2 или любая буква) и
//!   сортировкой (щелчок по заголовку или Alt+1…Alt+5 в меню).
//! * F4 — диалог выбора города (`VecTableModel`, выбор целой строки).
//! * Alt+X — выход.
//!
//! В таблице: стрелки, PgUp/PgDn, Home/End, Ctrl+Home/End; Ctrl+←/→ —
//! ширина столбца (или протяжка границы в заголовке мышью); пробел и
//! Shift+↑/↓ — выделение строк (где включено); Ctrl+C — копировать.

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


// ----------------------------------------------------------------------
// Своя модель: склад товаров
// ----------------------------------------------------------------------

struct Item {
    code: u32,
    name: String,
    qty: u32,
    price: f64,
}

/// Модель склада. Пятый столбец («Сумма») не хранится — он вычисляется.
struct Goods {
    items: Vec<Item>,
}

impl Goods {
    fn sample() -> Self {
        let names = [
            "Болт М6",
            "Гайка М6",
            "Шайба 6",
            "Саморез 3,5×25",
            "Дюбель 6×40",
            "Уголок 40×40",
            "Петля дверная",
            "Ручка-скоба",
            "Замок врезной",
            "Шуруп 4×50",
            "Хомут 20–32",
            "Кронштейн",
            "Анкер 10×100",
            "Скоба",
            "Гвоздь 70",
            "Заклёпка 4×10",
        ];
        let items = names
            .iter()
            .enumerate()
            .map(|(i, n)| Item {
                code: 1000 + i as u32 * 7,
                name: n.to_string(),
                qty: ((i * 37 + 11) % 500) as u32,
                price: ((i * 53 + 7) % 900) as f64 / 10.0 + 0.5,
            })
            .collect();
        Goods { items }
    }
}

impl TableModel for Goods {
    fn row_count(&self) -> usize {
        self.items.len()
    }
    fn col_count(&self) -> usize {
        5
    }
    fn cell(&self, row: usize, col: usize) -> String {
        let it = &self.items[row];
        match col {
            0 => it.code.to_string(),
            1 => it.name.clone(),
            2 => it.qty.to_string(),
            3 => format!("{:.2}", it.price),
            _ => format!("{:.2}", it.qty as f64 * it.price),
        }
    }
    fn header(&self, col: usize) -> String {
        ["Код", "Наименование", "Кол-во", "Цена", "Сумма"][col].to_string()
    }
    /// Код и сумму править нельзя.
    fn editable(&self, _row: usize, col: usize) -> bool {
        (1..=3).contains(&col)
    }
    /// Новое значение проверяется: в числовые столбцы — только числа.
    fn set_cell(&mut self, row: usize, col: usize, value: &str) -> bool {
        let it = &mut self.items[row];
        match col {
            1 if !value.trim().is_empty() => it.name = value.trim().to_string(),
            2 => match value.trim().parse() {
                Ok(q) => it.qty = q,
                Err(_) => return false,
            },
            3 => match value.trim().replace(',', ".").parse::<f64>() {
                Ok(p) if p >= 0.0 => it.price = p,
                _ => return false,
            },
            _ => return false,
        }
        true
    }
    fn sort(&mut self, col: usize, ascending: bool) -> bool {
        self.items.sort_by(|a, b| {
            let o = match col {
                0 => a.code.cmp(&b.code),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.qty.cmp(&b.qty),
                3 => a.price.total_cmp(&b.price),
                _ => (a.qty as f64 * a.price).total_cmp(&(b.qty as f64 * b.price)),
            };
            if ascending {
                o
            } else {
                o.reverse()
            }
        });
        true
    }
}

// ----------------------------------------------------------------------
// Приложение
// ----------------------------------------------------------------------

struct App {
    windows: i16,
}

impl App {
    /// Окно с таблицей во всю внутреннюю часть и двумя полосами прокрутки.
    fn table_window(&mut self, app: &mut Program, r: Rect, title: &str, model: Box<dyn TableModel>, setup: impl FnOnce(Table) -> Table) {
        self.windows += 1;
        let mut w = Window::new(r, title, self.windows);
        w.group.vs.options |= of::TILEABLE;
        let v = w.standard_scroll_bar(true);
        let h = w.standard_scroll_bar(false);
        let mut t = setup(Table::new(w.interior(), model, Some(v), Some(h)));
        // Таблица растягивается вместе с окном.
        t.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        w.add(t);
        app.insert_window(w);
    }

    /// Миллион строк × тысяча столбцов, вычисляемых на лету.
    fn big(&mut self, app: &mut Program) {
        let model = FnTableModel::new(1_000_000, 1_000, |row, col| {
            if col == 0 {
                format!("ID-{:07}", row + 1)
            } else {
                // Псевдослучайное, но повторяемое число для ячейки.
                let h = (row as u64).wrapping_mul(2_654_435_761) ^ (col as u64).wrapping_mul(40_503);
                format!("{}", h % 100_000)
            }
        });
        // Столбцы: первый — ключ (закреплён), остальные — числа справа.
        let mut cols = vec![TableColumn::new(11).title("Ключ").resizable(false)];
        cols.extend((1..1_000).map(|_| TableColumn::new(8).align(Align::Right).limits(4, 20)));
        let r = app.desktop_mut().group.vs.extent();
        self.table_window(app, r, "Большая таблица: 1 000 000 × 1 000", Box::new(model), |t| {
            t.with_columns(cols).row_numbers(true).frozen(1).sortable(false)
        });
    }

    /// Склад: своя модель, правка, сортировка, «зебра», выделение строк.
    fn goods(&mut self, app: &mut Program) {
        let cols = vec![
            TableColumn::new(6).align(Align::Right),
            TableColumn::new(18).limits(8, 40),
            TableColumn::new(8).align(Align::Right),
            TableColumn::new(9).align(Align::Right),
            TableColumn::new(11).align(Align::Right),
        ];
        let x = (self.windows as i32 * 2) % 10;
        self.table_window(app, Rect::new(4 + x, 2 + x / 2, 66 + x, 20 + x / 2), "Склад", Box::new(Goods::sample()), |t| {
            t.with_columns(cols).editable(true).striped(true).multi_select(true)
        });
    }

    /// Модальный диалог: выбрать город из таблицы.
    fn city(&mut self, app: &mut Program) {
        let rows: Vec<Vec<String>> = [
            ("Москва", "Центральный", "13 104 177"),
            ("Санкт-Петербург", "Северо-Западный", "5 597 763"),
            ("Новосибирск", "Сибирский", "1 633 595"),
            ("Екатеринбург", "Уральский", "1 536 183"),
            ("Казань", "Приволжский", "1 318 604"),
            ("Нижний Новгород", "Приволжский", "1 204 985"),
            ("Красноярск", "Сибирский", "1 205 473"),
            ("Челябинск", "Уральский", "1 177 058"),
            ("Самара", "Приволжский", "1 158 952"),
            ("Уфа", "Приволжский", "1 163 304"),
            ("Ростов-на-Дону", "Южный", "1 140 487"),
            ("Краснодар", "Южный", "1 138 654"),
        ]
        .iter()
        .map(|(a, b, c)| vec![a.to_string(), b.to_string(), c.to_string()])
        .collect();
        let model = VecTableModel::new(&["Город", "Округ", "Население"], rows);

        let mut d = Dialog::centered(60, 18, "Выбор города");
        let sb = d.add(ScrollBar::new(Rect::new(56, 3, 57, 14)));
        let cols = vec![TableColumn::new(18), TableColumn::new(17), TableColumn::new(14).align(Align::Right)];
        let table = Table::new(Rect::new(3, 3, 56, 14), Box::new(model), Some(sb), None).with_columns(cols).selection(SelectionMode::Row).grid(false);
        let tid = d.add(table);
        d.add(Label::new(Rect::new(2, 2, 20, 3), "~Г~орода:", Some(tid)));
        d.add(Button::new(Rect::new(17, 15, 29, 17), "O~K~", cm::OK, bf::DEFAULT));
        d.add(Button::new(Rect::new(31, 15, 43, 17), "Отмена", cm::CANCEL, bf::NORMAL));
        d.select(tid);

        let (c, d) = app.exec_dialog(d);
        if c == cm::OK {
            // Таблицу можно достать из диалога по id и спросить модель.
            if let Some(t) = d.win.group.find::<Table>(tid) {
                let (row, _) = t.cursor();
                let msg = format!("\x03Вы выбрали:\n\x03{} ({})", t.model().cell(row, 0), t.model().cell(row, 1));
                message_box(app, &msg, mf::INFORMATION | mf::OK_BUTTON);
            }
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_BIG) => self.big(app),
            Some(CM_GOODS) => self.goods(app),
            Some(CM_CITY) => self.city(app),
            _ => return,
        }
        ev.clear();
    }
}

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let menu = ui::menu_bar();
    let status = ui::status_line();
    let mut app = Program::with_terminal(Some(menu), Some(status))?;
    app.run(&mut App { windows: 0 });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::{HeadlessBackend, RawEvent};

    fn key(k: Key, m: Mods) -> RawEvent {
        RawEvent::Key(KeyEvent::new(k, m))
    }

    /// Все три окна открываются, рисуются без ошибок палитры, правка работает.
    #[test]
    fn smoke() {
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::F2); // большая таблица
        b.input.push_back(key(Key::End, Mods::CTRL));
        b.push_key(kb::f(3)); // склад
        b.input.push_back(key(Key::Right, Mods::NONE)); // «Наименование»
        b.input.push_back(key(Key::Right, Mods::NONE)); // «Кол-во»
        b.input.push_back(key(Key::Char('5'), Mods::NONE));
        b.input.push_back(key(Key::Enter, Mods::NONE));
        let menu = Menu::new().item("x", CM_BIG);
        let status = vec![StatusDef::all(vec![
            StatusItem::new("~F2~", kb::F2, CM_BIG),
            StatusItem::new("~F3~", kb::f(3), CM_GOODS),
            StatusItem::new("~F4~", kb::f(4), CM_CITY),
        ])];
        let mut app = Program::new(Box::new(b), Some(menu), Some(status));
        let mut h = App { windows: 0 };
        app.pump(&mut h);
        app.redraw();
        let s = app.screen_text();
        assert!(s.contains("Склад"), "{}", s);
        assert!(s.contains("Наименование"));
        assert!(s.contains("6.00"), "сумма пересчитана после правки количества: {}", s);
        assert!(s.contains("1000000"), "номер последней строки большой таблицы:\n{}", s);
        for y in 0..25 {
            for x in 0..80 {
                assert_ne!(app.screen().get(x, y).attr, color::ERROR_ATTR, "({}, {})", x, y);
            }
        }
    }
}
