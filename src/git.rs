//! Работа с системным Git. Машинный вывод остаётся байтами до разбора;
//! текст для экрана обрабатывается отдельно, чтобы сохранить пути файлов.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use std::{fs, io, thread};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub index: char,
    pub worktree: char,
    pub unmerged: bool,
}

#[derive(Debug)]
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: String,
    pub upstream: String,
    pub changes: Vec<Change>,
    pub initial: bool,
}

pub struct Git<'a> {
    pub executable: &'a str,
    pub cwd: &'a Path,
}

impl Git<'_> {
    /// Аргументы передаются напрямую процессу. Имена файлов с пробелами
    /// и символами синтаксиса оболочки остаются одиночными аргументами.
    fn command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(self.executable);
        command
            .arg("--literal-pathspecs")
            .args(args)
            .current_dir(self.cwd)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null());
        // Путь репозитория определяется выбранным каталогом IDE.
        for name in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR", "GIT_PREFIX"] {
            command.env_remove(name);
        }
        command
    }

    fn run<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.command(args).output().map_err(|e| format!("Не удалось запустить Git: {e}"))?;
        git_output(output.status, output.stdout, output.stderr)
    }

    /// Команды клонирования и чтения веток могут ждать сеть или помощник
    /// входа. Учитываем отсутствие вывода, чтобы длительная передача могла
    /// продолжаться, пока Git присылает прогресс.
    fn run_network<I, S>(&self, args: I, idle_seconds: u64) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let command = self.command(args);
        run_with_idle_timeout(command, Duration::from_secs(idle_seconds))
    }

    pub fn version(&self) -> Result<String> {
        self.run(["--version"]).map(|bytes| display(&bytes).trim().into())
    }

    fn root(&self) -> Result<PathBuf> {
        let mut bytes = self.run(["rev-parse", "--show-toplevel"])?;
        // Git завершает вывод переводом строки. Пробелы в самом пути значимы.
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            #[cfg(windows)]
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        native_path(&bytes)
    }

    /// Находим корень отдельно: проект TV Studio может лежать в подпапке.
    /// Статус и все относительные имена относятся к дереву Git целиком.
    pub fn snapshot(&self) -> Result<Snapshot> {
        let root = self.root()?;
        let raw = Git { cwd: &root, ..*self }.run(["status", "--porcelain=v2", "-z", "--branch", "--no-renames", "--untracked-files=all"])?;
        parse_status(root, &raw)
    }

    pub fn init(&self, branch: &str) -> Result<Snapshot> {
        match self.root() {
            Ok(root) => return Err(format!("Каталог уже входит в репозиторий {}", root.display())),
            Err(e) if !e.starts_with("fatal: not a git repository") => return Err(e),
            Err(_) => {}
        }
        self.run(["init", "--initial-branch", branch, "--", "."])?;
        self.snapshot()
    }

    /// Параметр ветки выбирает начальную ветку, сохраняя обычный набор
    /// удалённых веток для последующего переключения через окно Git.
    pub fn clone_into(&self, url: &str, destination: &Path, branch: Option<&str>) -> Result<Snapshot> {
        let mut args = vec![OsStr::new("clone"), OsStr::new("--progress")];
        if let Some(branch) = branch {
            args.extend([OsStr::new("--branch"), OsStr::new(branch)]);
        }
        args.extend([OsStr::new("--"), OsStr::new(url), destination.as_os_str()]);
        self.run_network(args, 60)?;
        Git { cwd: destination, ..*self }.snapshot()
    }

    pub fn diff(&self, change: &Change, staged: bool) -> Result<String> {
        // Обычный diff Git пропускает новые файлы. Предпросмотр читает
        // ограниченный объём и не следует за символьной ссылкой.
        if change.index == '?' && !staged {
            use io::Read;
            let path = self.cwd.join(&change.path);
            if !fs::symlink_metadata(&path).map_err(|e| e.to_string())?.is_file() {
                return Ok(format!("Новый файл: {} (особый тип)", change.path.display()));
            }
            let mut bytes = Vec::new();
            fs::File::open(&path).and_then(|file| file.take(256 * 1024 + 1).read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
            if bytes.contains(&0) {
                return Ok(format!("Новый двоичный файл: {}", change.path.display()));
            }
            let truncated = bytes.len() > 256 * 1024;
            bytes.truncate(256 * 1024);
            let mut text = format!("Новый файл: {}\n", change.path.display());
            text.push_str(&display(&bytes));
            if truncated {
                text.push_str("\n… Показаны первые 256 КиБ");
            }
            return Ok(text);
        }
        let mut args = vec![OsStr::new("diff"), OsStr::new("--no-ext-diff"), OsStr::new("--no-textconv"), OsStr::new("--no-color")];
        if staged {
            args.push(OsStr::new("--cached"));
        }
        args.extend([OsStr::new("--"), change.path.as_os_str()]);
        self.run(args).map(|bytes| display(&bytes))
    }

    pub fn stage(&self, path: &Path) -> Result<()> {
        self.run([OsStr::new("add"), OsStr::new("--"), path.as_os_str()]).map(drop)
    }

    pub fn unstage(&self, path: &Path, initial: bool) -> Result<()> {
        // До первого коммита у restore нет исходного дерева HEAD.
        // В этом случае удаляем запись только из индекса.
        if initial {
            self.run([OsStr::new("rm"), OsStr::new("--cached"), OsStr::new("--"), path.as_os_str()]).map(drop)
        } else {
            self.run([OsStr::new("restore"), OsStr::new("--staged"), OsStr::new("--"), path.as_os_str()]).map(drop)
        }
    }

    pub fn commit(&self, message: &str) -> Result<String> {
        self.run(["commit", "-m", message]).map(|bytes| display(&bytes))
    }

    pub fn identity(&self, local: bool, name: &str, email: &str) -> Result<String> {
        let scope = if local { "--local" } else { "--global" };
        self.run(["config", scope, "user.name", name])?;
        self.run(["config", scope, "user.email", email])?;
        Ok("Имя и email сохранены в настройках Git.".into())
    }

    pub fn add_origin(&self, url: &str) -> Result<String> {
        self.run(["remote", "add", "--", "origin", url])?;
        Ok("Добавлен remote origin.".into())
    }

    pub fn push(&self) -> Result<String> {
        self.run(["push", "--porcelain", "--set-upstream", "origin", "HEAD"]).map(|bytes| display(&bytes))
    }

    pub fn fetch(&self) -> Result<String> {
        self.run(["fetch", "origin"])?;
        Ok("Обновлены ссылки origin.".into())
    }
}

