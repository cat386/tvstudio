//! LFM на tvision-rs — двухпанельный файловый менеджер по мотивам
//! [Last File Manager](https://inigo.katxi.org/devel/lfm/) (Iñigo Serna).
//!
//! Запуск: `cargo run --example lfm [левый_каталог] [правый_каталог]`
//! (`--path-file ФАЙЛ` — при выходе записать в файл текущий каталог, чтобы
//! оболочка могла в него перейти, как функция `lfm` из README оригинала).
//!
//! Виджеты: [`DualPanel`] / [`FilePanel`] (панели), [`tvision::fileops`]
//! (копирование, перенос и удаление в отдельном потоке с прогрессом),
//! [`ProgressBar`], [`EditWindow`] (встроенный редактор), [`Terminal`]
//! (просмотр и вывод команд), [`ChDirDialog`] (дерево каталогов).
//!
//! Клавиши — как в lfm:
//!
//! | Клавиши | Действие |
//! |---------|----------|
//! | ↑ ↓ PgUp PgDn Home End, Ctrl+↑/↓ | движение |
//! | ← / Backspace, → / Enter | наверх, в каталог; Enter на файле — открыть |
//! | Tab | другая панель; `.` — одна/две панели; `=` — тот же каталог; `,` или Ctrl+U — поменять местами |
//! | `:` `!` `<` `>` | вкладки: новая, закрыть, влево, вправо |
//! | Ins, пробел, `+`, `-`, `*` | выделение: файл, по шаблону, снять, инвертировать |
//! | F2 F3 F4 F5 F6 F7 F8 | имя, просмотр, правка, копия, перенос, каталог, удалить |
//! | `g`, Ctrl+T, Alt+F1/F2 | перейти по пути, дерево, корни/диски левой/правой панели |
//! | `b` / `B` + 0–9, a–z; Ctrl+D | закладки: перейти, запомнить; список |
//! | Ctrl+Y | история каталогов вкладки |
//! | Ctrl+H, Ctrl+F, `s`, `#` | скрытые файлы, фильтр, сортировка, размеры каталогов |
//! | `/` | поиск файлов (и текста в них) → виртуальная панель |
//! | `i`, `t`, `l`, `@` | сведения, создать файл, ссылка, команда над файлом |
//! | Ctrl+X | PowerCLI — командная строка с подстановками (см. ниже) |
//! | Ctrl+O | оболочка ОС; F1 / `h` — справка; F10 / `q` — выход; Ctrl+Q — выход без смены каталога |
//!
//! **PowerCLI** (Ctrl+X, Enter — выполнить, Esc — назад к панелям):
//! `$f` — файл, `$F` — полный путь, `$E` — имя без расширения, `$e` —
//! расширение, `$p` — каталог, `$o` — каталог другой панели, `$s` —
//! выделенные, `$a` — все файлы, `$i` — номер в цикле, `$b0`…`$bz` —
//! закладки, `$tm`/`$dm` — время/дата изменения, `$tn`/`$dn` — сейчас.
//! Если выделены файлы и в команде есть `$f`, `$F`, `$E`, `$e` или `$i`,
//! команда выполняется для каждого выделенного. Подставленные значения
//! **уже в кавычках** (в отличие от lfm, имя с кавычками или `$(…)` не
//! выполнится как код). В конце `&` — запуск в фоне, `%` — интерактивная
//! программа (терминал отдаётся ей), иначе — вывод показывается в окне.
//! Вставки Python `{…}` из lfm не поддерживаются.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use tvision::fileops::{self, Conflict, FileJob, JobKind};
use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;

// ----------------------------------------------------------------------
// Команды
// ----------------------------------------------------------------------

const CM_QUIT_NOCD: u16 = 108;
const CM_GOTO: u16 = 110;
const CM_TREE: u16 = 111;
const CM_BOOKMARKS: u16 = 112;
const CM_HISTORY: u16 = 113;
const CM_FIND: u16 = 114;
const CM_POWERCLI: u16 = 115;
const CM_SHELL: u16 = 116;
const CM_FILTER: u16 = 117;
const CM_HIDDEN: u16 = 118;
const CM_SORT: u16 = 119;
const CM_DIRSIZE: u16 = 120;
const CM_SINGLE: u16 = 121;
const CM_SWAP: u16 = 122;
const CM_SYNC: u16 = 123;
const CM_TAB_NEW: u16 = 124;
const CM_TAB_CLOSE: u16 = 125;
const CM_TAB_LEFT: u16 = 126;
const CM_TAB_RIGHT: u16 = 127;
const CM_MARK: u16 = 128;
const CM_UNMARK: u16 = 129;
const CM_INVERT: u16 = 130;
const CM_INFO: u16 = 131;
const CM_TOUCH: u16 = 132;
const CM_LINK: u16 = 133;
const CM_EXEC_ON: u16 = 134;
const CM_REFRESH: u16 = 135;
const CM_ROOTS_LEFT: u16 = 136;
const CM_ROOTS_RIGHT: u16 = 137;
const CM_ABOUT: u16 = 138;

/// Номер истории строки PowerCLI.
const HIST_CLI: u16 = 70;
/// Номер истории диалога «Перейти».
const HIST_GOTO: u16 = 71;
/// Сколько найденных файлов показывать в виртуальной панели.
const FIND_LIMIT: usize = 5000;

// ----------------------------------------------------------------------
// Командная строка (PowerCLI)
// ----------------------------------------------------------------------

/// Палитра строки PowerCLI внутри рабочего стола: строка ввода (цвета
/// 19–21 у владельца) — как текст синего окна, подсказка (цвет 6) — как
/// горячая буква меню, значок истории (22–23) — как значки рамки.
const CLI_PALETTE: &[u8] = b"\x0D\x0D\x0D\x0D\x0D\x04\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0D\x0E\x0A\x0A\x0A";

/// Строка внизу экрана: подсказка, строка ввода, значок истории.
struct CommandLine {
    group: Group,
    input: ViewId,
}

impl CommandLine {
    fn new(r: Rect) -> Self {
        let mut group = Group::new(r);
        group.vs.options |= of::SELECTABLE;
        group.vs.grow_mode = gf::GROW_LO_Y | gf::GROW_HI_Y | gf::GROW_HI_X;
        let w = r.width();
        group.add(StaticText::new(Rect::new(0, 0, 8, 1), "PowerCLI"));
        let mut il = InputLine::new(Rect::new(9, 0, w - 3, 1), 1024);
        il.history_id = Some(HIST_CLI);
        il.vs.grow_mode = gf::GROW_HI_X;
        let input = group.add(il);
        let mut h = History::new(Rect::new(w - 3, 0, w, 1), input, HIST_CLI);
        h.vs.grow_mode = gf::GROW_LO_X | gf::GROW_HI_X;
        group.add(h);
        group.select_id(input, &mut Ctx::new());
        CommandLine { group, input }
    }
}

impl View for CommandLine {
    tvision::impl_group_basics!(group);

    fn draw(&self, dc: &mut DrawCtx) {
        // Фон строки — цвет поля ввода, иначе в промежутках виден рабочий стол.
        let bg = dc.color(19);
        dc.fill(self.group.vs.extent(), ' ', bg);
        self.group.draw_children(dc);
    }

    fn palette(&self) -> &'static [u8] {
        CLI_PALETTE
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        self.group.handle_group_event(ev, ctx);
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.group.set_group_state(flag, enable);
    }
}

// ----------------------------------------------------------------------
// Приложение
// ----------------------------------------------------------------------

/// Чего ждём после `b` / `B`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    GotoBookmark,
    SetBookmark,
}

struct Lfm {
    panels: ViewId,
    cli_input: ViewId,
    /// Закладки 0–9, a–z.
    bookmarks: HashMap<char, PathBuf>,
    pending: Option<Pending>,
    /// Время изменения каталогов панелей (для автообновления).
    seen: HashMap<ViewId, Option<SystemTime>>,
    last_check: Instant,
    /// Файл, в который при выходе пишется текущий каталог.
    path_file: Option<PathBuf>,
    /// Последний текущий каталог процесса (= каталог активной панели).
    cwd: PathBuf,
}

