//! TV Studio — запуск среды.
//!
//! `tvstudio [проект.tvsp | каталог проекта | файлы...]`. Без аргументов
//! открывается последний проект. Каталог с `Cargo.toml`, но без файла
//! проекта, берётся в новый проект «программа на Rust» и открывается.

use tvision::*;
use tvstudio::dialogs::Config;
use tvstudio::ide::{menu_bar, status_line, Ide};

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut ide = Ide::new(Config::load(), false);
    let backend = tvstudio::hover::HoverBackend::new(tvision::platform::default_backend()?, ide.lsp_state.pointer.clone());
    let mut app = Program::new(Box::new(backend), Some(menu_bar()), Some(status_line()));
    if tvision::color::color_mode() == tvision::color::ColorMode::Ansi16 {
        app.palette = tvision::color::palettes::APP_COLOR.to_vec();
    }
    if let Ok(path) = std::env::var(tvision::theme::THEME_ENV) {
        if let Ok(theme) = tvision::theme::Theme::load(&path) {
            app.set_theme(&theme);
        }
    }
    ide.startup(&mut app, std::env::args().skip(1).collect());
    app.run(&mut ide);
    ide.shutdown(&mut app);
    Ok(())
}
