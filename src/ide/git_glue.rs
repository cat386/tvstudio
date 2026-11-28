//! Связь Git с проектом TV Studio, окнами и очередью событий IDE.

use super::*;
use crate::codeedit::CodeWindow;
use crate::designer::DesignWindow;
use crate::git::{self, Branch, Change, Commit, Git, Snapshot, Switch, WorktreeUpdate};
use crate::git_ui::{self, Panel};
use std::sync::mpsc::{Receiver, TryRecvError};

pub(super) fn menu() -> Menu {
    Menu::new()
        .item("~И~зменения", CM_GIT_SHOW)
        .item("~В~ыбрать ветку...", CM_GIT_BRANCH)
        .item("~Н~овая ветка...", CM_GIT_NEW_BRANCH)
        .item("Истори~я~ коммитов...", CM_GIT_HISTORY)
        .item("~С~оздать репозиторий...", CM_GIT_INIT)
        .separator()
        .item("П~у~ть к Git...", CM_GIT_CONFIG)
        .item("Автор: этот ~р~епозиторий...", CM_GIT_IDENTITY_LOCAL)
        .item("Автор: ~г~лобально...", CM_GIT_IDENTITY_GLOBAL)
        .separator()
        .item("Добавить ~o~rigin...", CM_GIT_ORIGIN)
        .item("Получить сс~ы~лки (fetch)", CM_GIT_FETCH)
        .item("~О~бновить ветку (pull)", CM_GIT_PULL)
        .item("Pull с ~с~лиянием…", CM_GIT_PULL_MERGE)
        .item("Слить с ~u~pstream", CM_GIT_MERGE_UPSTREAM)
        .item("R~e~base на upstream", CM_GIT_REBASE_UPSTREAM)
        .item("Пр~о~должить merge/rebase", CM_GIT_CONTINUE)
        .item("~О~тменить merge/rebase", CM_GIT_ABORT)
        .item("О~т~править ветку (push)...", CM_GIT_PUSH)
}

/// Результат связывается с поколением проекта. Смена проекта закрывает
/// старую панель, даже если рабочий поток ещё завершает запрос.
#[derive(Default)]
pub(super) struct State {
    panel: Option<Panel>,
    pending: Option<Receiver<git::Result<Outcome>>>,
    generation: u64,
    running_generation: u64,
}

enum PullMode {
    FastForward,
    Merge,
}

enum Outcome {
    Status(Box<Snapshot>, String),
    Cloned(Box<Snapshot>),
    CloneBranches { dir: PathBuf, destination: PathBuf, url: String, default: Option<String>, branches: Vec<String> },
    Branches(PathBuf, Vec<Branch>),
    History(PathBuf, Vec<Commit>),
    Fetched(PathBuf, String, PullMode),
    Diff(String, String),
    HunkList(Change, Vec<crate::git::Hunk>, bool),
    Executable(String, String),
    Message(String),
    Worktree(Box<WorktreeUpdate>, String),
}

impl State {
    pub(super) fn reset_panel(&mut self, app: &mut Program) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(panel) = self.panel.take() {
            app.desktop_mut().group.remove(panel.window);
        }
    }

    fn panel(&mut self, app: &mut Program) -> &mut Panel {
        if self.panel.as_ref().is_none_or(|p| app.find::<Window>(p.window).is_none()) {
            self.panel = Some(Panel::open(app));
        }
        self.panel.as_mut().unwrap()
    }
}

impl Ide {
    fn git_dir(&self, app: &Program) -> git::Result<PathBuf> {
        if let Some(snapshot) = self.git.panel.as_ref().filter(|p| app.find::<Window>(p.window).is_some()).and_then(|p| p.snapshot.as_ref()) {
            return Ok(snapshot.root.clone());
        }
        self.base_dir().map(Ok).unwrap_or_else(|| std::env::current_dir().map_err(|e| e.to_string()))
    }

    fn git_error(&mut self, app: &mut Program, error: String) {
        self.last_error = Some(error.clone());
        self.say(app, &format!("Git: {error}"));
        if let Some(panel) = &self.git.panel {
            panel.error(app);
        }
    }