impl Lfm {
    /// Построить интерфейс на рабочем столе программы.
    fn build(app: &mut Program, left: &Path, right: &Path) -> Lfm {
        let r = app.desktop().group.vs.extent();
        let dp = DualPanel::new(Rect::new(0, 0, r.b.x, r.b.y - 1), left, right);
        let cl = CommandLine::new(Rect::new(0, r.b.y - 1, r.b.x, r.b.y));
        let cli_input = cl.input;
        app.insert_window(cl);
        let panels = app.insert_window(dp);
        let mut lfm = Lfm {
            panels,
            cli_input,
            bookmarks: load_bookmarks(),
            pending: None,
            seen: HashMap::new(),
            last_check: Instant::now(),
            path_file: None,
            cwd: PathBuf::new(),
        };
        lfm.sync_cwd(app);
        lfm
    }

    fn dp<'a>(&self, app: &'a mut Program) -> &'a mut DualPanel {
        app.find_mut::<DualPanel>(self.panels).expect("панели")
    }

    fn panel<'a>(&self, app: &'a mut Program) -> &'a mut FilePanel {
        self.dp(app).active_mut()
    }

    fn other<'a>(&self, app: &'a mut Program) -> &'a mut FilePanel {
        self.dp(app).other_mut()
    }

    fn cli_focused(&self, app: &Program) -> bool {
        app.find::<InputLine>(self.cli_input).map(|i| i.vs.is_focused()).unwrap_or(false)
    }

    /// Текущий каталог процесса = каталог активной панели (для оболочки,
    /// дерева каталогов и команд).
    fn sync_cwd(&mut self, app: &mut Program) {
        let p = self.panel(app).path().to_path_buf();
        if p != self.cwd && std::env::set_current_dir(&p).is_ok() {
            self.cwd = p;
        }
    }

    /// Перечитать обе панели.
    fn refresh_all(&mut self, app: &mut Program) {
        let dp = self.dp(app);
        dp.active_mut().refresh();
        dp.other_mut().refresh();
        self.seen.clear();
    }

    /// Сообщение в строке сведений активной панели.
    fn note(&self, app: &mut Program, msg: &str) {
        self.panel(app).set_message(msg);
    }

    // --- файловые операции ---

    fn rename(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) if !e.is_parent() => e,
            _ => return,
        };
        let name = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(new) = input_box(app, "Переименовать", "~Н~овое имя:", &name, 255) {
            let new = new.trim();
            if new.is_empty() || new == name {
                return;
            }
            let dest = e.path.parent().unwrap_or(Path::new(".")).join(new);
            if dest.exists() {
                error_box(app, &format!("«{}» уже существует.", new));
                return;
            }
            match std::fs::rename(&e.path, &dest) {
                Ok(()) => {
                    self.refresh_all(app);
                    self.panel(app).select_path(&dest);
                }
                Err(err) => error_box(app, &format!("Не удалось переименовать:\n{}", err)),
            }
        }
    }

    fn mkdir(&mut self, app: &mut Program) {
        if let Some(name) = input_box(app, "Создать каталог", "~И~мя:", "", 255) {
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            let path = self.panel(app).path().join(name);
            match std::fs::create_dir_all(&path) {
                Ok(()) => {
                    self.refresh_all(app);
                    self.panel(app).select_path(&path);
                }
                Err(e) => error_box(app, &format!("Не удалось создать каталог:\n{}", e)),
            }
        }
    }

    fn touch(&mut self, app: &mut Program) {
        if let Some(name) = input_box(app, "Создать файл", "~И~мя:", "", 255) {
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            let path = self.panel(app).path().join(name);
            let r = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|f| f.set_modified(SystemTime::now()));
            match r {
                Ok(()) => {
                    self.refresh_all(app);
                    self.panel(app).select_path(&path);
                }
                Err(e) => error_box(app, &format!("Не удалось создать файл:\n{}", e)),
            }
        }
    }

    fn link(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) if !e.is_parent() => e,
            _ => return,
        };
        let name = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let default = self.other(app).path().join(&name).display().to_string();
        if let Some(target) = input_box(app, "Создать ссылку", &format!("~С~сылка на «{}»:", name), &default, 1024) {
            let target = PathBuf::from(target.trim());
            #[cfg(unix)]
            let r = std::os::unix::fs::symlink(&e.path, &target);
            #[cfg(windows)]
            let r =
                if e.is_dir() { std::os::windows::fs::symlink_dir(&e.path, &target) } else { std::os::windows::fs::symlink_file(&e.path, &target) };
            #[cfg(not(any(unix, windows)))]
            let r: std::io::Result<()> = Err(std::io::Error::other("не поддерживается"));
            match r {
                Ok(()) => self.refresh_all(app),
                Err(err) => error_box(app, &format!("Не удалось создать ссылку:\n{}", err)),
            }
        }
    }

    /// F5 / F6: копировать или перенести выделенное в другую панель.
    fn copy_move(&mut self, app: &mut Program, kind: JobKind) {
        let sources = self.panel(app).targets();
        if sources.is_empty() {
            return;
        }
        let verb = if kind == JobKind::Copy { "Копировать" } else { "Перенести" };
        let what = describe(&sources);
        let default = self.other(app).path().display().to_string();
        let dest = match input_box(app, verb, &format!("~{}~ {} в:", &verb[..verb.char_indices().nth(1).unwrap().0], what), &default, 1024) {
            Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
            _ => return,
        };
        let dest = if dest.is_absolute() { dest } else { self.panel(app).path().join(dest) };
        if sources.len() > 1 && !dest.is_dir() {
            error_box(app, "Несколько файлов можно копировать только в каталог.");
            return;
        }
        let mut conflict = Conflict::Overwrite;
        let existing = fileops::conflicts(&sources, &dest);
        if !existing.is_empty() {
            let msg = format!("Уже существует: {} (например, «{}»).\n\nЗаменить?", existing.len(), file_name(&existing[0]));
            match message_box(app, &msg, mf::CONFIRMATION | mf::YES_NO_CANCEL) {
                c if c == cm::YES => conflict = Conflict::Overwrite,
                c if c == cm::NO => conflict = Conflict::Skip,
                _ => return,
            }
        }
        let p = run_job(app, verb, kind, sources, Some(dest), conflict);
        self.panel(app).clear_marks();
        self.refresh_all(app);
        report_job(app, verb, &p);
    }

    fn delete(&mut self, app: &mut Program) {
        let targets = self.panel(app).targets();
        if targets.is_empty() {
            return;
        }
        let msg = format!("Удалить {}?", describe(&targets));
        if message_box(app, &msg, mf::CONFIRMATION | mf::YES_BUTTON | mf::NO_BUTTON) != cm::YES {
            return;
        }
        let p = run_job(app, "Удаление", JobKind::Delete, targets, None, Conflict::Overwrite);
        self.panel(app).clear_marks();
        self.refresh_all(app);
        report_job(app, "Удаление", &p);
    }

    // --- просмотр и правка ---

    fn view(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) if !e.is_dir() => e,
            _ => return,
        };
        view_file(app, &e.path);
    }

    fn edit(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) if !e.is_dir() => e,
            _ => return,
        };
        let r = app.desktop().group.vs.extent();
        let mut w = EditWindow::new(r, &e.path, 0);
        w.win.group.vs.options |= of::TILEABLE;
        app.insert_window(w);
    }

    /// Enter на файле: программа — выполнить, текст — показать, остальное —
    /// открыть программой системы (как `[Programs]` в lfm.ini).
    fn execute(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) if !e.is_dir() => e,
            _ => return,
        };
        if e.executable {
            let cmd = quote(&e.path.display().to_string());
            let dir = self.panel(app).path().to_path_buf();
            run_and_show(app, &cmd, &dir);
            self.refresh_all(app);
            return;
        }
        match e.class() {
            FileClass::Source | FileClass::Data | FileClass::Normal | FileClass::Temp => view_file(app, &e.path),
            FileClass::Document if e.extension() == "txt" || e.extension() == "md" => view_file(app, &e.path),
            _ => {
                if let Err(err) = open_with_system(&e.path) {
                    error_box(app, &format!("Не удалось открыть файл:\n{}", err));
                }
            }
        }
    }

    fn info(&mut self, app: &mut Program) {
        let e = match self.panel(app).current().cloned() {
            Some(e) => e,
            None => return,
        };
        let kind = match e.kind {
            FileKind::Parent | FileKind::Dir => "каталог",
            FileKind::File if e.executable => "программа",
            FileKind::File => "файл",
            FileKind::Link { .. } => "ссылка",
            FileKind::Other => "особый файл",
        };
        let size = if e.is_dir() {
            let (b, n) = fileops::dir_size(&e.path);
            format!("{} байт в {} файлах", format_size_full(b), n)
        } else {
            format!("{} байт", format_size_full(e.size))
        };
        let mut text = format!(
            "{}\n\nТип: {}\nРазмер: {}\nПрава: {}\nИзменён: {}",
            e.path.display(),
            kind,
            size,
            e.perms_string(),
            format_time_full(e.modified)
        );
        if let FileKind::Link { .. } = e.kind {
            if let Ok(t) = std::fs::read_link(&e.path) {
                text.push_str(&format!("\nУказывает на: {}", t.display()));
            }
        }
        message_box_rect(app, Rect::new(0, 0, 60, 14), &text, mf::INFORMATION | mf::OK_BUTTON, true);
    }

    // --- переходы ---

    fn goto(&mut self, app: &mut Program) {
        let cur = self.panel(app).path().display().to_string();
        if let Some(p) = input_box_hist(app, "Перейти", "~К~аталог:", &cur, HIST_GOTO) {
            let p = expand_home(p.trim());
            let p = if p.is_absolute() { p } else { self.panel(app).path().join(p) };
            if let Err(e) = self.panel(app).set_path(&p) {
                error_box(app, &e);
            }
        }
    }

    fn tree(&mut self, app: &mut Program) {
        self.sync_cwd(app);
        let (c, d) = app.exec_dialog(ChDirDialog::new(cd::NORMAL, HIST_GOTO));
        if c == cm::OK {
            let dir = d.directory();
            if let Err(e) = self.panel(app).set_path(&dir) {
                error_box(app, &e);
            }
        }
    }

    fn roots(&mut self, app: &mut Program, right: bool) {
        let roots = available_roots();
        let items: Vec<String> = roots.iter().map(|p| p.display().to_string()).collect();
        if let Some(i) = choose(app, if cfg!(windows) { "Диски" } else { "Корни и точки монтирования" }, &items) {
            let dp = self.dp(app);
            let id = if right { dp.right_id() } else { dp.left_id() };
            if let Some(p) = dp.panel_mut(id) {
                let _ = p.set_path(&roots[i]);
            }
            app.focus(id);
        }
    }

    fn history(&mut self, app: &mut Program) {
        let mut h: Vec<PathBuf> = self.panel(app).history().to_vec();
        h.reverse();
        if h.is_empty() {
            self.note(app, "История вкладки пуста");
            return;
        }
        let items: Vec<String> = h.iter().map(|p| p.display().to_string()).collect();
        if let Some(i) = choose(app, "История каталогов", &items) {
            let _ = self.panel(app).set_path(&h[i]);
        }
    }

    fn bookmark_key(&mut self, app: &mut Program, mode: Pending, c: char) {
        let c = c.to_ascii_lowercase();
        if !c.is_ascii_alphanumeric() {
            self.note(app, "Закладки — 0–9 и a–z");
            return;
        }
        match mode {
            Pending::SetBookmark => {
                let p = self.panel(app).path().to_path_buf();
                self.note(app, &format!("Закладка {}: {}", c, p.display()));
                self.bookmarks.insert(c, p);
                save_bookmarks(&self.bookmarks);
            }
            Pending::GotoBookmark => match self.bookmarks.get(&c).cloned() {
                Some(p) => {
                    if let Err(e) = self.panel(app).set_path(&p) {
                        error_box(app, &e);
                    }
                }
                None => self.note(app, &format!("Закладка {} не задана", c)),
            },
        }
    }

    fn bookmarks_list(&mut self, app: &mut Program) {
        let mut keys: Vec<char> = self.bookmarks.keys().copied().collect();
        keys.sort();
        if keys.is_empty() {
            self.note(app, "Закладок нет: B и буква — запомнить каталог");
            return;
        }
        let items: Vec<String> = keys.iter().map(|k| format!("{}  {}", k, self.bookmarks[k].display())).collect();
        if let Some(i) = choose(app, "Закладки", &items) {
            let p = self.bookmarks[&keys[i]].clone();
            if let Err(e) = self.panel(app).set_path(&p) {
                error_box(app, &e);
            }
        }
    }

    // --- выделение, фильтр, сортировка ---

    fn mark(&mut self, app: &mut Program, on: bool) {
        let title = if on { "Выделить" } else { "Снять выделение" };
        if let Some(p) = input_box(app, title, "~Ш~аблон:", "*", 255) {
            let n = self.panel(app).mark_glob(p.trim(), on);
            self.note(app, &format!("{}: {}", if on { "Выделено" } else { "Снято" }, n));
        }
    }

    fn filter(&mut self, app: &mut Program) {
        let cur = self.panel(app).filter().to_string();
        if let Some(f) = input_box(app, "Фильтр вкладки", "~С~крыть (*.o,*.tmp,!исключение):", &cur, 255) {
            self.panel(app).set_filter(&f);
        }
    }

    fn sort(&mut self, app: &mut Program) {
        let (mode, rev, mix, cs) = {
            let p = self.panel(app);
            (p.sort, p.sort_reverse, p.sort_mix_dirs, p.sort_case_sensitive)
        };
        let mut d = Dialog::centered(44, 13, "Сортировка");
        let mut r = Cluster::radio_buttons(Rect::new(3, 3, 22, 8), &["~И~мя", "~Р~асширение", "Ра~з~мер", "~В~ремя", "~Б~ез сортировки"]);
        r.value = SortMode::ALL.iter().position(|&m| m == mode).unwrap_or(0) as u32;
        let rid = d.add(r);
        d.add(Label::new(Rect::new(2, 2, 20, 3), "~П~орядок", Some(rid)));
        let mut c = Cluster::check_boxes(Rect::new(23, 3, 42, 6), &["~О~братный", "~К~аталоги с файлами", "Р~е~гистр"]);
        c.value = rev as u32 | (mix as u32) << 1 | (cs as u32) << 2;
        let cid = d.add(c);
        d.add(Button::new(Rect::new(9, 10, 21, 12), "O~K~", cm::OK, bf::DEFAULT));
        d.add(Button::new(Rect::new(23, 10, 35, 12), "Отмена", cm::CANCEL, bf::NORMAL));
        d.select(rid);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::OK {
            return;
        }
        let m = d.win.group.find::<Cluster>(rid).map(|c| c.value).unwrap_or(0) as usize;
        let f = d.win.group.find::<Cluster>(cid).map(|c| c.value).unwrap_or(0);
        let p = self.panel(app);
        p.sort_mix_dirs = f & 2 != 0;
        p.sort_case_sensitive = f & 4 != 0;
        p.set_sort(SortMode::ALL[m.min(4)], f & 1 != 0);
    }

    fn dir_sizes(&mut self, app: &mut Program) {
        let total = self.panel(app).calc_dir_sizes();
        self.note(app, &format!("Размер каталогов: {} байт", format_size_full(total)));
    }

    // --- поиск ---

    fn find(&mut self, app: &mut Program) {
        let mut d = Dialog::centered(56, 11, "Поиск файлов");
        let mut pat = InputLine::new(Rect::new(3, 3, 53, 4), 255);
        pat.set_text("*");
        let pid = d.add(pat);
        d.add(Label::new(Rect::new(2, 2, 40, 3), "~И~мя (шаблоны через запятую):", Some(pid)));
        let tid = d.add(InputLine::new(Rect::new(3, 6, 53, 7), 255));
        d.add(Label::new(Rect::new(2, 5, 40, 6), "~С~одержит текст (необязательно):", Some(tid)));
        d.add(Button::new(Rect::new(14, 8, 26, 10), "~И~скать", cm::OK, bf::DEFAULT));
        d.add(Button::new(Rect::new(29, 8, 41, 10), "Отмена", cm::CANCEL, bf::NORMAL));
        d.select(pid);
        let (cmd, d) = app.exec_dialog(d);
        if cmd != cm::OK {
            return;
        }
        let pattern = d.win.group.find::<InputLine>(pid).map(|i| i.text()).unwrap_or_default();
        let text = d.win.group.find::<InputLine>(tid).map(|i| i.text()).unwrap_or_default();
        let base = self.panel(app).path().to_path_buf();
        let found = run_search(app, &base, pattern.trim(), &text);
        if found.is_empty() {
            message_box(app, "\x03Ничего не найдено.", mf::INFORMATION | mf::OK_BUTTON);
            return;
        }
        let title = if text.is_empty() { pattern.trim().to_string() } else { format!("{} «{}»", pattern.trim(), text) };
        let n = found.len();
        self.panel(app).set_listing(&title, &base, found);
        self.note(app, &format!("Найдено: {}{} — Enter на каталоге или ← выходит из списка", n, if n >= FIND_LIMIT { "+" } else { "" }));
    }

    // --- команды ---

    /// PowerCLI: выполнить строку (с подстановками и циклом по выделенным).
    fn powercli(&mut self, app: &mut Program, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        history_add(HIST_CLI, line);
        let (cmd, mode) = if let Some(c) = line.strip_suffix('&') {
            (c.trim_end(), RunMode::Background)
        } else if let Some(c) = line.strip_suffix('%') {
            (c.trim_end(), RunMode::Interactive)
        } else {
            (line, RunMode::Normal)
        };
        let vars = self.vars(app);
        let marked = self.panel(app).marked().iter().map(|e| (*e).clone()).collect::<Vec<_>>();
        let dir = self.panel(app).path().to_path_buf();
        let looped = !marked.is_empty() && ["$f", "$v", "$F", "$E", "$e", "$i", "$tm", "$dm"].iter().any(|v| cmd.contains(v));
        let commands: Vec<String> = if looped {
            marked.iter().enumerate().map(|(i, e)| expand(cmd, &vars, Some(e), i + 1)).collect()
        } else {
            let cur = self.panel(app).current().cloned();
            vec![expand(cmd, &vars, cur.as_ref(), 1)]
        };
        for c in commands {
            match mode {
                RunMode::Background => {
                    if let Err(e) = shell(&c, &dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                        error_box(app, &format!("Не удалось запустить:\n{}", e));
                        break;
                    }
                }
                RunMode::Interactive => {
                    let r = app.suspended(|| shell(&c, &dir).status());
                    if let Err(e) = r {
                        error_box(app, &format!("Не удалось запустить:\n{}", e));
                        break;
                    }
                }
                RunMode::Normal => {
                    if !run_and_show(app, &c, &dir) {
                        break;
                    }
                }
            }
        }
        if looped {
            self.panel(app).clear_marks();
        }
        if let Some(i) = app.find_mut::<InputLine>(self.cli_input) {
            i.set_text("");
        }
        self.refresh_all(app);
    }

    /// `@`: команда, к которой добавляются выделенные файлы (или текущий).
    fn exec_on(&mut self, app: &mut Program) {
        let targets = self.panel(app).targets();
        if targets.is_empty() {
            return;
        }
        if let Some(cmd) = input_box(app, "Команда над файлами", &format!("~К~оманда для {}:", describe(&targets)), "", 1024)
        {
            let cmd = cmd.trim();
            if cmd.is_empty() {
                return;
            }
            let files: Vec<String> = targets.iter().map(|p| quote(&file_name(p))).collect();
            let line = format!("{} {}", cmd, files.join(" "));
            let dir = self.panel(app).path().to_path_buf();
            run_and_show(app, &line, &dir);
            self.refresh_all(app);
        }
    }

    /// Значения переменных PowerCLI, не зависящие от файла.
    fn vars(&self, app: &mut Program) -> Vars {
        let dp = self.dp(app);
        let p = dp.active();
        Vars {
            dir: p.path().display().to_string(),
            other: dp.other().path().display().to_string(),
            selected: p.marked().iter().map(|e| e.name.clone()).collect(),
            all: p.entries().iter().filter(|e| !e.is_parent()).map(|e| e.name.clone()).collect(),
            bookmarks: self.bookmarks.iter().map(|(k, v)| (*k, v.display().to_string())).collect(),
        }
    }

    fn toggle_cli(&mut self, app: &mut Program) {
        if self.cli_focused(app) {
            app.focus(self.panels);
        } else {
            app.focus(self.cli_input);
        }
    }

    fn quit(&mut self, app: &mut Program, chdir: bool) {
        if chdir {
            if let Some(f) = &self.path_file {
                let p = self.panel(app).path().display().to_string();
                let _ = std::fs::write(f, p);
            }
        }
        save_bookmarks(&self.bookmarks);
        app.quit();
    }

    /// Буквенные клавиши lfm (когда фокус на панелях).
    fn letter(&mut self, app: &mut Program, k: KeyEvent) -> bool {
        if let Some(mode) = self.pending.take() {
            if let Key::Char(c) = k.key {
                self.bookmark_key(app, mode, c);
            }
            return true;
        }
        let c = match (k.key, k.mods.ctrl, k.mods.alt) {
            (Key::Char(c), false, false) => c,
            _ => return false,
        };
        let cmd = match c {
            'q' => {
                self.quit(app, true);
                return true;
            }
            'b' => {
                self.pending = Some(Pending::GotoBookmark);
                self.note(app, "Перейти к закладке: 0–9, a–z");
                return true;
            }
            'B' => {
                self.pending = Some(Pending::SetBookmark);
                self.note(app, "Запомнить каталог в закладке: 0–9, a–z");
                return true;
            }
            'g' => CM_GOTO,
            's' => CM_SORT,
            '#' => CM_DIRSIZE,
            '.' => CM_SINGLE,
            ',' => CM_SWAP,
            '=' => CM_SYNC,
            ':' => CM_TAB_NEW,
            '!' => CM_TAB_CLOSE,
            '<' => CM_TAB_LEFT,
            '>' => CM_TAB_RIGHT,
            '+' => CM_MARK,
            '-' => CM_UNMARK,
            '*' => CM_INVERT,
            '/' => CM_FIND,
            'i' => CM_INFO,
            't' => CM_TOUCH,
            'l' => CM_LINK,
            '@' => CM_EXEC_ON,
            'h' => CM_HELP_KEYS,
            _ => return false,
        };
        self.command(app, cmd);
        true
    }

    fn command(&mut self, app: &mut Program, cmd: u16) -> bool {
        match cmd {
            CM_RENAME => self.rename(app),
            CM_VIEW => self.view(app),
            CM_EDIT => self.edit(app),
            CM_COPY => self.copy_move(app, JobKind::Copy),
            CM_MOVE => self.copy_move(app, JobKind::Move),
            CM_MKDIR => self.mkdir(app),
            CM_DELETE => self.delete(app),
            CM_HELP_KEYS => show_help(app),
            CM_ABOUT => {
                message_box(
                    app,
                    "\x03LFM на tvision-rs\n\n\x03По мотивам Last File Manager\n\x03Iñigo Serna, 2001–2017",
                    mf::INFORMATION | mf::OK_BUTTON,
                );
            }
            CM_QUIT_NOCD => self.quit(app, false),
            c if c == cm::QUIT => self.quit(app, true),
            CM_GOTO => self.goto(app),
            CM_TREE => self.tree(app),
            CM_BOOKMARKS => self.bookmarks_list(app),
            CM_HISTORY => self.history(app),
            CM_FIND => self.find(app),
            CM_POWERCLI => self.toggle_cli(app),
            CM_SHELL => {
                self.sync_cwd(app);
                app.dos_shell("Оболочка ОС. Введите exit, чтобы вернуться в LFM.");
                self.refresh_all(app);
            }
            CM_FILTER => self.filter(app),
            CM_HIDDEN => self.panel(app).toggle_hidden(),
            CM_SORT => self.sort(app),
            CM_DIRSIZE => self.dir_sizes(app),
            CM_SINGLE => self.dp(app).toggle_single(),
            CM_SWAP => self.dp(app).swap(),
            CM_SYNC => self.dp(app).sync_other(),
            CM_TAB_NEW => self.panel(app).new_tab(),
            CM_TAB_CLOSE => {
                if !self.panel(app).close_tab() {
                    self.note(app, "Последнюю вкладку закрыть нельзя");
                }
            }
            CM_TAB_LEFT => self.panel(app).prev_tab(),
            CM_TAB_RIGHT => self.panel(app).next_tab(),
            CM_MARK => self.mark(app, true),
            CM_UNMARK => self.mark(app, false),
            CM_INVERT => self.panel(app).invert_marks(),
            CM_INFO => self.info(app),
            CM_TOUCH => self.touch(app),
            CM_LINK => self.link(app),
            CM_EXEC_ON => self.exec_on(app),
            CM_REFRESH => self.refresh_all(app),
            CM_ROOTS_LEFT => self.roots(app, false),
            CM_ROOTS_RIGHT => self.roots(app, true),
            c if c == cm::FILE_PANEL_EXECUTE => self.execute(app),
            _ => return false,
        }
        true
    }

    /// Автообновление: каталог панели изменился на диске — перечитать.
    fn auto_refresh(&mut self, app: &mut Program) {
        if self.last_check.elapsed().as_millis() < 1500 {
            return;
        }
        self.last_check = Instant::now();
        let ids = {
            let dp = self.dp(app);
            [dp.left_id(), dp.right_id()]
        };
        for id in ids {
            let dp = self.dp(app);
            let p = match dp.panel_mut(id) {
                Some(p) if p.listing_title().is_none() => p,
                _ => continue,
            };
            let m = std::fs::metadata(p.path()).and_then(|m| m.modified()).ok();
            match self.seen.get(&id) {
                Some(old) if *old != m => {
                    p.refresh();
                    self.seen.insert(id, m);
                }
                None => {
                    self.seen.insert(id, m);
                }
                _ => {}
            }
        }
    }
}

