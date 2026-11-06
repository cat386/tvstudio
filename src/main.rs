//! TV Studio — запуск среды.
//!
//! `tvstudio [проект.tvsp | каталог проекта | файлы...]`. Без аргументов
//! открывается последний проект.

use tvision::*;
use tvstudio::dialogs::Config;
use tvstudio::ide::{menu_bar, status_line, Ide};

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut app = Program::with_terminal(Some(menu_bar()), Some(status_line()))?;
    let mut ide = Ide::new(Config::load(), false);
    ide.startup(&mut app, std::env::args().skip(1).collect());
    app.run(&mut ide);
    ide.shutdown(&mut app);
    Ok(())
}