    /// Поток владеет своим каталогом и путём к Git. Состояние IDE
    /// меняется только из poll_git в основном потоке интерфейса.
    fn start_git<F>(&mut self, app: &mut Program, dir: PathBuf, label: &str, task: F)
    where
        F: FnOnce(Git<'_>) -> git::Result<Outcome> + Send + 'static,
    {
        let executable = self.cfg.git.clone();
        match git::background(move || task(Git { executable: &executable, cwd: &dir })) {
            Ok(pending) => {
                self.git.pending = Some(pending);
                self.git.running_generation = self.git.generation;
                self.say(app, &format!("Git: {label}…"));
                if let Some(panel) = &self.git.panel {
                    panel.title(app, format!("Git — {label}…"));
                }
            }
            Err(e) => self.git_error(app, e.to_string()),
        }
    }

    pub(super) fn poll_git(&mut self, app: &mut Program) {
        let Some(pending) = &self.git.pending else { return };
        let result = match pending.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("Фоновая задача Git прервана".into()),
        };
        self.git.pending = None;
        if self.git.running_generation != self.git.generation {
            self.say(app, "Git: фоновая операция закончилась для ранее открытого проекта.");
            return;
        }
        if let Some(panel) = &self.git.panel {
            panel.ready_title(app);
        }
        match result {
            Err(e) => self.git_error(app, e),
            Ok(Outcome::Status(snapshot, message)) => {
                if !message.is_empty() {
                    self.say(app, &message);
                }
                if let Some(panel) = &mut self.git.panel {
                    panel.set(app, *snapshot);
                }
                self.tree_dirty = true;
            }
            Ok(Outcome::Cloned(snapshot)) => self.git_open_clone(app, *snapshot),
            Ok(Outcome::CloneBranches { dir, destination, url, default, branches }) => {
                self.git_clone_options(app, dir, destination, url, default, branches);
            }
            Ok(Outcome::Branches(dir, branches)) => {
                self.git_choose_branch(app, dir, branches);
            }
            Ok(Outcome::History(dir, commits)) => {
                if commits.is_empty() {
                    self.say(app, "Git: в текущей ветке ещё нет коммитов.");
                } else if let Some(index) = crate::dialogs::choose_indexed(
                    app,
                    "Git — история текущей ветки",
                    commits.iter().map(|c| c.label.clone()).collect(),
                    0,
                )
                {
                    let oid = commits[index].oid.clone();
                    self.start_git(app, dir, "Чтение коммита", move |git| {
                        let text = git.show_commit(&oid)?;
                        Ok(Outcome::Diff(format!("Git — {}", &oid[..12]), text))
                    });
                }
            }
            Ok(Outcome::Fetched(dir, branch, mode)) => {
                // После фонового fetch пользователь мог изменить документ
                // или сменить ветку снаружи IDE. Проверяем ещё раз перед merge.
                if self.git_worktree_ready(app) {
                    let git = Git { executable: &self.cfg.git, cwd: &dir };
                    match git.snapshot() {
                        Ok(snapshot) if snapshot.branch == branch => {
                            let result = match mode {
                                PullMode::FastForward => git.fast_forward(),
                                PullMode::Merge => git.merge_upstream(),
                            };
                            let ok = match &result {
                                Ok(_) => match mode {
                                    PullMode::FastForward => "Pull (fast-forward) выполнен.",
                                    PullMode::Merge => "Pull (merge) выполнен.",
                                },
                                Err(_) => "Операция не завершена — см. сообщения.",
                            };
                            self.git_worktree_done(app, result, ok);
                        }
                        Ok(_) => self.say(app, "Git: ветка сменилась во время fetch. Повторите pull."),
                        Err(error) => self.git_error(app, error),
                    }
                } else {
                    self.say(app, "Git: fetch завершён. Для обновления файлов повторите pull после сохранения.");
                }
            }
            Ok(Outcome::Diff(title, text)) => {
                git_ui::show_diff(app, &title, &text);
            }
            Ok(Outcome::HunkList(change, hunks, unstage)) => {
                if let Ok(dir) = self.git_dir(app) {
                    self.git_hunks_interactive(app, dir, change, hunks, unstage);
                }
            }
            Ok(Outcome::Executable(path, version)) => {
                self.cfg.git = path;
                if !self.headless {
                    self.cfg.save();
                }
                self.say(app, &format!("Git: {version}"));
            }
            Ok(Outcome::Message(text)) => self.say(app, &text),
            Ok(Outcome::Worktree(update, message)) => {
                self.git_reload_files(app, &update);
                let window = self.git.panel(app);
                window.set(app, update.snapshot);
                if !message.is_empty() {
                    self.say(app, &message);
                }
                self.tree_dirty = true;
            }
        }
    }