impl AppHandler for Lfm {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        match ev {
            Event::Command(m) => {
                let c = m.command;
                if self.command(app, c) {
                    ev.clear();
                }
            }
            Event::KeyDown(k) => {
                let k = *k;
                if self.cli_focused(app) {
                    match k.key {
                        Key::Enter if k.mods == Mods::NONE => {
                            let line = app.find::<InputLine>(self.cli_input).map(|i| i.text()).unwrap_or_default();
                            self.powercli(app, &line);
                            ev.clear();
                        }
                        Key::Esc => {
                            app.focus(self.panels);
                            ev.clear();
                        }
                        _ => {}
                    }
                } else if self.letter(app, k) {
                    ev.clear();
                }
            }
            _ => {}
        }
        self.sync_cwd(app);
    }

    fn idle(&mut self, app: &mut Program) {
        self.auto_refresh(app);
        self.sync_cwd(app);
    }
}

// ----------------------------------------------------------------------
// Выполнение команд
// ----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Normal,
    Background,
    Interactive,
}

/// Значения переменных PowerCLI.
struct Vars {
    dir: String,
    other: String,
    selected: Vec<String>,
    all: Vec<String>,
    bookmarks: HashMap<char, String>,
}

/// Заключить строку в кавычки для оболочки (sh или cmd).
fn quote(s: &str) -> String {
    if cfg!(windows) {
        // В именах Windows кавычек не бывает; `%` и `^` внутри кавычек cmd безопасны.
        format!("\"{}\"", s.replace('"', ""))
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Подставить переменные PowerCLI. Длинные имена (`$tm`, `$b1`) — раньше
/// коротких; значения — в кавычках.
fn expand(cmd: &str, v: &Vars, file: Option<&PanelEntry>, index: usize) -> String {
    let (f, full, stem, ext, tm) = match file {
        Some(e) => {
            let name = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| e.name.clone());
            let (stem, ext) = split_ext(&name);
            (name.clone(), e.path.display().to_string(), stem, ext, e.modified)
        }
        None => Default::default(),
    };
    let now = Some(SystemTime::now());
    let date = |t: Option<SystemTime>| format_time_full(t).split(' ').next().unwrap_or("").to_string();
    let mut out = String::new();
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' || i + 1 >= chars.len() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let rest: String = chars[i + 1..].iter().take(2).collect();
        let two = |s: &str| rest.starts_with(s);
        let (val, len) = if two("tm") {
            (quote(&format_time_full(tm)), 2)
        } else if two("tn") {
            (quote(&format_time_full(now)), 2)
        } else if two("dm") {
            (quote(&date(tm)), 2)
        } else if two("dn") {
            (quote(&date(now)), 2)
        } else if rest.starts_with('b') && rest.chars().nth(1).map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
            let k = rest.chars().nth(1).unwrap().to_ascii_lowercase();
            (v.bookmarks.get(&k).map(|p| quote(p)).unwrap_or_default(), 2)
        } else {
            match chars[i + 1] {
                'f' | 'v' => (quote(&f), 1),
                'F' => (quote(&full), 1),
                'E' => (quote(&stem), 1),
                'e' => (quote(&ext), 1),
                'p' | 'd' => (quote(&v.dir), 1),
                'o' => (quote(&v.other), 1),
                's' => (v.selected.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" "), 1),
                'a' => (v.all.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" "), 1),
                'i' => (index.to_string(), 1),
                '$' => ("$".to_string(), 1),
                _ => {
                    out.push('$');
                    i += 1;
                    continue;
                }
            }
        };
        out.push_str(&val);
        i += 1 + len;
    }
    out
}

