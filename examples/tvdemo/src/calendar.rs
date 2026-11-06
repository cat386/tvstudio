//! Календарь (`calendar.cpp`): `TCalendarView` и `TCalendarWindow`.
//! «+»/«↓» — следующий месяц, «-»/«↑» — предыдущий, а также щелчки
//! по стрелкам ▲ ▼ в заголовке.

use std::time::SystemTime;

use tvision::*;

const MONTH_NAMES: [&str; 13] =
    ["", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS_IN_MONTH: [u32; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// День недели (0 — воскресенье), формула из оригинала (`dayOfWeek`).
pub fn day_of_week(day: i64, month: i64, year: i64) -> i64 {
    let (mut month, mut year) = (month, year);
    if month < 3 {
        month += 10;
        year -= 1;
    } else {
        month -= 2;
    }
    let century = year / 100;
    let yr = year % 100;
    let mut dw = (((26 * month - 2) / 10) + day + yr + (yr / 4) + (century / 4) - (2 * century)) % 7;
    if dw < 0 {
        dw += 7;
    }
    dw
}

/// Текущая дата (UTC) — год, месяц, день.
fn today() -> (i64, u32, u32) {
    let (y, m, d, _, _) = filedlg::civil_time(SystemTime::now());
    (y, m, d)
}

pub struct CalendarView {
    pub vs: ViewState,
    pub month: u32,
    pub year: i64,
    cur: (i64, u32, u32),
}

impl CalendarView {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.options |= of::SELECTABLE;
        let cur = today();
        CalendarView { vs, month: cur.1, year: cur.0, cur }
    }

    pub fn next_month(&mut self) {
        self.month += 1;
        if self.month > 12 {
            self.month = 1;
            self.year += 1;
        }
    }

    pub fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
    }
}

impl View for CalendarView {
    tvision::impl_view_basics!();

    /// Порт `TCalendarView::draw`.
    fn draw(&self, dc: &mut DrawCtx) {
        let color = dc.color(6);
        let bold = dc.color(7);
        let mut current = 1 - day_of_week(1, self.month as i64, self.year);
        let days = DAYS_IN_MONTH[self.month as usize] as i64 + if is_leap_year(self.year) && self.month == 2 { 1 } else { 0 };

        let mut b = DrawBuffer::new(22);
        b.move_char(0, ' ', color, 22);
        b.move_str(0, &format!("{:>9} {:4} ▲  ▼ ", MONTH_NAMES[self.month as usize], self.year), color);
        dc.write_line(0, 0, 22, 1, &b);

        b.move_char(0, ' ', color, 22);
        b.move_str(0, "Su Mo Tu We Th Fr Sa", color);
        dc.write_line(0, 1, 22, 1, &b);

        for i in 1..=6 {
            b.move_char(0, ' ', color, 22);
            for j in 0..7 {
                if current >= 1 && current <= days {
                    let today = self.year == self.cur.0 && self.month == self.cur.1 && current == self.cur.2 as i64;
                    b.move_str(j * 3, &format!("{:2}", current), if today { bold } else { color });
                }
                current += 1;
            }
            dc.write_line(0, i + 1, 22, 1, &b);
        }
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        tvision::view::default_handle_event(&self.vs, ev, ctx);
        if !self.vs.is_selected() {
            return;
        }
        match ev {
            Event::MouseDown(m) | Event::MouseAuto(m) => {
                let p = self.vs.make_local(m.pos);
                if p == Point::new(15, 0) {
                    self.prev_month();
                } else if p == Point::new(18, 0) {
                    self.next_month();
                }
                ev.clear();
            }
            Event::KeyDown(k) => {
                match k.key {
                    Key::Char('+') | Key::Down => self.next_month(),
                    Key::Char('-') | Key::Up => self.prev_month(),
                    _ => return,
                }
                ev.clear();
            }
            _ => {}
        }
    }
}

/// Окно календаря (`TCalendarWindow`): бирюзовое, без изменения размера.
pub fn calendar_window() -> Window {
    let mut w = Window::new(Rect::new(1, 1, 23, 11), "Calendar", 0);
    w.flags &= !(wf::ZOOM | wf::GROW);
    w.group.vs.grow_mode = 0;
    w.palette = color::palettes::CYAN_WINDOW;
    let r = w.group.vs.extent().grown(-1, -1);
    w.add(CalendarView::new(r));
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays() {
        assert_eq!(day_of_week(1, 1, 2024), 1); // понедельник
        assert_eq!(day_of_week(22, 10, 2026), 4); // четверг
    }
}
