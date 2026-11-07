//! Демонстрация возможностей порта: окна, меню, диалог со всеми элементами,
//! список с полосой прокрутки, окна сообщений, контекстное меню.
//!
//! Запуск: `cargo run --example demo`
//!
//! Клавиши: F2 — новое окно, F3 — список, Alt+D — диалог, F5 — распахнуть,
//! F6 — следующее окно, Ctrl+F5 — двигать окно стрелками, Alt+F3 — закрыть,
//! правая кнопка мыши на рабочем столе — контекстное меню, F1 — справка
//! по текущему окну, Alt+X — выход.

use std::rc::Rc;

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// Команды приложения (как `const ushort cmXXX = 100...` в программах на C++).

// Контексты справки (сгенерированы бы `tvhc` из examples/demohelp.txt).
const HC_WINDOWS: u16 = 10;
const HC_SETTINGS_DIALOG: u16 = 20;
const HC_OUTLINE_WINDOW: u16 = 30;
const HC_COLORS: u16 = 40;

struct Demo {
    /// Счётчик окон — для номеров и заголовков.
    windows: i16,
    /// Данные диалога между запусками (как структура `DialogData` в C++).
    dialog_data: Value,
    /// Справка (исходник в формате tvhc вшит в программу).
    help: Rc<HelpFile>,
}

impl Demo {
    /// Новое окно с текстом.
    fn new_window(&mut self, app: &mut Program) {
        self.windows += 1;
        let n = self.windows;
        let x = (n as i32 * 3) % 30;
        let y = (n as i32 * 2) % 10;
        let mut w = Window::new(Rect::new(x, y, x + 40, y + 12), &format!("Окно {}", n), n);
        w.group.vs.options |= of::TILEABLE;
        w.group.vs.help_ctx = HC_WINDOWS;
        let r = w.interior().grown(-1, 0);
        let mut t = StaticText::new(
            r,
            "Это порт Turbo Vision на Rust.\n\nОкно можно двигать мышью за заголовок, менять размер за правый нижний угол, распахивать кнопкой [↑] или F5.",
        );
        t.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        w.add(t);
        app.insert_window(w);
    }

    /// Окно со списком и полосой прокрутки.
    fn list_window(&mut self, app: &mut Program) {
        self.windows += 1;
        let n = self.windows;
        let mut w = Window::new(Rect::new(10, 3, 50, 18), "Список", n);
        w.group.vs.options |= of::TILEABLE;
        let sb = w.standard_scroll_bar(true);
        let mut lb = ListBox::new(w.interior(), 1, Some(sb));
        lb.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;
        lb.set_items((1..=100).map(|i| format!("Строка номер {:3}", i)).collect());
        w.add(lb);
        app.insert_window(w);
    }

    /// Диалог со всеми видами элементов.
    fn dialog(&mut self, app: &mut Program) {
        let mut d = Dialog::centered(56, 18, "Настройки");
        d.win.group.vs.help_ctx = HC_SETTINGS_DIALOG;

        let name = d.add(InputLine::new(Rect::new(14, 2, 52, 3), 64));
        d.add(Label::new(Rect::new(2, 2, 13, 3), "~И~мя:", Some(name)));

        let opts = d.add(Cluster::check_boxes(Rect::new(3, 5, 26, 8), &["~Ж~ирный", "~К~урсив", "~П~одчёркнутый"]));
        d.add(Label::new(Rect::new(2, 4, 12, 5), "Стиль", Some(opts)));

        let color = d.add(Cluster::radio_buttons(Rect::new(29, 5, 52, 8), &["~С~иний", "~З~елёный", "К~р~асный"]));
        d.add(Label::new(Rect::new(28, 4, 38, 5), "Цвет", Some(color)));

        let sb = d.add(ScrollBar::new(Rect::new(52, 10, 53, 14)));
        let mut lb = ListBox::new(Rect::new(3, 10, 52, 14), 2, Some(sb));
        lb.set_items(["Rust", "C++", "Pascal", "C", "Go", "Zig", "Ada", "Modula-2", "Oberon", "Fortran"].iter().map(|s| s.to_string()).collect());
        let list = d.add(lb);
        d.add(Label::new(Rect::new(2, 9, 12, 10), "~Я~зык", Some(list)));

        d.add(Button::new(Rect::new(14, 15, 26, 17), "~O~K", cm::OK, bf::DEFAULT));
        d.add(Button::new(Rect::new(30, 15, 42, 17), "Отмена", cm::CANCEL, bf::NORMAL));

        // Восстанавливаем прошлые значения (`setData`).
        if self.dialog_data != Value::None {
            d.set_data(&self.dialog_data);
        }
        let (cmd, d) = app.exec_dialog(d);
        if cmd == cm::OK {
            // Читаем значения (`getData`).
            self.dialog_data = d.get_data();
            let text = format!("Получены данные:\n{:?}", self.dialog_data);
            message_box_rect(app, Rect::new(0, 0, 60, 12), &text, mf::INFORMATION | mf::OK_BUTTON, true);
        }
    }