/// Имя без расширения и расширение (`.tar.gz` — целиком, как в lfm).
fn split_ext(name: &str) -> (String, String) {
    for double in [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.lz", ".tar.zst"] {
        if name.len() > double.len() && name.to_lowercase().ends_with(double) {
            let cut = name.len() - double.len();
            return (name[..cut].to_string(), name[cut + 1..].to_string());
        }
    }
    match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), e.to_string()),
        _ => (name.to_string(), String::new()),
    }
}

/// Команда оболочки (sh или cmd) в каталоге `dir`.
fn shell(cmd: &str, dir: &Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        // chcp 65001 — вывод в UTF-8, иначе русские буквы будут в cp866.
        c.raw_arg(format!("/S /C \"chcp 65001 >nul & {}\"", cmd));
        c.current_dir(dir);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd).current_dir(dir);
        c
    }
}

/// Открыть файл программой системы (в фоне).
fn open_with_system(path: &Path) -> std::io::Result<()> {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ())
}

/// Выполнить команду с окном «Выполняется…» (можно остановить) и показать
/// вывод. `false` — команду не удалось запустить или её остановили.
fn run_and_show(app: &mut Program, cmd: &str, dir: &Path) -> bool {
    let mut child: Child = match shell(cmd, dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            error_box(app, &format!("Не удалось запустить:\n{}", e));
            return false;
        }
    };
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut readers = Vec::new();
    for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>), child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>)]
        .into_iter()
        .flatten()
    {
        let out = out.clone();
        readers.push(std::thread::spawn(move || {
            let mut pipe = pipe;
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Ok(mut o) = out.lock() {
                    o.extend_from_slice(&buf[..n]);
                }
            }
        }));
    }
    let mut d = Dialog::centered(60, 8, "PowerCLI");
    d.add(StaticText::new(Rect::new(2, 2, 58, 3), &format!("Выполняется: {}", cmd)));
    d.add(ProgressBar::new(Rect::new(2, 3, 58, 4), 1).indeterminate());
    d.add(Button::new(Rect::new(24, 5, 36, 7), "~С~топ", cm::CANCEL, bf::DEFAULT));
    let mut status = None;
    let (c, _) = app.exec_dialog_idle(d, |_, _| match child.try_wait() {
        Ok(Some(s)) => {
            status = Some(s);
            Some(cm::OK)
        }
        Ok(None) => None,
        Err(_) => Some(cm::OK),
    });
    if c != cm::OK {
        let _ = child.kill();
        let _ = child.wait();
    }
    for r in readers {
        let _ = r.join();
    }
    let text = String::from_utf8_lossy(&out.lock().map(|o| o.clone()).unwrap_or_default()).into_owned();
    let mut title = format!("Вывод: {}", cmd);
    if c != cm::OK {
        title.push_str(" (остановлено)");
    } else if let Some(s) = status {
        if !s.success() {
            title.push_str(&format!(" (код {})", s.code().unwrap_or(-1)));
        }
    }
    if !text.trim().is_empty() {
        show_text(app, &title, &text);
    } else if c == cm::OK && status.map(|s| !s.success()).unwrap_or(false) {
        error_box(app, &format!("Команда завершилась с ошибкой (код {}).", status.and_then(|s| s.code()).unwrap_or(-1)));
    }
    c == cm::OK
}

