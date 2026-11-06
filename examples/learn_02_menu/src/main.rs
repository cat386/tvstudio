//! Урок 2. Меню, строка состояния и свои команды.
//!
//! Запуск: `cargo run --example learn_02_menu`. F10 — меню, F2 — приветствие,
//! Alt+X — выход.

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// Свои команды. Это просто числа; главное — не пересечься со стандартными
// (`cm::QUIT`, `cm::OK`…), которые заняли номера меньше 100.

/// Наше приложение. Здесь живут данные программы — пока только счётчик.
struct App {
    greetings: u32,
}

// `AppHandler` — «мозг» программы: сюда приходят команды из меню и строки
// состояния, которые не обработал никто другой.
impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev.command() {
            Some(CM_HELLO) => {
                self.greetings += 1;
                let text = format!("\x03Привет! Это приветствие номер {}.", self.greetings);
                message_box(app, &text, mf::INFORMATION | mf::OK_BUTTON);
            }
            Some(CM_ABOUT) => {
                message_box(app, "\x03Урок 2\n\x03Меню и команды", mf::INFORMATION | mf::OK_BUTTON);
            }
            // Чужие события не трогаем — просто выходим.
            _ => return,
        }
        // Команда обработана: помечаем событие, чтобы оно не пошло дальше.
        ev.clear();
    }
}

/// Строка меню: два выпадающих меню.
fn menu() -> Menu {
    ui::menu_bar()
}

/// Строка состояния.
fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    // Кнопки окон сообщений («Да», «Отмена»…) — по-русски.
    locale::set_language(locale::Language::Ru);
    let mut app = Program::with_terminal(Some(menu()), Some(status()))?;
    app.run(&mut App { greetings: 0 });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    #[test]
    fn f2_greets() {
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::F2); // окно сообщения закроет Esc тестового терминала
        let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
        let mut h = App { greetings: 0 };
        app.pump(&mut h);
        assert_eq!(h.greetings, 1);
        // Второй раз — командой, как будто выбрали пункт меню.
        app.put_event(Event::command_event(CM_HELLO));
        app.pump(&mut h);
        assert_eq!(h.greetings, 2);
        assert!(app.screen_text().contains("Файл"));
    }
}
