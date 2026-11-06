//! Порт примера `examples/tvedit` из Turbo Vision: многооконный текстовый
//! редактор с меню, файловыми диалогами, поиском и заменой.
//!
//! Запуск: `cargo run --example tvedit [файлы...]`
//!
//! Отличие от оригинала: «Выход» — Alt+X, а не Ctrl+Q, потому что Ctrl+Q
//! в редакторе — префикс команд WordStar (Ctrl+Q F — поиск, Ctrl+Q A — замена).

use std::path::{Path, PathBuf};

use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// Команды приложения (как в `tvedit.h`).

struct EditorApp {
    /// Счётчик для номеров окон.
    windows: i16,
}

impl EditorApp {
    /// Открыть окно редактора (`openEditor`).
    fn open_editor(&mut self, app: &mut Program, file: &Path) {
        self.windows += 1;
        let r = app.desktop().group.vs.extent();
        let w = EditWindow::new(r, file, if self.windows < 10 { self.windows } else { 0 });
        app.insert_window(w);
    }

    /// «Открыть файл» (`fileOpen`).
    fn file_open(&mut self, app: &mut Program) {
        let d = FileDialog::new("*", "Open file", "~N~ame", fd::OPEN_BUTTON, 100);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::CANCEL {
            let name = d.file_name();
            self.open_editor(app, &name);
        }
    }

    /// «Сменить каталог» (`changeDir`).
    fn change_dir(&mut self, app: &mut Program) {
        let d = ChDirDialog::new(cd::NORMAL, 0);
        app.exec_dialog(d);
    }
}

impl AppHandler for EditorApp {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        let c = match ev.command() {
            Some(c) => c,
            None => return,
        };
        match c {
            cm::OPEN => self.file_open(app),
            cm::NEW => self.open_editor(app, Path::new("")),
            CM_CHANGE_DRCT => self.change_dir(app),
            CM_SHOW_CLIP => {
                let t = app.ctx.clipboard.get();
                let text = if t.is_empty() { "(пусто)".to_string() } else { t };
                message_box_rect(app, Rect::new(0, 0, 60, 12), &format!("Буфер обмена:\n{}", text), mf::INFORMATION | mf::OK_BUTTON, true);
            }
            cm::DOS_SHELL => {
                message_box(app, "\x03Выход в оболочку не поддерживается в этом примере.", mf::INFORMATION | mf::OK_BUTTON);
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
    let mut ed = EditorApp { windows: 0 };
    // Файлы из командной строки (как в оригинале).
    for arg in std::env::args().skip(1) {
        ed.open_editor(&mut app, &PathBuf::from(arg));
    }
    app.run(&mut ed);
    Ok(())
}