// ----------------------------------------------------------------------
// Операции с прогрессом
// ----------------------------------------------------------------------

/// Выполнить операцию над файлами с окном прогресса.
fn run_job(app: &mut Program, title: &str, kind: JobKind, sources: Vec<PathBuf>, dest: Option<PathBuf>, conflict: Conflict) -> fileops::JobProgress {
    let job = FileJob::start(kind, sources, dest, conflict);
    let mut d = Dialog::centered(62, 10, title);
    let name = d.add(StaticText::new(Rect::new(2, 2, 60, 3), "Подсчёт…"));
    let bar = d.add(ProgressBar::new(Rect::new(2, 3, 60, 4), 100).with_caption("{percent}%"));
    let counts = d.add(StaticText::new(Rect::new(2, 5, 60, 6), ""));
    d.add(Button::new(Rect::new(25, 7, 37, 9), "~С~топ", cm::CANCEL, bf::DEFAULT));
    let (c, _) = app.exec_dialog_idle(d, |app, _| {
        let p = job.progress();
        if let Some(t) = app.find_mut::<StaticText>(name) {
            t.text = if p.counting { "Подсчёт…".into() } else { p.current.clone() };
        }
        if let Some(b) = app.find_mut::<ProgressBar>(bar) {
            b.set_value(p.percent() as i64);
        }
        if let Some(t) = app.find_mut::<StaticText>(counts) {
            t.text = if kind == JobKind::Delete {
                format!("Файлов: {} из {}", p.done_files, p.total_files)
            } else {
                format!(
                    "Файлов: {} из {}   {} из {} байт",
                    p.done_files,
                    p.total_files,
                    format_size_full(p.done_bytes),
                    format_size_full(p.total_bytes)
                )
            };
        }
        if p.finished {
            Some(cm::OK)
        } else {
            None
        }
    });
    let mut job = job;
    if c != cm::OK {
        job.cancel();
    }
    job.wait()
}