    pub(super) fn git_command(&mut self, app: &mut Program, command: u16) -> bool {
        if !(CM_GIT_SHOW..=CM_GIT_PULL_MERGE).contains(&command) {
            return false;
        }
        if self.git.pending.is_some() {
            self.say(app, "Git: дождитесь текущей операции.");
            return true;
        }
        let dir = match self.git_dir(app) {
            Ok(dir) => dir,
            Err(e) => {
                self.git_error(app, e);
                return true;
            }
        };
        match command {
            CM_GIT_SHOW | CM_GIT_REFRESH => {
                self.dismiss_start(app);
                self.msgs(app);
                let id = self.git.panel(app).window;
                app.focus(id);
                self.start_git(app, dir, "Чтение статуса", |git| Ok(status(git.snapshot()?, "")));
            }
            CM_GIT_CONFIG => {
                if let Some(path) = input_box(app, "Настройка Git", "Команда или путь к git:", &self.cfg.git, 1024).filter(|s| !s.trim().is_empty())
                {
                    self.start_git(app, dir, "Проверка Git", move |git| {
                        let version = Git { executable: &path, ..git }.version()?;
                        Ok(Outcome::Executable(path, version))
                    });
                }
            }
            CM_GIT_INIT => {
                if let Some(branch) = input_box(app, "Создать репозиторий", "Имя начальной ветки:", "main", 200).filter(|s| !s.trim().is_empty())
                {
                    self.git.panel(app);
                    self.start_git(app, dir, "Создание репозитория", move |git| {
                        Ok(status(git.init(&branch)?, "Репозиторий создан."))
                    });
                }
            }
            CM_GIT_CLONE => {
                if let Some(request) = git_ui::clone_dialog(app, &self.cfg.default_parent()) {
                    let git_ui::CloneRequest { url, parent, branch } = request;
                    let destination = match git::clone_destination(&parent, &url) {
                        Ok(path) => path,
                        Err(error) => {
                            self.git_error(app, error);
                            return true;
                        }
                    };
                    // Обычный сценарий не зависит от дополнительного
                    // сетевого запроса ls-remote. Список — отдельный режим.
                    match branch {
                        git_ui::CloneBranch::Direct(branch) => self.git_start_clone(app, parent, destination, url, branch),
                        git_ui::CloneBranch::List => {
                            let command_dir = parent.clone();
                            self.start_git(app, parent, "Чтение веток сервера", move |git| {
                                let (default, branches) = git.remote_branches(&url)?;
                                Ok(Outcome::CloneBranches { dir: command_dir, destination, url, default, branches })
                            });
                        }
                    }
                }
            }
            CM_GIT_BRANCH => {
                let selected_dir = dir.clone();
                self.start_git(app, dir, "Чтение веток", move |git| Ok(Outcome::Branches(selected_dir, git.branches()?)));
            }
            CM_GIT_NEW_BRANCH => {
                if self.git_worktree_ready(app) {
                    if let Some(name) = input_box(app, "Новая ветка Git", "Имя ветки:", "", 200).filter(|s| !s.trim().is_empty()) {
                        let result = Git { executable: &self.cfg.git, cwd: &dir }.switch_branch(Switch::Create(name.trim()));
                        self.git_worktree_done(app, result, "Новая ветка создана и открыта.");
                    }
                }
            }
            CM_GIT_PULL => {
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Получение изменений для pull", move |git| {
                        let snapshot = git.snapshot()?;
                        git.fetch_upstream()?;
                        Ok(Outcome::Fetched(snapshot.root, snapshot.branch, PullMode::FastForward))
                    });
                }
            }
            CM_GIT_PULL_MERGE => {
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Pull (fetch + merge)", move |git| {
                        let snapshot = git.snapshot()?;
                        git.fetch_upstream()?;
                        Ok(Outcome::Fetched(snapshot.root, snapshot.branch, PullMode::Merge))
                    });
                }
            }
            CM_GIT_MERGE_UPSTREAM => {
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Слияние с upstream", |git| {
                        Ok(Outcome::Worktree(Box::new(git.merge_upstream()?), "Слияние с upstream.".into()))
                    })
                }
            }
            CM_GIT_REBASE_UPSTREAM => {
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Rebase на upstream", |git| {
                        Ok(Outcome::Worktree(Box::new(git.rebase_upstream()?), "Rebase на upstream.".into()))
                    })
                }
            }
            CM_GIT_CONTINUE => {
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Продолжение Git", |git| {
                        Ok(Outcome::Worktree(Box::new(git.operation_continue()?), "Операция продолжена.".into()))
                    })
                }
            }
            CM_GIT_ABORT => {
                if message_box(app, "Отменить незавершённый merge/rebase?", mf::CONFIRMATION | mf::YES_NO_CANCEL) != cm::YES {
                    return true;
                }
                if self.git_worktree_ready(app) {
                    self.start_git(app, dir, "Отмена merge/rebase", |git| {
                        Ok(Outcome::Worktree(Box::new(git.operation_abort()?), "Операция отменена.".into()))
                    })
                }
            }
            CM_GIT_HISTORY => {
                let selected_dir = dir.clone();
                self.start_git(app, dir, "Чтение истории", move |git| Ok(Outcome::History(selected_dir, git.history()?)));
            }
            CM_GIT_FILTER => {
                self.git.panel(app).cycle_filter(app);
            }
            CM_GIT_OPEN => self.git_open_selected(app, dir),
            CM_GIT_HUNKS => self.git_hunks_load(app, dir, false),
            CM_GIT_HUNKS_STAGED => self.git_hunks_load(app, dir, true),
            CM_GIT_DIFF | CM_GIT_DIFF_STAGED | CM_GIT_STAGE | CM_GIT_UNSTAGE => self.git_selected_command(app, dir, command),
            CM_GIT_COMMIT => {
                let index_ready = self
                    .git
                    .panel
                    .as_ref()
                    .and_then(|p| p.snapshot.as_ref())
                    .is_some_and(|s| s.changes.iter().any(|c| !matches!(c.index, '.' | '?')) && !s.changes.iter().any(|c| c.unmerged));
                if !index_ready {
                    self.say(app, "Git: сначала подготовьте файлы в индекс и разрешите конфликты.");
                    return true;
                }
                if Self::dirty_windows(app) {
                    self.say(app, "Git: есть несохранённые окна. В коммит попадёт текущее содержимое индекса.");
                }
                if let Some(message) = input_box(app, "Коммит из индекса", "Сообщение:", "", 1024).filter(|s| !s.trim().is_empty())
                {
                    self.start_git(app, dir, "Коммит", move |git| {
                        let output = git.commit(&message)?;
                        Ok(status(git.snapshot()?, output.trim()))
                    });
                }
            }
            CM_GIT_IDENTITY_LOCAL | CM_GIT_IDENTITY_GLOBAL => {
                let local = command == CM_GIT_IDENTITY_LOCAL;
                let title = if local { "Автор для репозитория" } else { "Автор Git глобально" };
                if let Some(name) = input_box(app, title, "Имя автора:", "", 250).filter(|s| !s.trim().is_empty()) {
                    if let Some(email) = input_box(app, title, "Email автора:", "", 250).filter(|s| !s.trim().is_empty()) {
                        self.start_git(app, dir, "Настройка автора", move |git| {
                            git.identity(local, &name, &email).map(Outcome::Message)
                        });
                    }
                }
            }
            CM_GIT_ORIGIN => {
                if let Some(url) = git_url(app, "Добавить origin") {
                    self.start_git(app, dir, "Настройка origin", move |git| git.add_origin(&url).map(Outcome::Message));
                }
            }
            CM_GIT_FETCH => {
                self.start_git(app, dir, "Fetch", |git| {
                    let message = git.fetch()?;
                    Ok(status(git.snapshot()?, &message))
                });
            }
            CM_GIT_PUSH => {
                if message_box(app, "Отправить текущую ветку в origin?", mf::CONFIRMATION | mf::YES_NO_CANCEL) == cm::YES {
                    self.start_git(app, dir, "Push", |git| {
                        let message = git.push()?;
                        Ok(status(git.snapshot()?, message.trim()))
                    });
                }
            }
            _ => unreachable!(),
        }
        true
    }

    fn git_clone_options(
        &mut self,
        app: &mut Program,
        dir: PathBuf,
        destination: PathBuf,
        url: String,
        default: Option<String>,
        branches: Vec<String>,
    ) {
        let default = default.map_or_else(|| "Ветка по умолчанию на сервере".into(), |name| format!("Ветка по умолчанию — {name}"));
        let rows = std::iter::once(default).chain(branches.iter().cloned()).collect();
        let Some(index) = crate::dialogs::choose_indexed(app, "Клонирование — выберите ветку", rows, 0) else { return };
        let branch = index.checked_sub(1).map(|i| branches[i].clone());
        self.git_start_clone(app, dir, destination, url, branch);
    }

    fn git_start_clone(&mut self, app: &mut Program, dir: PathBuf, destination: PathBuf, url: String, branch: Option<String>) {
        self.start_git(app, dir, "Клонирование", move |git| {
            Ok(Outcome::Cloned(Box::new(git.clone_into(&url, &destination, branch.as_deref())?)))
        });
    }

    /// Приоритет у сохранённого проекта. Если в корне только Cargo.toml,
    /// IDE создаёт свой файл проекта и открывает точку входа Cargo.
    fn git_open_clone(&mut self, app: &mut Program, snapshot: Snapshot) {
        let path = &snapshot.root;
        self.say(app, &format!("Репозиторий клонирован в {}", path.display()));
        let project_file = if let Some(file) = find_project_file(path) {
            Some(file)
        } else if path.join("Cargo.toml").is_file() {
            match Project::adopt_rust(path) {
                Ok(project) => Some(project.file()),
                Err(error) => {
                    self.git_error(app, error);
                    None
                }
            }
        } else {
            None
        };
        if let Some(file) = project_file {
            self.open_project(app, &file);
        } else {
            self.dismiss_start(app);
            let panel = self.git.panel(app);
            panel.set(app, snapshot);
            app.focus(panel.window);
            self.say(app, "Git: для этого каталога открыто окно репозитория.");
        }
    }

    fn git_choose_branch(&mut self, app: &mut Program, dir: PathBuf, branches: Vec<Branch>) {
        if branches.is_empty() {
            self.say(app, "Git: список веток пуст. Сделайте первый коммит или выполните fetch.");
            return;
        }
        let rows = branches
            .iter()
            .map(|branch| {
                let mark = if branch.current { "*" } else { " " };
                let suffix = if branch.remote { "  [удалённая]" } else { "" };
                format!("{mark} {}{suffix}", branch.name)
            })
            .collect();
        let selected = branches.iter().position(|branch| branch.current).unwrap_or(0);
        let Some(index) = crate::dialogs::choose_indexed(app, "Git — выбрать ветку", rows, selected) else { return };
        let branch = &branches[index];
        if branch.current || !self.git_worktree_ready(app) {
            return;
        }
        let target = if branch.remote { Switch::Track(&branch.name) } else { Switch::Local(&branch.name) };
        let result = Git { executable: &self.cfg.git, cwd: &dir }.switch_branch(target);
        self.git_worktree_done(app, result, &format!("Открыта ветка {}.", branch.name));
    }

    /// Git работает с диском. Изменённый буфер мог бы остаться открытым
    /// поверх новой версии файла и впоследствии её перезаписать.
    fn git_worktree_ready(&mut self, app: &mut Program) -> bool {
        if self.job.is_some() {
            self.say(app, "Git: дождитесь завершения сборки.");
            return false;
        }
        if Self::dirty_windows(app) {
            self.say(app, "Git: сохраните изменения в редакторах и формах перед сменой файлов.");
            return false;
        }
        true
    }

    fn git_worktree_done(&mut self, app: &mut Program, result: git::Result<WorktreeUpdate>, message: &str) {
        match result {
            Ok(update) => {
                self.git_reload_files(app, &update);
                let window = self.git.panel(app);
                window.set(app, update.snapshot);
                app.focus(window.window);
                self.say(app, message);
            }
            Err(error) => self.git_error(app, error),
        }
    }

    /// Перечитываем только затронутые окна. При удалении файла его
    /// старое окно закрывается, чтобы случайно не вернуть файл при сохранении.
    fn git_reload_files(&mut self, app: &mut Program, update: &WorktreeUpdate) {
        let changed_paths = update.changed.as_ref().map(|paths| paths.iter().map(|path| update.snapshot.root.join(path)).collect::<Vec<_>>());
        let affected = |path: &Path| {
            !path.as_os_str().is_empty()
                && changed_paths.as_ref().is_none_or(|paths| {
                    paths.iter().any(|candidate| {
                        same_file(candidate, path)
                            || (cfg!(windows)
                                && crate::model::cargo_path(&candidate.to_string_lossy())
                                    .eq_ignore_ascii_case(&crate::model::cargo_path(&path.to_string_lossy())))
                    })
                })
        };

        let project_file = self.project.as_ref().map(Project::file).filter(|path| affected(path));
        if changed_paths.as_ref().is_some_and(Vec::is_empty) {
            return;
        }
        // Сначала обновляем модель проекта, затем запускаем анализаторы
        // с новой конфигурацией и открываем перечитанные документы.
        self.lsp_stop(app);
        if let Some(file) = project_file {
            match Project::load(&file) {
                Ok(project) => self.project = Some(project),
                Err(error) => {
                    self.project = None;
                    self.say(app, &format!("Git: проект не удалось открыть после смены файлов — {error}"));
                }
            }
        }
        self.tree_dirty = true;
        self.words_cache = None;
        self.build_diags.clear();
        self.marks.clear();
        self.problems_dirty = true;
        self.ensure_tools(app);
        self.lsp_start(app);

        for id in Self::windows::<CodeWindow>(app) {
            let path = app.find::<CodeWindow>(id).unwrap().path();
            if affected(&path) {
                app.desktop_mut().group.remove(id);
                if path.is_file() {
                    self.open_file(app, &path, None);
                } else {
                    self.say(app, &format!("Git: закрыт исчезнувший файл {}", path.display()));
                }
            }
        }
        for id in Self::windows::<DesignWindow>(app) {
            let (name, path) = {
                let doc = app.find::<DesignWindow>(id).unwrap().doc.borrow();
                (doc.form.name.clone(), doc.path.clone())
            };
            let in_project = self.project.as_ref().is_some_and(|p| p.forms.contains(&name));
            if affected(&path) || !in_project {
                app.desktop_mut().group.remove(id);
                if in_project && path.is_file() {
                    self.open_form(app, &name);
                }
            }
        }
    }

    fn git_hunks_load(&mut self, app: &mut Program, dir: PathBuf, from_index: bool) {
        let Some(panel) = &self.git.panel else { return };
        let Some(change) = panel.selected(app).cloned() else {
            self.say(app, "Git: выберите файл в списке.");
            return;
        };
        if change.index == '?' && !from_index {
            self.say(app, "Git: для нового файла используйте «+ В индекс» целиком.");
            return;
        }
        if change.unmerged {
            self.say(app, "Git: сначала разрешите конфликт.");
            return;
        }
        if from_index && matches!(change.index, '.' | '?') {
            self.say(app, "Git: в индексе нет подготовленных изменений для этого файла.");
            return;
        }
        if !from_index && change.worktree == '.' {
            let msg = if change.index != '.' && change.index != '?' {
                "Git: в рабочей копии нет новых правок (есть только индекс — «−Блоки»)."
            } else {
                "Git: нет неподготовленных правок. Для индекса — «−Блоки»."
            };
            self.say(app, msg);
            return;
        }
        let label = if from_index { "Чтение блоков индекса" } else { "Чтение блоков diff" };
        self.start_git(app, dir, label, move |git| {
            let text = git.diff(&change, from_index)?;
            let hunks = crate::git::parse_patch_hunks(&text);
            if hunks.is_empty() {
                let msg = if from_index {
                    "Git: в индексе нет блоков для выборочного снятия."
                } else {
                    "Git: в файле нет блоков для выборочной подготовки."
                };
                return Ok(Outcome::Message(msg.into()));
            }
            Ok(Outcome::HunkList(change, hunks, from_index))
        });
    }

    fn git_hunks_interactive(
        &mut self,
        app: &mut Program,
        dir: PathBuf,
        change: Change,
        mut hunks: Vec<crate::git::Hunk>,
        unstage: bool,
    ) {
        let mut changed = false;
        loop {
            match git_ui::pick_hunk(app, &change.path, &hunks, unstage) {
                git_ui::HunkPick::Close => break,
                git_ui::HunkPick::View(i) => {
                    git_ui::show_diff(app, &format!("Блок {}", i + 1), &hunks[i].patch);
                }
                git_ui::HunkPick::Apply(i) => {
                    let patch = hunks[i].patch.clone();
                    let git = Git { executable: &self.cfg.git, cwd: &dir };
                    match git.apply_hunk(&patch, unstage) {
                        Ok(()) => {
                            hunks.remove(i);
                            changed = true;
                            let msg = if unstage {
                                format!("Git: блок {} снят с индекса.", i + 1)
                            } else {
                                format!("Git: блок {} подготовлен в индекс.", i + 1)
                            };
                            self.say(app, &msg);
                        }
                        Err(error) => {
                            let what = if unstage { "снять" } else { "подготовить" };
                            self.git_error(app, format!("Git: не удалось {what} блок — {error}"));
                            break;
                        }
                    }
                }
            }
        }
        if changed {
            if let Ok(snapshot) = (Git { executable: &self.cfg.git, cwd: &dir }).snapshot() {
                if let Some(panel) = &mut self.git.panel {
                    panel.set(app, snapshot);
                }
            }
        }
    }

    fn git_open_selected(&mut self, app: &mut Program, dir: PathBuf) {
        let Some(panel) = &self.git.panel else { return };
        let Some(change) = panel.selected(app).cloned() else {
            self.say(app, "Git: выберите файл в списке.");
            return;
        };
        let root = panel.snapshot.as_ref().map(|s| s.root.clone()).unwrap_or(dir);
        let path = root.join(&change.path);
        if !path.is_file() {
            self.say(app, &format!("Git: файл не найден — {}", path.display()));
            return;
        }
        let at = if change.unmerged { git_ui::conflict_marker_line(&path).map(|line| (line, 1)) } else { None };
        self.open_file(app, &path, at);
        if change.unmerged && at.is_none() {
            self.say(app, "Git: маркеры конфликта не найдены — открыт весь файл.");
        }
    }

    fn git_selected_command(&mut self, app: &mut Program, dir: PathBuf, command: u16) {
        let Some(panel) = &self.git.panel else { return };
        let Some(change) = panel.selected(app).cloned() else { return };
        if command == CM_GIT_DIFF || command == CM_GIT_DIFF_STAGED {
            let staged = command == CM_GIT_DIFF_STAGED;
            self.start_git(app, dir, "Чтение diff", move |git| {
                let title = if staged { "Diff индекса" } else { "Diff рабочего дерева" };
                Ok(Outcome::Diff(title.into(), git.diff(&change, staged)?))
            });
        } else if command == CM_GIT_STAGE {
            if Self::dirty_windows(app) {
                self.say(app, "Git: индекс обновляется из файлов на диске. Несохранённые окна в индекс не попадут.");
            }
            self.start_git(app, dir, "Подготовка файла", move |git| {
                git.stage(&change.path)?;
                Ok(status(git.snapshot()?, "Файл подготовлен в индекс."))
            });
        } else if !matches!(change.index, '.' | '?') {
            self.start_git(app, dir, "Снятие подготовки", move |git| {
                git.unstage(&change.path, git.snapshot()?.initial)?;
                Ok(status(git.snapshot()?, "Файл снят с подготовки."))
            });
        }
    }
}

fn status(snapshot: Snapshot, message: &str) -> Outcome {
    Outcome::Status(Box::new(snapshot), message.into())
}

fn git_url(app: &mut Program, title: &str) -> Option<String> {
    let url = input_box(app, title, "URL или путь:", "", 1024)?;
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    if let Some(address) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        if address.split('/').next().is_some_and(|authority| authority.contains('@')) {
            message_box(app, "Введите URL без учётных данных. Для HTTPS Git использует credential helper.", mf::ERROR | mf::OK_BUTTON);
            return None;
        }
    }
    Some(url.into())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod clone_ui_tests;
