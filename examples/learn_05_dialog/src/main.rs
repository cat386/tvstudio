//! Урок 5. Первый диалог: спросить имя и поздороваться.
//!
//! Запуск: `cargo run --example learn_05_dialog`. F2 — диалог, Alt+X — выход.

use tvision::*;

// Меню, строка состояния, команды и формы — из модели проекта
// (ui.rs и forms/ генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


struct App {
    /// Имя, которое ввели в прошлый раз (диалог предложит его снова).
    name: String,
}

/// Диалог «Знакомство» нарисован в дизайнере TV Studio (форма
/// forms/name_dialog.tvf, код — src/forms/name_dialog.rs). Возвращаем сам
/// диалог и номер (id) строки ввода — по нему потом достанем введённый текст.
fn name_dialog(old_name: &str) -> (Dialog, ViewId) {
    let (mut d, ids) = forms::name_dialog::build();
    // Предлагаем имя, которое ввели в прошлый раз.
    d.win.group.find_mut::<InputLine>(ids.name).unwrap().set_text(old_name);
    (d, ids.name)
}

impl App {
    fn ask(&mut self, app: &mut Program) {
        let (d, input_id) = name_dialog(&self.name);

        // Показать диалог и ждать, пока его закроют. Пока он открыт, остальная
        // программа «замирает» — это и называется модальным окном.
        // Возвращаются: команда, которой закрыли (OK или Cancel), и сам диалог.
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::OK {
            return; // нажали «Отмена» или Esc
        }

        // Диалог ещё у нас в руках — достаём из него строку ввода по id.
        let name = d.win.group.find::<InputLine>(input_id).unwrap().text();
        let name = name.trim().to_string();
        if name.is_empty() {
            message_box(app, "\x03Вы не ввели имя.", mf::WARNING | mf::OK_BUTTON);
        } else {
            message_box(app, &format!("\x03Привет, {}!", name), mf::INFORMATION | mf::OK_BUTTON);
            self.name = name;
        }
    }
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        if ev.command() == Some(CM_ASK) {
            self.ask(app);
            ev.clear();
        }
    }
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut app = Program::with_terminal(None, Some(status()))?;
    let mut h = App { name: String::new() };
    app.put_event(Event::command_event(CM_ASK)); // сразу открыть диалог
    app.run(&mut h);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    #[test]
    fn remembers_name() {
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::F2);
        for c in "Аня".chars() {
            b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
        }
        b.push_key(kb::ENTER); // OK; окно «Привет» закроет Esc тестового терминала
        let mut app = Program::new(Box::new(b), None, Some(status()));
        let mut h = App { name: String::new() };
        app.pump(&mut h);
        assert_eq!(h.name, "Аня");
    }
}