/// Итог операции: ошибки — в окне.
fn report_job(app: &mut Program, title: &str, p: &fileops::JobProgress) {
    if p.errors.is_empty() {
        return;
    }
    let mut text = format!("{}: ошибок {}\n\n", title, p.errors.len());
    for e in p.errors.iter().take(200) {
        text.push_str(e);
        text.push('\n');
    }
    show_text(app, &format!("{} — ошибки", title), &text);
}

// ----------------------------------------------------------------------
// Окна и диалоги
// ----------------------------------------------------------------------

fn error_box(app: &mut Program, msg: &str) {
    message_box_rect(app, Rect::new(0, 0, 60, 10), msg, mf::ERROR | mf::OK_BUTTON, true);
}

/// Окно с прокручиваемым текстом.
fn show_text(app: &mut Program, title: &str, text: &str) {
    let r = app.desktop().group.vs.extent();
    let mut w = Window::new(Rect::new(2, 1, r.b.x - 2, r.b.y - 2), title, 0);
    w.group.vs.options |= of::TILEABLE;
    let h = w.standard_scroll_bar(false);
    let v = w.standard_scroll_bar(true);
    let mut t = Terminal::new(w.interior(), Some(h), Some(v), 100_000);
    for line in text.lines() {
        t.write_line(&line.replace('\t', "    "));
    }
    let size = t.vs.size;
    t.scroller.scroll_to(0, 0, size);
    w.add(t);
    app.insert_window(w);
}

/// F3: просмотр файла (текст или шестнадцатеричный вид).
fn view_file(app: &mut Program, path: &Path) {
    const LIMIT: usize = 4 << 20;
    let data = match std::fs::File::open(path) {
        Ok(f) => {
            let mut v = Vec::new();
            let _ = f.take(LIMIT as u64).read_to_end(&mut v);
            v
        }
        Err(e) => {
            error_box(app, &format!("Не удалось открыть:\n{}", e));
            return;
        }
    };
    let binary = data.iter().take(8192).any(|&b| b == 0);
    let mut text = String::new();
    if binary {
        for (i, chunk) in data.chunks(16).enumerate() {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
            let asc: String = chunk.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' }).collect();
            text.push_str(&format!("{:08X}  {:<48} {}\n", i * 16, hex.join(" "), asc));
        }
    } else {
        text = String::from_utf8_lossy(&data).into_owned();
    }
    if data.len() >= LIMIT {
        text.push_str("\n… (показаны первые 4 МБ)\n");
    }
    show_text(app, &path.display().to_string(), &text);
}

/// Выбор строки из списка. `None` — отмена.
fn choose(app: &mut Program, title: &str, items: &[String]) -> Option<usize> {
    let w = items.iter().map(|s| tvision::text::str_width(s)).max().unwrap_or(10).clamp(30, 70) as i32 + 6;
    let h = (items.len() as i32).clamp(5, 15) + 6;
    let mut d = Dialog::centered(w, h, title);
    let sb = d.add(ScrollBar::new(Rect::new(w - 4, 2, w - 3, h - 4)));
    let mut lb = ListBox::new(Rect::new(2, 2, w - 4, h - 4), 1, Some(sb));
    lb.set_items(items.to_vec());
    let lid = d.add(lb);
    d.add(Button::new(Rect::new(w / 2 - 13, h - 3, w / 2 - 1, h - 1), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(w / 2 + 1, h - 3, w / 2 + 13, h - 1), "Отмена", cm::CANCEL, bf::NORMAL));
    d.select(lid);
    let (c, d) = app.exec_dialog(d);
    if c != cm::OK || items.is_empty() {
        return None;
    }
    d.win.group.find::<ListBox>(lid).map(|l| l.focused)
}

/// Строка ввода с историей.
fn input_box_hist(app: &mut Program, title: &str, label: &str, initial: &str, hist: u16) -> Option<String> {
    let mut d = Dialog::centered(62, 8, title);
    let mut il = InputLine::new(Rect::new(3, 3, 56, 4), 1024);
    il.set_text(initial);
    il.history_id = Some(hist);
    let iid = d.add(il);
    d.add(History::new(Rect::new(56, 3, 59, 4), iid, hist));
    d.add(Label::new(Rect::new(2, 2, 40, 3), label, Some(iid)));
    d.add(Button::new(Rect::new(17, 5, 29, 7), "O~K~", cm::OK, bf::DEFAULT));
    d.add(Button::new(Rect::new(31, 5, 43, 7), "Отмена", cm::CANCEL, bf::NORMAL));
    d.select(iid);
    let (c, d) = app.exec_dialog(d);
    if c != cm::OK {
        return None;
    }
    let t = d.win.group.find::<InputLine>(iid).map(|i| i.text()).unwrap_or_default();
    history_add(hist, &t);
    Some(t)
}

