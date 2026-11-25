//! Окно статуса Git и диалоги выбора файла, каталога и ветки.

use crate::cmds::*;
use crate::git::{Change, Snapshot};
use crate::toolwins::Relay;
use std::path::PathBuf;
use tvision::layout::{item, HBox, VBox};
use tvision::*;

pub struct Panel {
    pub window: ViewId,
    pub list: ViewId,
    pub snapshot: Option<Snapshot>,
}

impl Panel {
    pub fn open(app: &mut Program) -> Self {
        let extent = app.desktop().group.vs.extent();
        let bottom = app
            .desktop()
            .group
            .children
            .iter()
            .find_map(|v| v.as_any().downcast_ref::<crate::toolwins::MessagesWindow>().map(|_| v.vs().origin.y))
            .unwrap_or(extent.height() - ((extent.height() / 4).clamp(5, 9) - 2));
        let mut window = Window::new(Rect::new(1, 0, extent.width() - 1, bottom), "Git", 0);
        window.group.vs.grow_mode = gf::GROW_HI_X | gf::GROW_HI_Y;

        let scroll1 = window.add(ScrollBar::new(Rect::new(0, 0, 1, 10)));

        let mut list = ListBox::new(Rect::default(), 1, Some(scroll1));
        list.set_items(vec!["Загрузка…".into()]);
        let list_view = window.add(list);

        let line = window.add(Line::new(Rect::default()).to_frame());

        window.add(Relay::new(list_view, CM_GIT_DIFF));

        let refresh = button(&mut window, "~О~бновить", CM_GIT_REFRESH);
        let diff = button(&mut window, "~И~зменения", CM_GIT_DIFF);
        let staged = button(&mut window, "Изменения и~н~декса", CM_GIT_DIFF_STAGED);
        let add = button(&mut window, "~+~ В индекс", CM_GIT_STAGE);
        let remove = button(&mut window, "~-~ Из индекса", CM_GIT_UNSTAGE);
        let commit = button(&mut window, "~К~оммит...", CM_GIT_COMMIT);
        let branch = button(&mut window, "~В~етка...", CM_GIT_BRANCH);
        // Добавляем кнопки
        window.set_layout(
            VBox::new()
                .add(HBox::new().add(item(list_view).expand().stretch(1)).add(scroll1))
                .space(1)
                .add(line)
                .add(HBox::new().spacing(1).add(refresh).add(diff).add(staged))
                .add(HBox::new().spacing(1).add(add).add(remove).add(commit).add(branch)),
        );
        window.select(list_view);
        let window = crate::dialogs::insert(app, window);
        Self { window, list: list_view, snapshot: None }
    }

    pub fn selected(&self, app: &Program) -> Option<&Change> {
        let focused = app.find::<ListBox>(self.list)?.focused;
        self.snapshot.as_ref()?.changes.get(focused)
    }

    /// Сохраняем выбор по пути: после подготовки файла порядок записей
    /// статуса меняется и номер строки может указывать на другой файл.
    pub fn set(&mut self, app: &mut Program, snapshot: Snapshot) {
        let keep: Option<PathBuf> = self.selected(app).map(|entry| entry.path.clone());
        let mut rows = snapshot
            .changes
            .iter()
            .map(|change| format!("{}  {}", change_label(change), crate::git::display(change.path.to_string_lossy().as_bytes()).replace('\n', "↵")))
            .collect::<Vec<_>>();
        if rows.is_empty() {
            rows.push("Нет изменений".into());
        }
        if let Some(list) = app.find_mut::<ListBox>(self.list) {
            let old = list.focused;
            let cursor = keep.and_then(|path| snapshot.changes.iter().position(|entry| entry.path == path)).unwrap_or(old);
            list.set_items(rows);
            list.focus_item(cursor);
        }
        self.snapshot = Some(snapshot);
        self.ready_title(app);
    }

    pub fn ready_title(&self, app: &mut Program) {
        if let Some(snapshot) = &self.snapshot {
            let upstream = if snapshot.upstream.is_empty() { String::new() } else { format!(" → {}", snapshot.upstream) };
            let pending = snapshot.changes.iter().filter(|entry| !matches!(entry.index, '.' | '?') && !entry.unmerged).count();
            self.title(app, format!("Git — {}{}  •  файлов {}  •  в индексе {}", snapshot.branch, upstream, snapshot.changes.len(), pending));
        } else {
            self.title(app, "Git — нет данных".into());
        }
    }