    /// Окно с деревом каталогов (`TOutline`).
    fn outline_window(&mut self, app: &mut Program) {
        fn tree(dir: &std::path::Path, depth: usize) -> Vec<Node> {
            let mut v: Vec<_> = std::fs::read_dir(dir).map(|rd| rd.flatten().collect()).unwrap_or_else(|_| Vec::new());
            v.sort_by_key(|e| e.file_name());
            v.into_iter()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .take(50)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() && depth > 0 {
                        Node::with_children(&name, tree(&e.path(), depth - 1), false)
                    } else {
                        Node::new(&name)
                    }
                })
                .collect()
        }
        self.windows += 1;
        let cur = std::env::current_dir().unwrap_or_default();
        let mut w = Window::new(Rect::new(20, 1, 70, 20), "Дерево", self.windows);
        w.group.vs.options |= of::TILEABLE;
        w.group.vs.help_ctx = HC_OUTLINE_WINDOW;
        let h = w.standard_scroll_bar(false);
        let v = w.standard_scroll_bar(true);
        let root = Node::with_children(&cur.display().to_string(), tree(&cur, 2), true);
        w.add(Outline::new(w.interior(), Some(h), Some(v), vec![root]));
        app.insert_window(w);
    }

    /// Диалог цветов (`TColorDialog`): меняет палитру всего приложения.
    fn colors(&mut self, app: &mut Program) {
        let mut d = ColorDialog::new(&app.palette, standard_color_groups());
        d.dlg.win.group.vs.help_ctx = HC_COLORS;
        let (cmd, d) = app.exec_dialog(d);
        if cmd == cm::OK {
            app.palette = d.palette.clone();
        }
    }

    /// Справка по текущему контексту (F1).
    fn help(&mut self, app: &mut Program) {
        let ctx = app.help_ctx();
        let w = HelpWindow::new(self.help.clone(), ctx);
        app.exec_dialog(w);
    }

    fn context_menu() -> Rc<Menu> {
        Rc::new(
            Menu::new().item("Новое ~о~кно", cm::NEW).item("~С~писок", CM_LIST).separator().item("~М~озаика", cm::TILE).item("~К~аскад", cm::CASCADE),
        )
    }
}

impl AppHandler for Demo {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        // Правая кнопка мыши на свободном месте — контекстное меню.
        if let Event::MouseDown(m) = ev {
            if m.buttons & 2 != 0 {
                app.ctx.popup_menu(Self::context_menu(), m.pos);
                ev.clear();
                return;
            }
        }
        let c = match ev.command() {
            Some(c) => c,
            None => return,
        };
        match c {
            cm::NEW => self.new_window(app),
            CM_LIST => self.list_window(app),
            CM_DIALOG => self.dialog(app),
            CM_OUTLINE => self.outline_window(app),
            CM_COLORS => self.colors(app),
            cm::HELP => self.help(app),
            CM_ABOUT => {
                message_box(app, "\x03Turbo Vision для Rust\n\x03порт magiblot/tvision", mf::INFORMATION | mf::OK_BUTTON);
            }
            CM_ASK_NAME => {
                if let Some(name) = input_box(app, "Знакомство", "~К~ак вас зовут?", "", 40) {
                    message_box(app, &format!("\x03Привет, {}!", name), mf::INFORMATION | mf::OK_BUTTON);
                }
            }
            _ => return,
        }
        ev.clear();
    }
}

fn main() -> std::io::Result<()> {
    let menu = ui::menu_bar();

    let status = ui::status_line();

    let mut app = Program::with_terminal(Some(menu), Some(status))?;

    let help = Rc::new(HelpFile::parse(include_str!("demohelp.txt")).expect("справка"));

    let mut demo = Demo { windows: 0, dialog_data: Value::None, help };

    demo.new_window(&mut app);

    app.run(&mut demo);

    Ok(())
}