/// Поиск файлов в отдельном потоке с окном «Поиск…».
fn run_search(app: &mut Program, base: &Path, pattern: &str, text: &str) -> Vec<PathBuf> {
    let found = Arc::new(Mutex::new((Vec::<PathBuf>::new(), String::new(), false)));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (found, stop) = (found.clone(), stop.clone());
        let base = base.to_path_buf();
        let patterns: Vec<String> = pattern.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let needle = text.to_string();
        std::thread::spawn(move || {
            let cs = !cfg!(windows);
            let mut stack = vec![base];
            while let Some(dir) = stack.pop() {
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if let Ok(mut f) = found.lock() {
                    f.1 = dir.display().to_string();
                }
                let rd = match std::fs::read_dir(&dir) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let mut subdirs = Vec::new();
                for e in rd.flatten() {
                    let path = e.path();
                    let name = e.file_name().to_string_lossy().into_owned();
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    if is_dir {
                        subdirs.push(path.clone());
                    }
                    let name_ok = patterns.is_empty() || patterns.iter().any(|p| glob_match(p, &name, cs));
                    let text_ok = needle.is_empty() || (!is_dir && file_contains(&path, &needle));
                    if name_ok && text_ok {
                        if let Ok(mut f) = found.lock() {
                            f.0.push(path);
                            if f.0.len() >= FIND_LIMIT {
                                f.2 = true;
                                return;
                            }
                        }
                    }
                }
                subdirs.sort();
                stack.extend(subdirs.into_iter().rev());
            }
            if let Ok(mut f) = found.lock() {
                f.2 = true;
            }
        });
    }
    let mut d = Dialog::centered(62, 9, "Поиск");
    let where_ = d.add(StaticText::new(Rect::new(2, 2, 60, 3), ""));
    let count = d.add(StaticText::new(Rect::new(2, 3, 60, 4), ""));
    d.add(ProgressBar::new(Rect::new(2, 4, 60, 5), 1).indeterminate());
    d.add(Button::new(Rect::new(25, 6, 37, 8), "~С~топ", cm::CANCEL, bf::DEFAULT));
    app.exec_dialog_idle(d, |app, _| {
        let (n, dir, done) = found.lock().map(|f| (f.0.len(), f.1.clone(), f.2)).unwrap_or((0, String::new(), true));
        if let Some(t) = app.find_mut::<StaticText>(where_) {
            t.text = dir;
        }
        if let Some(t) = app.find_mut::<StaticText>(count) {
            t.text = format!("Найдено: {}", n);
        }
        if done {
            Some(cm::OK)
        } else {
            None
        }
    });
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut v = found.lock().map(|f| f.0.clone()).unwrap_or_default();
    v.sort();
    v
}

/// Есть ли в файле текст (без учёта регистра; файлы больше 32 МБ пропускаются).
fn file_contains(path: &Path, needle: &str) -> bool {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() <= 32 << 20 => {}
        _ => return false,
    }
    match std::fs::read(path) {
        Ok(data) => String::from_utf8_lossy(&data).to_lowercase().contains(&needle.to_lowercase()),
        Err(_) => false,
    }
}

/// F1: справка по клавишам.
fn show_help(app: &mut Program) {
    let text = "\
LFM на tvision-rs — клавиши (как в Last File Manager)

Движение
  ↑ ↓ PgUp PgDn Home End      по списку; Ctrl+↑/↓ — на 10
  ← или Backspace             в родительский каталог
  → или Enter                 в каталог; Enter на файле — открыть/выполнить
  Ctrl+S / Alt+буква          поиск по имени / к файлу на букву
  g                           перейти по пути (с историей)
  Ctrl+T                      дерево каталогов
  Alt+F1 / Alt+F2             диски и точки монтирования для левой/правой
  b + 0-9,a-z / B + …         к закладке / запомнить закладку
  Ctrl+D / Ctrl+Y             список закладок / история каталогов

Панели и вкладки
  Tab                         другая панель
  .                           одна или две панели
  = , Ctrl+U или ,            тот же каталог в другой / поменять местами
  : ! < >                     вкладка: новая, закрыть, влево, вправо
  Ctrl+H                      скрытые файлы
  Ctrl+F                      фильтр вкладки: *.o,*.tmp,!keep*
  s / #                       сортировка / размеры каталогов
  Ctrl+R                      перечитать

Выделение
  Ins или пробел              выделить файл
  + / - / *                   по шаблону / снять / инвертировать

Файлы
  F2 имя   F3 просмотр   F4 правка   F5 копия   F6 перенос
  F7 каталог   F8 удалить   i сведения   t создать файл   l ссылка
  @ команда над выделенными файлами
  / поиск файлов (и текста) → виртуальная панель (← — выход из неё)

PowerCLI (Ctrl+X, Enter — выполнить, Esc — к панелям)
  $f файл  $F полный путь  $E имя без расширения  $e расширение
  $p каталог  $o другая панель  $s выделенные  $a все  $i номер
  $b0…$bz закладки  $tm/$dm время/дата изменения  $tn/$dn сейчас
  С выделением и $f/$F/$E/$e/$i команда выполняется для каждого файла.
  Значения подставляются в кавычках. В конце: & — в фоне,
  % — интерактивная программа; иначе вывод показывается в окне.
  Пример: cp $f $o           (скопировать каждый выделенный)
          mv $f $E.bak       (переименовать в .bak)

Прочее
  Ctrl+O оболочка ОС   F9 меню   F10 или q выход   Ctrl+Q выход без cd
";
    show_text(app, "Справка — клавиши", text);
}

// ----------------------------------------------------------------------
// Мелочи
// ----------------------------------------------------------------------

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// «файл «x»» или «N файлов».
fn describe(paths: &[PathBuf]) -> String {
    match paths.len() {
        1 => format!("«{}»", file_name(&paths[0])),
        n => format!("{} {}", n, plural(n, "файл", "файла", "файлов")),
    }
}

fn plural<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let (m10, m100) = (n % 10, n % 100);
    if m10 == 1 && m100 != 11 {
        one
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        few
    } else {
        many
    }
}

fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix('~'), std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })) {
        (Some(rest), Some(h)) => PathBuf::from(h).join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(p),
    }
}

/// Каталог настроек: `%APPDATA%\tvision-lfm` или `~/.config/tvision-lfm`.
fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("tvision-lfm"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("tvision-lfm"))
    }
}

fn load_bookmarks() -> HashMap<char, PathBuf> {
    let mut m = HashMap::new();
    if let Some(text) = config_dir().and_then(|d| std::fs::read_to_string(d.join("bookmarks")).ok()) {
        for line in text.lines() {
            let mut it = line.splitn(2, ' ');
            if let (Some(k), Some(p)) = (it.next(), it.next()) {
                if let Some(c) = k.chars().next() {
                    m.insert(c, PathBuf::from(p));
                }
            }
        }
    }
    m
}

fn save_bookmarks(b: &HashMap<char, PathBuf>) {
    if let Some(d) = config_dir() {
        let _ = std::fs::create_dir_all(&d);
        let mut keys: Vec<&char> = b.keys().collect();
        keys.sort();
        let text: String = keys.iter().map(|k| format!("{} {}\n", k, b[k].display())).collect();
        let _ = std::fs::write(d.join("bookmarks"), text);
    }
}

/// Пункт меню с подписью клавиши, но без привязки (буквы lfm обрабатывает
/// программа: иначе их перехватывало бы меню при наборе в PowerCLI).
fn item_label(m: Menu, name: &str, cmd: u16, label: &str) -> Menu {
    let mut m = m;
    m.items.push(MenuItem::Command { name: name.into(), command: cmd, key: None, param: label.into(), help_ctx: 0 });
    m
}