    pub fn error(&self, app: &mut Program) {
        if self.snapshot.is_none() {
            if let Some(list) = app.find_mut::<ListBox>(self.list) {
                list.set_items(vec!["Не удалось прочитать статус. Подробности — в сообщениях.".into()]);
            }
        }
        self.title(app, "Git — ошибка, см. сообщения".into());
    }

    pub fn title(&self, app: &mut Program, title: String) {
        if let Some(window) = app.find_mut::<Window>(self.window) {
            window.title = title;
        }
    }
}

fn change_label(change: &Change) -> &'static str {
    if change.unmerged {
        "Конфликт"
    } else if change.index == '?' {
        "Новый файл"
    } else if change.index != '.' && change.worktree != '.' {
        "Индекс + правки"
    } else if change.index != '.' {
        "В индексе"
    } else if change.worktree == 'D' {
        "Удалён"
    } else {
        "Изменён"
    }
}

fn button(window: &mut Window, title: &str, command: u16) -> ViewId {
    window.add(Button::new(Rect::default(), title, command, bf::NORMAL))
}

/// Terminal здесь служит просмотрщиком. Он даёт прокрутку длинных строк
/// и не меняет содержимое рабочего дерева при просмотре.
pub fn show_diff(app: &mut Program, title: &str, text: &str) {
    let area = app.desktop().group.vs.extent();
    let mut dialog = Dialog::centered((area.width() - 2).clamp(20, 140), (area.height() - 2).max(8), title);
    let vertical = dialog.win.standard_scroll_bar(true);
    let horizontal = dialog.win.standard_scroll_bar(false);
    let mut terminal = Terminal::new(Rect::default(), Some(horizontal), Some(vertical), 20_001);
    let mut lines = text.lines();
    for line in lines.by_ref().take(20_000) {
        terminal.write_line(line);
    }
    if lines.next().is_some() {
        terminal.write_line("… Показаны первые 20 000 строк");
    }
    if text.is_empty() {
        terminal.write_line("Нет изменений для выбранной версии файла.");
    }
    terminal.scroller.delta = Point::new(0, 0);
    let viewer = dialog.add(terminal);
    let ok = dialog.add(Button::new(Rect::default(), "Закрыть", cm::OK, bf::DEFAULT));
    dialog.set_layout(VBox::new().add(item(viewer).expand().stretch(1)).add(ok));
    dialog.select(viewer);
    crate::dialogs::exec(app, dialog);
}

/// Список выбора в одном диалоге, включая двойной щелчок и Enter.
pub fn choose(app: &mut Program, title: &str, rows: Vec<String>, selected: usize) -> Option<usize> {
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
    let (command, dialog) = crate::dialogs::exec(app, dialog);
    if command != cm::OK {
        return None;
    }
    let index = dialog.win.group.find::<ListBox>(list)?.focused;
    (index < count).then_some(index)
}

