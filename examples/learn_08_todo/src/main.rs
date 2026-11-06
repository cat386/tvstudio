//! Урок 8. Своё окно с данными: список дел.
//!
//! Запуск: `cargo run --example learn_08_todo`. Ins — новое дело,
//! F4 — отметить сделанным, Del — удалить, F3 — ещё одно окно-список,
//! Alt+X — выход.

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


/// Одно дело.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub text: String,
    pub done: bool,
}

/// Окно «Дела»: обычное окно + список на экране + наши данные.
pub struct TodoWindow {
    /// Стандартное окно: рамка, заголовок, дети.
    pub win: Window,
    /// Номер (id) списка внутри окна — по нему список находится.
    pub list: ViewId,
    /// Данные: что показывает список.
    pub tasks: Vec<Task>,
}

impl TodoWindow {
    pub fn new(bounds: Rect, title: &str, number: i16) -> Self {
        let mut win = Window::new(bounds, title, number);
        let sb = win.standard_scroll_bar(true); // полоса прокрутки на правой рамке
        let mut list = ListBox::new(win.interior(), 1, Some(sb));
        list.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y; // тянуться вместе с окном
        let list = win.add(list);
        TodoWindow { win, list, tasks: Vec::new() }
    }

    /// Переложить `tasks` в список на экране.
    pub fn refresh(&mut self) {
        let lines: Vec<String> = self.tasks.iter().map(|t| format!("[{}] {}", if t.done { 'x' } else { ' ' }, t.text)).collect();
        let list = self.win.group.find_mut::<ListBox>(self.list).unwrap();
        let keep = list.focused;
        list.set_items(lines);
        list.focus_item(keep.min(self.tasks.len().saturating_sub(1)));
    }

    /// Номер дела под курсором.
    pub fn current(&self) -> Option<usize> {
        let list = self.win.group.find::<ListBox>(self.list)?;
        (list.focused < self.tasks.len()).then_some(list.focused)
    }
}

// Чтобы окно попало на рабочий стол, оно должно быть представлением.
// Всю работу делаем руками стандартного окна `win` — «передаём» её ему.
impl View for TodoWindow {
    // Служебные методы и всё про детей — у группы окна.
    tvision::impl_group_basics!(win.group);

    // «Да, это окно» — тогда работают Alt+F3, F5, Alt+номер.
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
    }
}

struct App {
    lists: i16,
}

impl App {
    /// Активное окно «Дела», если оно сейчас сверху.
    fn active(app: &mut Program) -> Option<&mut TodoWindow> {
        // Окна хранятся как «какое-то представление» (`Box<dyn View>`);
        // `downcast_mut` проверяет, что это именно `TodoWindow`.
        app.desktop_mut().group.current_view_mut()?.as_any_mut().downcast_mut::<TodoWindow>()
    }

    fn new_list(&mut self, app: &mut Program) {
        self.lists += 1;
        let n = self.lists;
        let r = Rect::new(2 + 4 * (n as i32 - 1), 1 + (n as i32 - 1), 46 + 4 * (n as i32 - 1), 15 + (n as i32 - 1));
        let mut w = TodoWindow::new(r, &format!("Дела {}", n), n);
        w.tasks = vec![Task { text: "Прочитать урок 8".into(), done: false }];
        w.refresh();
        app.insert_window(w);
    }

    fn add(&mut self, app: &mut Program) {
        // Сначала спрашиваем текст — окно в это время не трогаем.
        let Some(text) = input_box(app, "Новое дело", "~Ч~то сделать:", "", 80) else { return };
        if text.trim().is_empty() {
            return;
        }
        // Потом снова находим окно и меняем данные.
        if let Some(w) = Self::active(app) {
            w.tasks.push(Task { text: text.trim().to_string(), done: false });
            w.refresh();
            let last = w.tasks.len() - 1;
            w.win.group.find_mut::<ListBox>(w.list).unwrap().focus_item(last);
        }
    }

    fn done(&mut self, app: &mut Program) {
        if let Some(w) = Self::active(app) {
            if let Some(i) = w.current() {
                w.tasks[i].done = !w.tasks[i].done;
                w.refresh();
            }
        }
    }

    fn delete(&mut self, app: &mut Program) {
        if let Some(w) = Self::active(app) {
            if let Some(i) = w.current() {
                w.tasks.remove(i);
                w.refresh();
            }
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_ADD) => self.add(app),
            Some(CM_DONE) => self.done(app),
            Some(CM_DELETE) => self.delete(app),
            Some(CM_NEW_LIST) => self.new_list(app),
            _ => return,
        }
        ev.clear();
    }
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut app = Program::with_terminal(None, Some(status()))?;
    let mut h = App { lists: 0 };
    h.new_list(&mut app);
    app.run(&mut h);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    #[test]
    fn add_done_delete() {
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::key(Key::Ins));
        for c in "Купить хлеб".chars() {
            b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
        }
        b.push_key(kb::ENTER);
        b.push_key(kb::f(4)); // курсор на новом деле — отметить
        let mut app = Program::new(Box::new(b), None, Some(status()));
        let mut h = App { lists: 0 };
        h.new_list(&mut app);
        app.pump(&mut h);
        let w = App::active(&mut app).unwrap();
        assert_eq!(w.tasks[1], Task { text: "Купить хлеб".into(), done: true });
        assert!(app.screen_text().contains("[x] Купить хлеб"));

        app.put_event(Event::command_event(CM_DELETE));
        app.pump(&mut h);
        assert_eq!(App::active(&mut app).unwrap().tasks.len(), 1);
    }
}