fn menu() -> Menu {
    let file = Menu::new()
        .item_key("~П~росмотр", CM_VIEW, kb::f(3), "F3")
        .item_key("П~р~авка", CM_EDIT, kb::f(4), "F4")
        .item_key("~К~опировать", CM_COPY, kb::f(5), "F5")
        .item_key("П~е~ренести", CM_MOVE, kb::f(6), "F6")
        .item_key("~И~мя", CM_RENAME, kb::F2, "F2")
        .item_key("Создать ~к~аталог", CM_MKDIR, kb::f(7), "F7")
        .item_key("~У~далить", CM_DELETE, kb::f(8), "F8");
    let file = item_label(file, "Создать ~ф~айл", CM_TOUCH, "t");
    let file = item_label(file, "~С~сылка", CM_LINK, "l");
    let file = item_label(file, "С~в~едения", CM_INFO, "i");
    let file = item_label(file, "~Д~ействие над файлами…", CM_EXEC_ON, "@").separator().item_key("В~ы~ход", cm::QUIT, kb::F10, "F10").item_key(
        "Выход ~б~ез смены каталога",
        CM_QUIT_NOCD,
        kb::ctrl('q'),
        "Ctrl+Q",
    );

    let panel = Menu::new();
    let panel = item_label(panel, "~Н~овая вкладка", CM_TAB_NEW, ":");
    let panel = item_label(panel, "~З~акрыть вкладку", CM_TAB_CLOSE, "!");
    let panel = item_label(panel, "Вкладка ~в~лево", CM_TAB_LEFT, "<");
    let panel = item_label(panel, "Вкладка в~п~раво", CM_TAB_RIGHT, ">").separator();
    let panel = item_label(panel, "~С~ортировка…", CM_SORT, "s").item_key("~Ф~ильтр…", CM_FILTER, kb::ctrl('f'), "Ctrl+F").item_key(
        "~С~крытые файлы",
        CM_HIDDEN,
        kb::ctrl('h'),
        "Ctrl+H",
    );
    let panel = item_label(panel, "~Р~азмеры каталогов", CM_DIRSIZE, "#").item_key("П~е~речитать", CM_REFRESH, kb::ctrl('r'), "Ctrl+R").separator();
    let panel = item_label(panel, "~О~дна/две панели", CM_SINGLE, ".");
    let panel = item_label(panel, "Тот ~ж~е каталог", CM_SYNC, "=").item_key("Поменять ~м~естами", CM_SWAP, kb::ctrl('u'), "Ctrl+U");

    let cmds = Menu::new();
    let cmds = item_label(cmds, "~П~оиск файлов…", CM_FIND, "/");
    let cmds = item_label(cmds, "П~е~рейти…", CM_GOTO, "g").item_key("~Д~ерево каталогов", CM_TREE, kb::ctrl('t'), "Ctrl+T").item_key(
        "Д~и~ски слева",
        CM_ROOTS_LEFT,
        kb::alt_f(1),
        "Alt+F1",
    );
    let cmds = cmds
        .item_key("Ди~с~ки справа", CM_ROOTS_RIGHT, kb::alt_f(2), "Alt+F2")
        .item_key("~З~акладки", CM_BOOKMARKS, kb::ctrl('d'), "Ctrl+D")
        .item_key("~И~стория", CM_HISTORY, kb::ctrl('y'), "Ctrl+Y");
    let cmds = item_label(cmds.separator(), "~В~ыделить…", CM_MARK, "+");
    let cmds = item_label(cmds, "~С~нять выделение…", CM_UNMARK, "-");
    let cmds = item_label(cmds, "~О~братить выделение", CM_INVERT, "*")
        .separator()
        .item_key("~P~owerCLI", CM_POWERCLI, kb::ctrl('x'), "Ctrl+X")
        .item_key("~О~болочка ОС", CM_SHELL, kb::ctrl('o'), "Ctrl+O");

    let help = Menu::new().item_key("~К~лавиши", CM_HELP_KEYS, kb::F1, "F1").item("~О~ программе", CM_ABOUT);

    Menu::new().submenu("~Ф~айл", file).submenu("~П~анель", panel).submenu("~К~оманды", cmds).submenu("~С~правка", help)
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut args = std::env::args().skip(1);
    let mut paths = Vec::new();
    let mut path_file = None;
    while let Some(a) = args.next() {
        if a == "--path-file" {
            path_file = args.next().map(PathBuf::from);
        } else {
            paths.push(PathBuf::from(a));
        }
    }
    let cur = std::env::current_dir()?;
    let left = paths.first().cloned().unwrap_or_else(|| cur.clone());
    let right = paths.get(1).cloned().unwrap_or_else(|| left.clone());
    let mut app = Program::with_terminal(Some(menu()), Some(status()))?;
    let mut lfm = Lfm::build(&mut app, &left, &right);
    lfm.path_file = path_file;
    app.run(&mut lfm);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::{HeadlessBackend, RawEvent};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lfm-example-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("left")).unwrap();
        std::fs::create_dir_all(d.join("right")).unwrap();
        d
    }

    fn key(k: Key, m: Mods) -> RawEvent {
        RawEvent::Key(KeyEvent::new(k, m))
    }

    fn text(s: &str) -> Vec<RawEvent> {
        s.chars().map(|c| key(Key::Char(c), Mods::NONE)).collect()
    }

    fn run(d: &Path, input: Vec<RawEvent>) -> (Program, Lfm) {
        let mut b = HeadlessBackend::new(100, 30);
        b.input.extend(input);
        let mut app = Program::new(Box::new(b), Some(menu()), Some(status()));
        let mut lfm = Lfm::build(&mut app, &d.join("left"), &d.join("right"));
        app.pump(&mut lfm);
        app.redraw();
        (app, lfm)
    }

    #[test]
    fn mkdir_copy_delete() {
        let d = tmp("ops");
        std::fs::write(d.join("left/a.txt"), "привет").unwrap();
        std::fs::write(d.join("left/b.txt"), "мир").unwrap();
        let mut input = vec![key(Key::F(7), Mods::NONE)];
        input.extend(text("новый"));
        input.push(key(Key::Enter, Mods::NONE));
        // Выделить *.txt и скопировать в правую панель.
        input.push(key(Key::Char('+'), Mods::NONE));
        input.push(key(Key::Backspace, Mods::NONE));
        input.extend(text("*.txt"));
        input.push(key(Key::Enter, Mods::NONE));
        input.push(key(Key::F(5), Mods::NONE));
        input.push(key(Key::Enter, Mods::NONE));
        let (app, _) = run(&d, input);
        assert!(d.join("left/новый").is_dir(), "{}", app.screen_text());
        assert_eq!(std::fs::read_to_string(d.join("right/a.txt")).unwrap(), "привет");
        assert!(d.join("right/b.txt").exists());

        // Удалить в правой панели b.txt (Tab, к файлу, F8, Да).
        let mut input = vec![key(Key::Tab, Mods::NONE)];
        input.push(key(Key::Char('b'), Mods::ALT));
        input.push(key(Key::F(8), Mods::NONE));
        input.push(key(Key::Enter, Mods::NONE));
        let (_, _) = run(&d, input);
        assert!(!d.join("right/b.txt").exists());
        assert!(d.join("right/a.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn letters_tabs_and_filter() {
        let d = tmp("letters");
        std::fs::write(d.join("left/x.rs"), "").unwrap();
        std::fs::write(d.join("left/y.o"), "").unwrap();
        let mut input = vec![key(Key::Char(':'), Mods::NONE), key(Key::Char('f'), Mods::CTRL)];
        input.extend(text("*.o"));
        input.push(key(Key::Enter, Mods::NONE));
        input.push(key(Key::Char('.'), Mods::NONE)); // одна панель
        let (mut app, lfm) = run(&d, input);
        let s = app.screen_text();
        assert!(s.contains("1:left") && s.contains("2:left"), "{}", s);
        assert!(s.contains("x.rs") && !s.contains("y.o"), "{}", s);
        let dp = lfm.dp(&mut app);
        assert!(dp.single);
        assert_eq!(dp.active().tab_count(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn powercli_expands_and_quotes() {
        let v =
            Vars { dir: "/d".into(), other: "/o".into(), selected: vec!["a b".into()], all: vec![], bookmarks: HashMap::from([('1', "/bm".into())]) };
        let e = PanelEntry::from_path(Path::new("/nonexistent/it's.tar.gz"), "it's.tar.gz");
        let s = expand("cp $f $o/$E.$e $b1 $s $i $$", &v, Some(&e), 3);
        if cfg!(windows) {
            assert_eq!(s, "cp \"it's.tar.gz\" \"/o\"/\"it's\".\"tar.gz\" \"/bm\" \"a b\" 3 $");
        } else {
            assert_eq!(s, "cp 'it'\\''s.tar.gz' '/o'/'it'\\''s'.'tar.gz' '/bm' 'a b' 3 $");
        }
        assert_eq!(split_ext("x.TAR.GZ"), ("x".into(), "TAR.GZ".into()));
        assert_eq!(plural(21, "файл", "файла", "файлов"), "файл");
        assert_eq!(plural(12, "файл", "файла", "файлов"), "файлов");
    }

    #[cfg(unix)]
    #[test]
    fn powercli_runs_in_loop() {
        let d = tmp("cli");
        std::fs::write(d.join("left/one.txt"), "1").unwrap();
        std::fs::write(d.join("left/two.txt"), "2").unwrap();
        let mut input = vec![key(Key::Char('+'), Mods::NONE), key(Key::Backspace, Mods::NONE)];
        input.extend(text("*.txt"));
        input.push(key(Key::Enter, Mods::NONE));
        input.push(key(Key::Char('x'), Mods::CTRL));
        input.extend(text("cp $f $o/$E.bak"));
        input.push(key(Key::Enter, Mods::NONE));
        let (_, _) = run(&d, input);
        assert!(d.join("right/one.bak").exists() && d.join("right/two.bak").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