/// Прямой режим запускает clone сразу, даже когда имя ветки задано вручную.
/// Запрос списка может потребовать отдельной авторизации на сервере.
#[derive(Debug, PartialEq, Eq)]
pub enum CloneBranch {
    Direct(Option<String>),
    List,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CloneRequest {
    pub url: String,
    pub parent: PathBuf,
    pub branch: CloneBranch,
}

pub fn clone_dialog(app: &mut Program, start: &std::path::Path) -> Option<CloneRequest> {
    let mut url = String::new();
    let mut parent = start.to_string_lossy().into_owned();
    // При возврате из выбора папки переносим введённые значения
    // в новый диалог и возвращаем фокус в поле назначения.
    let mut focus_on_parent = false;
    let mut branch_name = String::new();
    loop {
        let width = (app.desktop().group.vs.size.x - 2).clamp(42, 76);
        let mut dialog = Dialog::centered(width, 15, "Клонирование репозитория");
        let mut address = InputLine::new(Rect::new(2, 2, width - 2, 3), 2048);
        address.set_text(&url);
        let address = dialog.add(address);
        dialog.add(Label::new(Rect::new(2, 1, width - 2, 2), "~А~дрес репозитория", Some(address)));
        let mut destination = InputLine::new(Rect::new(2, 5, width - 9, 6), 2048);
        destination.set_text(&parent);
        let destination = dialog.add(destination);
        dialog.add(Label::new(Rect::new(2, 4, width - 2, 5), "~К~аталог назначения", Some(destination)));
        dialog.add(Button::new(Rect::new(width - 8, 5, width - 2, 7), "~…~", cm::YES, bf::NORMAL));
        let mut branch = InputLine::new(Rect::new(2, 9, width - 23, 10), 256);
        branch.set_text(&branch_name);
        let branch = dialog.add(branch);
        dialog.add(Label::new(Rect::new(2, 8, width - 2, 9), "~В~етка (пусто — основная)", Some(branch)));
        dialog.add(Button::new(Rect::new(width / 2 - 15, 12, width / 2, 14), "К~л~онировать", cm::OK, bf::DEFAULT));
        dialog.add(Button::new(Rect::new(width - 21, 8, width - 2, 10), "~С~писок веток...", cm::NO, bf::NORMAL));
        dialog.add(Button::new(Rect::new(width / 2 + 1, 12, width / 2 + 13, 14), "Отмена", cm::CANCEL, bf::NORMAL));
        dialog.add(StaticText::new(Rect::new(2, 10, width - 2, 12), "В каталоге назначения будет создана папка репозитория."));
        dialog.select(if focus_on_parent { destination } else { address });
        let (command, dialog) = crate::dialogs::exec(app, dialog);
        if !matches!(command, cm::YES | cm::OK | cm::NO) {
            return None;
        }
        url = dialog.win.group.find::<InputLine>(address)?.text();
        parent = dialog.win.group.find::<InputLine>(destination)?.text();
        branch_name = dialog.win.group.find::<InputLine>(branch)?.text();
        let folder = if parent.trim().is_empty() { start.to_path_buf() } else { start.join(parent.trim()) };
        if command == cm::YES {
            if let Some(path) = browse_directory(app, folder) {
                parent = path.to_string_lossy().into_owned();
            }
            focus_on_parent = true;
        } else if !folder.is_dir() {
            crate::dialogs::message_box(app, "Выберите существующий каталог назначения.", mf::ERROR | mf::OK_BUTTON);
            focus_on_parent = true;
        } else {
            match crate::git::clone_destination(&folder, &url) {
                Ok(_) => {
                    let branch = if command == cm::NO {
                        CloneBranch::List
                    } else {
                        let name = branch_name.trim();
                        CloneBranch::Direct((!name.is_empty()).then(|| name.to_owned()))
                    };
                    return Some(CloneRequest { url: url.trim().into(), parent: folder, branch });
                }
                Err(error) => {
                    crate::dialogs::message_box(app, &error, mf::ERROR | mf::OK_BUTTON);
                    focus_on_parent = false;
                }
            }
        }
    }
}

/// Просмотр каталога хранит путь локально. Стандартный ChDirDialog
/// меняет текущий каталог всего процесса, что здесь нежелательно.
fn browse_directory(app: &mut Program, start: PathBuf) -> Option<PathBuf> {
    let mut directory = start.ancestors().find(|p| p.is_dir())?.to_path_buf();
    loop {
        let mut children: Vec<_> =
            std::fs::read_dir(&directory).ok()?.filter_map(|entry| entry.ok()).map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
        children.sort();
        let rows = ["Выбрать этот каталог", ".. (выше)", "Диски и корни…"]
            .into_iter()
            .map(str::to_string)
            .chain(children.iter().map(|path| path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
            .collect();
        let title = format!("Каталог — {}", directory.display());
        match choose(app, &title, rows, 0)? {
            0 => return Some(directory),
            1 => {
                directory.pop();
            }
            2 => {
                let roots = available_roots();
                if !roots.is_empty() {
                    let (cmd, dialog) = crate::dialogs::exec(app, DriveDialog::new(&roots, &directory));
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

#[cfg(test)]
mod tests;
