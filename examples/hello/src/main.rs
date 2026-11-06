//! Порт классического примера `hello.cpp` из Turbo Vision.
//!
//! Запуск: `cargo run --example hello`
//! Меню «Hello» → «Greeting...» (или Alt+G) открывает диалог;
//! Alt+X — выход.

use tvision::*;

// Меню, строка состояния, команды и формы — из модели проекта
// (ui.rs и forms/ генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


/// Аналог класса `THelloApp`: обрабатывает команды, которые не обработали окна.
struct HelloApp;

impl HelloApp {
    /// Аналог `THelloApp::greetingBox()`.
    fn greeting_box(&self, app: &mut Program) {
        // Диалог нарисован в дизайнере TV Studio (форма forms/greeting.tvf).
        let (d, _ids) = forms::greeting::build();
        // `deskTop->execView(d)` — модальный запуск диалога.
        app.exec_dialog(d);
    }
}

impl AppHandler for HelloApp {
    /// Аналог `THelloApp::handleEvent`.
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        if ev.command() == Some(GREET_THEM_CMD) {
            self.greeting_box(app);
            ev.clear();
        }
    }
}

fn main() -> std::io::Result<()> {
    // Аналог `initMenuBar`.
    let menu = ui::menu_bar();
    // Аналог `initStatusLine`.
    let status = ui::status_line();

    let mut app = Program::with_terminal(Some(menu), Some(status))?;
    app.run(&mut HelloApp);
    Ok(())
}