fn git_output(status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Result<Vec<u8>> {
    if status.success() {
        return Ok(stdout);
    }
    let detail = display(&stderr);
    if detail.trim().is_empty() {
        Err(format!("Git: {status}"))
    } else {
        Err(detail.trim().to_owned())
    }
}

/// Два потока нужно читать параллельно: прогресс клонирования приходит
/// в stderr, служебные данные Git — в stdout.
enum ProcessOutput {
    Bytes(usize, Vec<u8>),
    Closed(io::Result<()>),
}

fn read_process_output(mut pipe: impl io::Read, stream: usize, send: &mpsc::Sender<ProcessOutput>) {
    let mut buffer = [0; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                let _ = send.send(ProcessOutput::Closed(Ok(())));
                return;
            }
            Ok(n) => {
                if send.send(ProcessOutput::Bytes(stream, buffer[..n].to_vec())).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = send.send(ProcessOutput::Closed(Err(e)));
                return;
            }
        }
    }
}

fn run_with_idle_timeout(mut command: Command, idle: Duration) -> Result<Vec<u8>> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Отдельная группа даёт завершить и Git, и дочерний процесс
    // транспорта, который может удерживать открытый поток вывода.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| format!("Не удалось запустить Git: {e}"))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let (send, receive) = mpsc::channel();
    let out = send.clone();
    thread::spawn(move || read_process_output(stdout, 0, &out));
    thread::spawn(move || read_process_output(stderr, 1, &send));

    let mut output = [Vec::new(), Vec::new()];
    let mut closed = 0;
    // Сбрасываем срок на любом выводе, включая строки прогресса с \r.
    // Окончательный результат выдаём после завершения обоих читателей.
    let mut deadline = Instant::now() + idle;
    loop {
        let status = child.try_wait().map_err(|e| format!("Ожидание Git: {e}"))?;
        if let Some(status) = status {
            if closed == 2 {
                let [stdout, stderr] = output;
                return git_output(status, stdout, stderr);
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            if status.is_none() {
                terminate_git(&mut child);
            }
            return Err(format!("Git не прислал данных за {} сек. Проверьте адрес, сеть или вход в репозиторий.", idle.as_secs()));
        }
        let interval = remaining.min(Duration::from_millis(100));
        match receive.recv_timeout(interval) {
            Ok(ProcessOutput::Bytes(stream, bytes)) => {
                output[stream].extend(bytes);
                deadline = Instant::now() + idle;
            }
            Ok(ProcessOutput::Closed(Ok(()))) => closed += 1,
            Ok(ProcessOutput::Closed(Err(e))) => {
                if status.is_none() {
                    terminate_git(&mut child);
                }
                return Err(format!("Чтение вывода Git: {e}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Процесс может закрыть оба потока до завершения.
                closed = 2;
                thread::sleep(interval);
            }
        }
    }
}

fn terminate_git(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            let program = PathBuf::from(system_root).join("System32").join("taskkill.exe");
            let _ = Command::new(program)
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    #[cfg(unix)]
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// Porcelain v2 отделяет записи нулевым байтом. У обычных строк и
/// конфликтов разное число полей, а хвост содержит путь целиком.
fn parse_status(root: PathBuf, raw: &[u8]) -> Result<Snapshot> {
    let mut snapshot = Snapshot { root, branch: String::new(), upstream: String::new(), changes: Vec::new(), initial: false };
    for row in raw.split(|&b| b == 0).filter(|row| !row.is_empty()) {
        if let Some(branch) = row.strip_prefix(b"# branch.head ") {
            snapshot.branch = display(branch);
        } else if let Some(upstream) = row.strip_prefix(b"# branch.upstream ") {
            snapshot.upstream = display(upstream);
        } else if row == b"# branch.oid (initial)" {
            snapshot.initial = true;
        } else if let Some(path) = row.strip_prefix(b"? ") {
            snapshot.changes.push(Change { path: relative_path(path)?, index: '?', worktree: '?', unmerged: false });
        } else if row.starts_with(b"1 ") || row.starts_with(b"u ") {
            let unmerged = row[0] == b'u';
            let mut fields = row.splitn(if unmerged { 11 } else { 9 }, |&b| b == b' ');
            fields.next();
            let xy = fields.next().filter(|xy| xy.len() == 2).ok_or("Некорректный статус Git")?;
            let path = fields.nth(if unmerged { 8 } else { 6 }).ok_or("Git не вернул путь файла")?;
            snapshot.changes.push(Change { path: relative_path(path)?, index: xy[0] as char, worktree: xy[1] as char, unmerged });
        }
    }
    Ok(snapshot)
}

/// Путь из статуса должен оставаться внутри рабочего дерева, прежде
/// чем IDE использует его для диффа или чтения файла.
fn relative_path(bytes: &[u8]) -> Result<PathBuf> {
    let path = native_path(bytes)?;
    if path.as_os_str().is_empty() || !path.components().all(|c| matches!(c, std::path::Component::Normal(_))) {
        return Err("Git вернул некорректный относительный путь".into());
    }
    Ok(path)
}

fn native_path(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
    }
    #[cfg(not(unix))]
    {
        let path = std::str::from_utf8(bytes).map_err(|e| format!("Путь Git не в UTF-8: {e}"))?;
        Ok(PathBuf::from(path))
    }
}

/// Преобразование только для интерфейса. Управляющие символы из вывода
/// Git не должны попадать в терминал.
pub fn display(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter_map(|c| match c {
            '\t' => Some(' '),
            '\r' => None,
            c if c.is_control() && c != '\n' => Some('�'),
            c => Some(c),
        })
        .collect()
}

pub fn background<T: Send + 'static>(task: impl FnOnce() -> T + Send + 'static) -> std::io::Result<Receiver<T>> {
    let (send, receive) = mpsc::channel();
    thread::Builder::new().name("tvstudio-git".into()).spawn(move || {
        let _ = send.send(task());
    })?;
    Ok(receive)
}

pub struct Branch {
    pub name: String,
    pub remote: bool,
    pub current: bool,
}

pub struct Commit {
    pub oid: String,
    pub label: String,
}

pub enum Switch<'a> {
    Local(&'a str),
    Track(&'a str),
    Create(&'a str),
}

pub struct WorktreeUpdate {
    pub snapshot: Snapshot,
    /// Пути относительно корня, изменённые между старым и новым HEAD.
    pub changed: Option<Vec<PathBuf>>,
}

impl Git<'_> {
    /// Символические ссылки наподобие origin/HEAD показывают ветку
    /// по умолчанию, поэтому их исключаем из списка переключения.
    pub fn branches(&self) -> Result<Vec<Branch>> {
        let raw = self.run(["for-each-ref", "--format=%(HEAD)%00%(refname)%00%(symref)", "refs/heads", "refs/remotes"])?;
        let mut branches = Vec::new();
        for row in raw.split(|&b| b == b'\n').filter(|r| !r.is_empty()) {
            let mut fields = row.split(|&b| b == 0);
            let current = fields.next() == Some(b"*");
            let reference = fields.next().ok_or("Git не вернул имя ветки")?;
            if fields.next().is_some_and(|symbol| !symbol.is_empty() && symbol != b"\r") {
                continue;
            }
            let (name, remote) = if let Some(name) = reference.strip_prefix(b"refs/heads/") {
                (name, false)
            } else if let Some(name) = reference.strip_prefix(b"refs/remotes/") {
                (name, true)
            } else {
                continue;
            };
            branches.push(Branch { name: ref_name(name)?, remote, current });
        }
        Ok(branches)
    }

    /// Запрашивается только по кнопке «Список веток». Символический HEAD
    /// позволяет подписать вариант «Ветка по умолчанию».
    pub fn remote_branches(&self, url: &str) -> Result<(Option<String>, Vec<String>)> {
        let raw = self.run_network(["ls-remote", "--symref", "--", url, "HEAD", "refs/heads/*"], 20)?;
        let mut default = None;
        let mut branches = Vec::new();
        for row in raw.split(|&b| b == b'\n') {
            let Some(separator) = row.iter().position(|&b| b == b'\t') else { continue };
            let object = &row[..separator];
            let name = &row[separator + 1..];
            let name = name.strip_suffix(b"\r").unwrap_or(name);
            if name == b"HEAD" {
                if let Some(target) = object.strip_prefix(b"ref: refs/heads/") {
                    default = Some(ref_name(target)?);
                }
            } else if let Some(branch) = name.strip_prefix(b"refs/heads/") {
                branches.push(ref_name(branch)?);
            }
        }
        Ok((default, branches))
    }

    pub fn switch_branch(&self, branch: Switch<'_>) -> Result<WorktreeUpdate> {
        let root = self.root()?;
        let git = Git { cwd: &root, ..*self };
        let old = git.head();
        match branch {
            Switch::Local(name) => git.run(["switch", "--no-guess", "--", name])?,
            Switch::Track(name) => {
                let target = format!("refs/remotes/{name}");
                git.run(["switch", "--track", "--", &target])?
            }
            Switch::Create(name) => git.run(["switch", "-c", name])?,
        };
        git.worktree_update(old.as_deref())
    }

    pub fn fetch_upstream(&self) -> Result<()> {
        self.run(["rev-parse", "--verify", "@{upstream}"])?;
        self.run(["fetch"])?;
        Ok(())
    }

    pub fn fast_forward(&self) -> Result<WorktreeUpdate> {
        let root = self.root()?;
        let git = Git { cwd: &root, ..*self };
        let old = git.head();
        git.run(["merge", "--ff-only", "--no-autostash", "--no-edit", "@{upstream}"])?;
        git.worktree_update(old.as_deref())
    }

    fn head(&self) -> Option<String> {
        self.run(["rev-parse", "--verify", "HEAD"]).ok().map(|raw| display(&raw).trim().to_owned())
    }

    /// Восстанавливаем список изменённых файлов по двум HEAD.
    /// При отсутствии сравнения IDE перечитывает открытые документы.
    fn worktree_update(&self, old: Option<&str>) -> Result<WorktreeUpdate> {
        let snapshot = self.snapshot()?;
        let changed = old.zip(self.head()).and_then(|(old, head)| {
            let raw = self.run(["diff", "--no-renames", "--name-only", "-z", old, &head, "--"]).ok()?;
            raw.split(|&b| b == 0).filter(|row| !row.is_empty()).map(relative_path).collect::<Result<Vec<_>>>().ok()
        });
        Ok(WorktreeUpdate { snapshot, changed })
    }

    pub fn history(&self) -> Result<Vec<Commit>> {
        if self.snapshot()?.initial {
            return Ok(Vec::new());
        }
        let raw = self.run(["log", "-100", "--date=short", "--format=%H%x00%h  %ad  %s", "HEAD", "--"])?;
        Ok(raw
            .split(|&b| b == b'\n')
            .filter_map(|row| {
                let separator = row.iter().position(|&b| b == 0)?;
                Some(Commit { oid: display(&row[..separator]), label: display(&row[separator + 1..]).trim_end().into() })
            })
            .collect())
    }

    pub fn show_commit(&self, oid: &str) -> Result<String> {
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Некорректный идентификатор коммита".into());
        }
        self.run(["show", "--no-ext-diff", "--no-textconv", "--no-color", "--format=fuller", oid, "--"]).map(|raw| display(&raw))
    }
}

fn ref_name(raw: &[u8]) -> Result<String> {
    String::from_utf8(raw.to_vec()).map_err(|_| "Имя ветки Git не в UTF-8".into())
}

/// Каталог клонирования по имени последнего компонента URL.
/// Имя берём из последнего компонента HTTPS, SSH или локального пути.
/// Оно должно быть одним компонентом пути, пригодным для join.
pub fn clone_destination(parent: &Path, url: &str) -> Result<PathBuf> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Укажите адрес репозитория.".into());
    }
    if let Some(address) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        if address.split('/').next().is_some_and(|authority| authority.contains('@')) {
            return Err("Введите URL без учётных данных. Для HTTPS Git использует credential helper.".into());
        }
    }
    let path = if let Some((_, rest)) = url.split_once("://") { rest.split_once('/').map(|(_, path)| path).unwrap_or("") } else { url };
    let path = path.trim_end_matches(['/', '\\']);
    let path = path.strip_suffix("/.git").or_else(|| path.strip_suffix("\\.git")).unwrap_or(path);
    let leaf = path.rsplit(['/', '\\', ':']).next().unwrap_or_default();
    let name = leaf.strip_suffix(".git").or_else(|| leaf.strip_suffix(".bundle")).unwrap_or(leaf);
    if name.is_empty() || !matches!(Path::new(name).components().collect::<Vec<_>>().as_slice(), [std::path::Component::Normal(_)]) {
        return Err("Не удалось получить имя папки из адреса репозитория.".into());
    }
    Ok(parent.join(name))
}

#[cfg(test)]
mod clone_tests;

#[cfg(test)]
mod timeout_tests;
