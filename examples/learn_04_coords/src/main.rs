//! Урок 4. Координаты: где что находится.
//!
//! Запуск: `cargo run --example learn_04_coords`. Щёлкайте мышью внутри
//! окна — внизу окна появятся координаты щелчка. F2 — диалог, разложенный
//! по правилам из урока. Alt+X — выход.

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


/// «Линейка»: рисует номера столбцов и строк и показывает, куда щёлкнули.
/// (Как писать свои представления, разберём в уроке 7 — здесь важны числа.)
struct Ruler {
    vs: ViewState,
    /// Последний щелчок: (свои координаты, координаты владельца-окна).
    click: Option<(Point, Point)>,
}

impl Ruler {
    fn new(bounds: Rect) -> Self {
        let mut vs = ViewState::new(bounds);
        vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y; // растягиваться вместе с окном
        Ruler { vs, click: None }
    }
}

impl View for Ruler {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let normal = dc.color(1);
        let w = self.vs.size.x;
        let h = self.vs.size.y;
        // Рисуем в СВОИХ координатах: (0, 0) — левый верхний угол линейки.
        dc.fill(Rect::new(0, 0, w, h), '·', normal);
        for x in 0..w {
            dc.write_char(x, 0, char::from_digit((x % 10) as u32, 10).unwrap(), normal, 1);
        }
        for y in 0..h {
            dc.write_char(0, y, char::from_digit((y % 10) as u32, 10).unwrap(), normal, 1);
        }
        let text = match self.click {
            Some((own, owner)) => format!(" свои: ({}, {})   в окне: ({}, {}) ", own.x, own.y, owner.x, owner.y),
            None => " щёлкните мышью ".to_string(),
        };
        dc.write_str(2, h - 1, &text, normal);
    }

    fn handle_event(&mut self, ev: &mut Event, _ctx: &mut Ctx) {
        if let Event::MouseDown(m) = ev {
            // До представления позиция доходит в координатах ВЛАДЕЛЬЦА (окна).
            // `make_local` переводит её в свои: вычитает наш `origin`.
            self.click = Some((self.vs.make_local(m.pos), m.pos));
            ev.clear();
        }
    }
}

/// Диалог, разложенный по правилам: подписи слева, поля справа,
/// кнопки высотой 2 внизу по центру.
fn layout_dialog() -> Dialog {
    let (w, h) = (44, 12); // ширина и высота диалога вместе с рамкой
    let mut d = Dialog::centered(w, h, "Разметка");
    let rows = ["~И~мя:", "~Г~ород:", "~П~очта:"];
    for (i, text) in rows.iter().enumerate() {
        let y = 2 + 2 * i as i32; // строки 2, 4, 6: через одну
                                  // Поле: от столбца 12 до `w - 2` (за 2 клетки до правой рамки).
        let field = d.add(InputLine::new(Rect::new(12, y, w - 2, y + 1), 50));
        // Подпись: столбцы 2..12 на той же строке.
        d.add(Label::new(Rect::new(2, y, 12, y + 1), text, Some(field)));
    }
    // Две кнопки шириной 12 с промежутком 2 — ровно по центру.
    let bw = 12;
    let x0 = (w - (bw + 2 + bw)) / 2;
    d.add(Button::new(Rect::new(x0, h - 3, x0 + bw, h - 1), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(x0 + bw + 2, h - 3, x0 + 2 * bw + 2, h - 1), "Отмена", cm::CANCEL, bf::NORMAL));
    d
}

struct App;

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        if ev.command() == Some(CM_LAYOUT) {
            app.exec_dialog(layout_dialog());
            ev.clear();
        }
    }
}

fn open_window(app: &mut Program) {
    // Окно в координатах РАБОЧЕГО СТОЛА: левый верхний угол — (3, 1).
    let mut w = Window::new(Rect::new(3, 1, 63, 18), "Координаты", 1);
    // Линейка занимает всё внутри рамки.
    let inside = w.interior(); // для окна 60×17 это (1, 1)–(59, 16)
    w.add(Ruler::new(inside));
    app.insert_window(w);
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    let mut app = Program::with_terminal(None, Some(status()))?;
    open_window(&mut app);
    app.run(&mut App);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    #[test]
    fn click_coordinates() {
        // Меню нет — рабочий стол начинается со строки 0 экрана.
        // Окно — (3, 1) на столе; линейка — (1, 1) в окне; значит, её
        // (0, 0) на экране — (4, 2). Щелчок в (10, 5) — это (6, 3) у линейки.
        let mut b = HeadlessBackend::new(80, 25);
        b.push_click(10, 5);
        let mut app = Program::new(Box::new(b), None, Some(status()));
        open_window(&mut app);
        app.pump(&mut App);
        assert!(app.screen_text().contains("свои: (6, 3)   в окне: (7, 4)"), "{}", app.screen_text());
    }

    #[test]
    fn dialog_inside_frame() {
        let d = layout_dialog();
        let s = d.win.group.vs.size;
        let inside = Rect::new(1, 1, s.x - 1, s.y - 1);
        for c in &d.win.group.children {
            let b = c.vs().bounds();
            assert_eq!(inside.intersect(&b), b, "{:?} залезает на рамку", b);
        }
    }
}
