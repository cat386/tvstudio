//! Выбор из списка и каталога без смены cwd процесса.

use std::path::PathBuf;

use tvision::layout::{item, HBox, VBox};
use tvision::*;

use crate::toolwins::Relay;

use super::exec;

/// Список с прокруткой: Enter, двойной щелчок или «Выбрать» — `Some(индекс)`.
pub fn choose_indexed(app: &mut Program, title: &str, rows: Vec<String>, selected: usize) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let area = app.desktop().group.vs.extent();
    let mut dialog = Dialog::centered((area.width() - 2).clamp(32, 100), (rows.len().min(20) as i32 + 5).min((area.height() - 2).max(7)), title);
    let scroll = dialog.win.standard_scroll_bar(true);
    let mut list = ListBox::new(Rect::default(), 1, Some(scroll));
    let count = rows.len();
    list.set_items(rows);
    list.focus_item(selected);
    let list = dialog.add(list);
    dialog.add(Relay::new(list, cm::OK));
    let choose = dialog.add(Button::new(Rect::default(), "~В~ыбрать", cm::OK, bf::DEFAULT));
    let cancel = dialog.add(Button::new(Rect::default(), "~О~тмена", cm::CANCEL, bf::NORMAL));
    dialog.set_layout(VBox::new().add(item(list).expand().stretch(1)).add(HBox::new().spacing(1).add(choose).add(cancel)));
    dialog.select(list);
    let (command, dialog) = exec(app, dialog);
    if command != cm::OK {
        return None;
    }
    let index = dialog.win.group.find::<ListBox>(list)?.focused;
    (index < count).then_some(index)
}

/// Просмотр каталога; стандартный ChDirDialog меняет cwd всего процесса.
pub fn browse_directory(app: &mut Program, start: PathBuf) -> Option<PathBuf> {
    let mut directory = start.ancestors().find(|p| p.is_dir())?.to_path_buf();
    loop {
        let mut children: Vec<_> = std::fs::read_dir(&directory)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        children.sort();
        let rows = ["Выбрать этот каталог", ".. (выше)", "Диски и корни…"]
            .into_iter()
            .map(str::to_string)
            .chain(children.iter().map(|path| path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
            .collect();
        let title = format!("Каталог — {}", directory.display());
        match choose_indexed(app, &title, rows, 0)? {
            0 => return Some(directory),
            1 => {
                directory.pop();
            }
            2 => {
                let roots = available_roots();
                if !roots.is_empty() {
                    let (cmd, dialog) = exec(app, DriveDialog::new(&roots, &directory));
                    if cmd == cm::OK {
                        if let Some(path) = dialog.selection() {
                            directory = path;
                        }
                    }
                }
            }
            index => directory = children[index - 3].clone(),
        }
    }
}