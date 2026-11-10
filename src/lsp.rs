//! Клиент Language Server Protocol (rust-analyzer для `.rs`, taplo для `.toml`).
//!
//! Сервер — отдельный процесс; обмен — JSON-RPC 2.0 через stdin/stdout,
//! каждое сообщение предваряется заголовком `Content-Length: N\r\n\r\n`.
//! Чтение идёт в фоновом потоке, разобранные сообщения приходят в
//! основной поток через канал: среда забирает их в простое
//! ([`LspClient::poll`]) — интерфейс не ждёт сервер никогда.
//!
//! Что поддержано: синхронизация документов (`didOpen` / `didChange`
//! целиком / `didSave` / `didClose`), диагностика (`publishDiagnostics`),
//! дополнение (`textDocument/completion`), переход к определению
//! (`textDocument/definition`), поиск ссылок (`textDocument/references`),
//! ход индексации (`$/progress`).
//!
//! Позиции: LSP считает столбцы в единицах UTF-16, редактор — в символах.
//! Клиент предлагает серверу UTF-32 (= символы; rust-analyzer умеет) и
//! пересчитывает, если сервер выбрал UTF-16.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use crate::json::{self, Json};

// ======================================================================
// URI и позиции
// ======================================================================

/// Путь → `file:///…` (с процентным кодированием, как в VS Code).
pub fn path_to_uri(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    let s = if s.starts_with('/') { s } else { format!("/{}", s) };
    let mut out = String::from("file://");
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "/-._~".contains(c) {
            out.push(c);
        } else if c == ':' {
            out.push_str("%3A");
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// `file:///…` → путь.
pub fn uri_to_path(uri: &str) -> PathBuf {
    let rest = uri.strip_prefix("file://").unwrap_or(uri);
    let bytes = rest.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let s = String::from_utf8_lossy(&out).to_string();
    // Windows: «/C:/…» → «C:/…».
    let b = s.as_bytes();
    if b.len() > 2 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() {
        return PathBuf::from(&s[1..]);
    }
    PathBuf::from(s)
}

/// Кодировка столбцов, о которой договорились с сервером.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf16,
    Utf32,
    Utf8,
}

/// Столбец в символах → столбец в единицах сервера.
pub fn col_to_lsp(line: &str, col: usize, enc: Encoding) -> usize {
    let pre = line.chars().take(col);
    match enc {
        Encoding::Utf32 => col,
        Encoding::Utf16 => pre.map(|c| c.len_utf16()).sum(),
        Encoding::Utf8 => pre.map(|c| c.len_utf8()).sum(),
    }
}

/// Столбец сервера → столбец в символах.
pub fn col_from_lsp(line: &str, col: usize, enc: Encoding) -> usize {
    if enc == Encoding::Utf32 {
        return col;
    }
    let mut units = 0;
    for (i, c) in line.chars().enumerate() {
        if units >= col {
            return i;
        }
        units += if enc == Encoding::Utf16 { c.len_utf16() } else { c.len_utf8() };
    }
    line.chars().count()
}

// ======================================================================
// Данные
// ======================================================================

/// Важность диагностики.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error = 1,
    Warning = 2,
    Info = 3,
    Hint = 4,
}

/// Позиция в документе (строка и столбец в единицах сервера, с 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LPos {
    pub line: usize,
    pub col: usize,
}

/// Диапазон.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LRange {
    pub start: LPos,
    pub end: LPos,
}

/// Диагностика.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub range: LRange,
    pub severity: Severity,
    pub message: String,
    /// Код (E0308) и источник (rustc, rust-analyzer).
    pub code: String,
    pub source: String,
}

/// Место в файле.
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub path: PathBuf,
    pub range: LRange,
}

/// Вариант дополнения.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    /// Подробности (тип, сигнатура).
    pub detail: String,
    /// Вид: fn, struct, field… (для значка).
    pub kind: i64,
    /// Текст вставки (сниппеты уже развёрнуты в обычный текст).
    pub insert: String,
    /// Что заменить (если сервер указал `textEdit`).
    pub range: Option<LRange>,
    /// По чему фильтровать.
    pub filter: String,
    pub sort: String,
}

/// Событие от сервера для среды.
#[derive(Clone, Debug, PartialEq)]
pub enum LspEvent {
    /// Сервер готов (ответ на initialize).
    Ready {
        name: String,
    },
    Diagnostics {
        path: PathBuf,
        items: Vec<Diagnostic>,
    },
    Completion {
        id: i64,
        items: Vec<CompletionItem>,
    },
    Locations {
        id: i64,
        items: Vec<Location>,
    },
    /// Ход длительной работы: заголовок, процент; `None` — закончено.
    Progress {
        title: String,
        percent: Option<i64>,
        done: bool,
    },
    /// Ошибка запроса.
    Error {
        id: i64,
        message: String,
    },
    /// Сообщение для пользователя (`window/showMessage`, `logMessage` ошибки).
    Message(String),
    /// Процесс сервера завершился.
    Exited,
}

// ======================================================================
// Разбор ответов
// ======================================================================

fn parse_pos(j: &Json) -> LPos {
    LPos { line: j.int("line", 0).max(0) as usize, col: j.int("character", 0).max(0) as usize }
}

fn parse_range(j: &Json) -> LRange {
    LRange { start: parse_pos(j.get("start").unwrap_or(&Json::Null)), end: parse_pos(j.get("end").unwrap_or(&Json::Null)) }
}

fn pos_json(p: LPos) -> Json {
    Json::obj(vec![("line", Json::i(p.line as i64)), ("character", Json::i(p.col as i64))])
}

/// Развернуть сниппет LSP в обычный текст: `foo(${1:x}, $2)$0` → `foo(x, )`.
pub fn strip_snippet(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' if i + 1 < c.len() => {
                out.push(c[i + 1]);
                i += 2;
            }
            '$' if i + 1 < c.len() && c[i + 1].is_ascii_digit() => {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            '$' if i + 1 < c.len() && c[i + 1] == '{' => {
                // ${N:текст} или ${N|a,b|}
                let mut j = i + 2;
                while j < c.len() && c[j].is_ascii_digit() {
                    j += 1;
                }
                let mut depth = 1;
                let start = j;
                let mut k = j;
                while k < c.len() && depth > 0 {
                    if c[k] == '{' {
                        depth += 1;
                    } else if c[k] == '}' {
                        depth -= 1;
                    }
                    k += 1;
                }
                let inner: String = c[start..k.saturating_sub(1).max(start)].iter().collect();
                if let Some(t) = inner.strip_prefix(':') {
                    out.push_str(&strip_snippet(t));
                } else if let Some(t) = inner.strip_prefix('|') {
                    out.push_str(t.trim_end_matches('|').split(',').next().unwrap_or(""));
                }
                i = k;
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

fn parse_completion(result: &Json) -> Vec<CompletionItem> {
    let items: &[Json] = match result {
        Json::Arr(a) => a,
        Json::Obj(_) => result.arr("items"),
        _ => &[],
    };
    let mut v: Vec<CompletionItem> = items
        .iter()
        .map(|it| {
            let label = it.str("label");
            let snippet = it.int("insertTextFormat", 1) == 2;
            let (raw, range) = match it.get("textEdit") {
                Some(te) => {
                    // TextEdit {range, newText} или InsertReplaceEdit {insert, replace, newText}.
                    let r = te.get("range").or_else(|| te.get("replace")).or_else(|| te.get("insert")).map(parse_range);
                    (te.str("newText"), r)
                }
                None => (Some(it.str("insertText")).filter(|s| !s.is_empty()).unwrap_or_else(|| label.clone()), None),
            };
            let insert = if snippet { strip_snippet(&raw) } else { raw };
            let detail = Some(it.str("detail"))
                .filter(|s| !s.is_empty())
                .or_else(|| it.get("labelDetails").map(|d| format!("{}{}", d.str("detail"), d.str("description"))))
                .unwrap_or_default();
            CompletionItem {
                filter: Some(it.str("filterText")).filter(|s| !s.is_empty()).unwrap_or_else(|| label.clone()),
                sort: Some(it.str("sortText")).filter(|s| !s.is_empty()).unwrap_or_else(|| label.clone()),
                label,
                detail,
                kind: it.int("kind", 0),
                insert,
                range,
            }
        })
        .collect();
    v.sort_by(|a, b| a.sort.cmp(&b.sort));
    v
}

fn parse_locations(result: &Json) -> Vec<Location> {
    let one = |j: &Json| -> Option<Location> {
        // Location {uri, range} или LocationLink {targetUri, targetSelectionRange}.
        let uri = Some(j.str("uri")).filter(|s| !s.is_empty()).or_else(|| Some(j.str("targetUri")).filter(|s| !s.is_empty()))?;
        let range = j.get("range").or_else(|| j.get("targetSelectionRange")).or_else(|| j.get("targetRange")).map(parse_range).unwrap_or_default();
        Some(Location { path: uri_to_path(&uri), range })
    };
    match result {
        Json::Arr(a) => a.iter().filter_map(one).collect(),
        Json::Obj(_) => one(result).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn parse_diagnostics(params: &Json) -> (PathBuf, Vec<Diagnostic>) {
    let path = uri_to_path(&params.str("uri"));
    let items = params
        .arr("diagnostics")
        .iter()
        .map(|d| Diagnostic {
            range: parse_range(d.get("range").unwrap_or(&Json::Null)),
            severity: match d.int("severity", 1) {
                1 => Severity::Error,
                2 => Severity::Warning,
                3 => Severity::Info,
                _ => Severity::Hint,
            },
            message: d.str("message"),
            code: match d.get("code") {
                Some(Json::Str(s)) => s.clone(),
                Some(Json::Num(n)) => format!("{}", *n as i64),
                _ => String::new(),
            },
            source: d.str("source"),
        })
        .collect();
    (path, items)
}

// ======================================================================
// Клиент
// ======================================================================

/// Что было запрошено (чтобы понять ответ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReqKind {
    Initialize,
    Completion,
    Definition,
    References,
    Shutdown,
    Other,
}

/// Клиент LSP: процесс сервера и состояние обмена.
pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Option<Json>>,
    next_id: i64,
    pending: HashMap<i64, ReqKind>,
    /// Открытые документы: версия.
    docs: HashMap<PathBuf, i64>,
    pub encoding: Encoding,
    pub ready: bool,
    pub root: PathBuf,
    /// Язык документов (`languageId` в `didOpen`): `rust`, `toml`.
    pub language: String,
    /// Сообщения, отправленные до ответа на initialize (сервер их ждёт после).
    queued: Vec<Json>,
    /// Заголовки работ `$/progress` по токенам.
    progress: HashMap<String, String>,
    /// Последние строки stderr сервера (для сообщения об ошибке).
    pub stderr_tail: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

fn read_message<R: BufRead>(r: &mut R) -> Option<Json> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut buf = vec![0u8; len?];
    r.read_exact(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    Some(json::parse(&text).unwrap_or(Json::Null))
}

impl LspClient {
    /// Запустить сервер `command` (например, `rust-analyzer` или
    /// `taplo lsp stdio`) для проекта `root`; `language` — `languageId`
    /// документов, которые ему показываются.
    pub fn start(command: &str, root: &Path, language: &str) -> Result<LspClient, String> {
        let mut parts = command.split_whitespace();
        let prog = parts.next().ok_or("не задана программа сервера")?;
        let mut child = Command::new(prog)
            .args(parts)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("не удалось запустить «{}»: {}", command, e))?;
        let stdin = child.stdin.take().ok_or("нет stdin")?;
        let stdout = child.stdout.take().ok_or("нет stdout")?;
        let stderr = child.stderr.take();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            while let Some(m) = read_message(&mut r) {
                if tx.send(Some(m)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
        let tail = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        if let Some(e) = stderr {
            let tail2 = tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    let mut t = tail2.lock().unwrap();
                    t.push(line);
                    if t.len() > 20 {
                        t.remove(0);
                    }
                }
            });
        }
        let mut c = LspClient {
            child,
            stdin,
            rx,
            next_id: 1,
            pending: HashMap::new(),
            docs: HashMap::new(),
            encoding: Encoding::Utf16,
            ready: false,
            root: root.to_path_buf(),
            language: language.to_string(),
            queued: Vec::new(),
            progress: HashMap::new(),
            stderr_tail: tail,
        };
        let root_uri = path_to_uri(root);
        let name = root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let caps = json::parse(
            r#"{
            "general": {"positionEncodings": ["utf-32", "utf-16"]},
            "window": {"workDoneProgress": true},
            "textDocument": {
                "synchronization": {"didSave": true, "dynamicRegistration": false},
                "publishDiagnostics": {"relatedInformation": false},
                "completion": {"completionItem": {"snippetSupport": true, "labelDetailsSupport": true}, "contextSupport": true},
                "definition": {"linkSupport": true},
                "references": {}
            },
            "workspace": {"configuration": true, "workspaceFolders": true}
        }"#,
        )
        .unwrap();
        let params = Json::obj(vec![
            ("processId", Json::i(std::process::id() as i64)),
            ("clientInfo", Json::obj(vec![("name", Json::s("TV Studio")), ("version", Json::s(env!("CARGO_PKG_VERSION")))])),
            ("rootUri", Json::s(&root_uri)),
            ("workspaceFolders", Json::Arr(vec![Json::obj(vec![("uri", Json::s(&root_uri)), ("name", Json::s(&name))])])),
            ("capabilities", caps),
        ]);
        c.request_now("initialize", params, ReqKind::Initialize);
        Ok(c)
    }

    fn send(&mut self, msg: &Json) {
        let body = msg.to_compact();
        let _ = write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body);
        let _ = self.stdin.flush();
    }

    /// Отправить (или отложить до готовности сервера).
    fn send_or_queue(&mut self, msg: Json) {
        if self.ready {
            self.send(&msg);
        } else {
            self.queued.push(msg);
        }
    }

    fn request_now(&mut self, method: &str, params: Json, kind: ReqKind) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        let m = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", Json::i(id)), ("method", Json::s(method)), ("params", params)]);
        self.send(&m);
        id
    }

    fn request(&mut self, method: &str, params: Json, kind: ReqKind) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        let m = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", Json::i(id)), ("method", Json::s(method)), ("params", params)]);
        self.send_or_queue(m);
        id
    }

    fn notify(&mut self, method: &str, params: Json) {
        let m = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("method", Json::s(method)), ("params", params)]);
        self.send_or_queue(m);
    }

    fn doc_id(p: &Path) -> Json {
        Json::obj(vec![("uri", Json::s(&path_to_uri(p)))])
    }

    /// Открыт ли документ на сервере.
    pub fn is_open(&self, p: &Path) -> bool {
        self.docs.contains_key(p)
    }

    /// Документы, открытые на сервере.
    pub fn open_docs(&self) -> Vec<PathBuf> {
        self.docs.keys().cloned().collect()
    }

    pub fn did_open(&mut self, p: &Path, text: &str) {
        self.docs.insert(p.to_path_buf(), 1);
        let item = Json::obj(vec![
            ("uri", Json::s(&path_to_uri(p))),
            ("languageId", Json::s(&self.language)),
            ("version", Json::i(1)),
            ("text", Json::s(text)),
        ]);
        self.notify("textDocument/didOpen", Json::obj(vec![("textDocument", item)]));
    }

    /// Новое содержимое документа (полная синхронизация).
    pub fn did_change(&mut self, p: &Path, text: &str) {
        let v = self.docs.get(p).copied().unwrap_or(0) + 1;
        self.docs.insert(p.to_path_buf(), v);
        let id = Json::obj(vec![("uri", Json::s(&path_to_uri(p))), ("version", Json::i(v))]);
        self.notify(
            "textDocument/didChange",
            Json::obj(vec![("textDocument", id), ("contentChanges", Json::Arr(vec![Json::obj(vec![("text", Json::s(text))])]))]),
        );
    }

    pub fn did_save(&mut self, p: &Path) {
        self.notify("textDocument/didSave", Json::obj(vec![("textDocument", Self::doc_id(p))]));
    }

    pub fn did_close(&mut self, p: &Path) {
        self.docs.remove(p);
        self.notify("textDocument/didClose", Json::obj(vec![("textDocument", Self::doc_id(p))]));
    }

    fn pos_params(p: &Path, pos: LPos) -> Vec<(&'static str, Json)> {
        vec![("textDocument", Self::doc_id(p)), ("position", pos_json(pos))]
    }

    pub fn completion(&mut self, p: &Path, pos: LPos, trigger: Option<char>) -> i64 {
        let mut v = Self::pos_params(p, pos);
        v.push((
            "context",
            match trigger {
                Some(c) => Json::obj(vec![("triggerKind", Json::i(2)), ("triggerCharacter", Json::s(&c.to_string()))]),
                None => Json::obj(vec![("triggerKind", Json::i(1))]),
            },
        ));
        self.request("textDocument/completion", Json::obj(v), ReqKind::Completion)
    }

    pub fn definition(&mut self, p: &Path, pos: LPos) -> i64 {
        self.request("textDocument/definition", Json::obj(Self::pos_params(p, pos)), ReqKind::Definition)
    }

    pub fn references(&mut self, p: &Path, pos: LPos) -> i64 {
        let mut v = Self::pos_params(p, pos);
        v.push(("context", Json::obj(vec![("includeDeclaration", Json::Bool(true))])));
        self.request("textDocument/references", Json::obj(v), ReqKind::References)
    }

    /// Ответить на запрос сервера.
    fn reply(&mut self, id: &Json, result: Json) {
        let m = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", id.clone()), ("result", result)]);
        self.send(&m);
    }

    /// Забрать события от сервера.
    pub fn poll(&mut self) -> Vec<LspEvent> {
        let mut out = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            let Some(m) = m else {
                out.push(LspEvent::Exited);
                break;
            };
            self.handle(m, &mut out);
        }
        out
    }

    fn handle(&mut self, m: Json, out: &mut Vec<LspEvent>) {
        let method = m.str("method");
        let id = m.get("id").cloned();
        // Запрос сервера к клиенту.
        if let (false, Some(id)) = (method.is_empty(), &id) {
            let id = id.clone();
            let result = match method.as_str() {
                // Настройки: по одной «по умолчанию» на каждый запрошенный раздел.
                // rust-analyzer ждёт null; taplo разбирает раздел как объект
                // настроек, и пустой объект для него — «всё по умолчанию».
                "workspace/configuration" => {
                    let dflt = if self.language == "rust" { Json::Null } else { Json::obj(vec![]) };
                    Json::Arr(m.get("params").map(|p| p.arr("items")).unwrap_or(&[]).iter().map(|_| dflt.clone()).collect())
                }
                _ => Json::Null,
            };
            self.reply(&id, result);
            return;
        }
        // Уведомление.
        if !method.is_empty() {
            let params = m.get("params").cloned().unwrap_or(Json::Null);
            match method.as_str() {
                "textDocument/publishDiagnostics" => {
                    let (path, items) = parse_diagnostics(&params);
                    out.push(LspEvent::Diagnostics { path, items });
                }
                "$/progress" => {
                    let token = match params.get("token") {
                        Some(Json::Str(s)) => s.clone(),
                        Some(Json::Num(n)) => n.to_string(),
                        _ => String::new(),
                    };
                    let v = params.get("value").cloned().unwrap_or(Json::Null);
                    match v.str("kind").as_str() {
                        "begin" => {
                            let t = v.str("title");
                            self.progress.insert(token, t.clone());
                            out.push(LspEvent::Progress { title: t, percent: v.get("percentage").and_then(|x| x.as_i64()), done: false });
                        }
                        "report" => {
                            let t = self.progress.get(&token).cloned().unwrap_or_default();
                            let msg = v.str("message");
                            let title = if msg.is_empty() { t } else { format!("{} {}", t, msg) };
                            out.push(LspEvent::Progress { title, percent: v.get("percentage").and_then(|x| x.as_i64()), done: false });
                        }
                        "end" => {
                            let t = self.progress.remove(&token).unwrap_or_default();
                            out.push(LspEvent::Progress { title: t, percent: None, done: self.progress.is_empty() });
                        }
                        _ => {}
                    }
                }
                "window/showMessage" => out.push(LspEvent::Message(params.str("message"))),
                "window/logMessage" if params.int("type", 4) == 1 => out.push(LspEvent::Message(params.str("message"))),
                _ => {}
            }
            return;
        }
        // Ответ на наш запрос.
        let Some(id) = id.and_then(|i| i.as_i64()) else { return };
        let kind = self.pending.remove(&id).unwrap_or(ReqKind::Other);
        if let Some(err) = m.get("error") {
            out.push(LspEvent::Error { id, message: err.str("message") });
            return;
        }
        let result = m.get("result").cloned().unwrap_or(Json::Null);
        match kind {
            ReqKind::Initialize => {
                let caps = result.get("capabilities").cloned().unwrap_or(Json::Null);
                self.encoding = match caps.str("positionEncoding").as_str() {
                    "utf-32" => Encoding::Utf32,
                    "utf-8" => Encoding::Utf8,
                    _ => Encoding::Utf16,
                };
                self.ready = true;
                let init = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("method", Json::s("initialized")), ("params", Json::obj(vec![]))]);
                self.send(&init);
                for q in std::mem::take(&mut self.queued) {
                    self.send(&q);
                }
                let name = result.get("serverInfo").map(|s| format!("{} {}", s.str("name"), s.str("version"))).unwrap_or_default();
                out.push(LspEvent::Ready { name: name.trim().to_string() });
            }
            ReqKind::Completion => out.push(LspEvent::Completion { id, items: parse_completion(&result) }),
            ReqKind::Definition | ReqKind::References => out.push(LspEvent::Locations { id, items: parse_locations(&result) }),
            ReqKind::Shutdown | ReqKind::Other => {}
        }
    }

    /// Вид запроса по номеру (пока не пришёл ответ).
    pub fn pending_kind(&self, id: i64) -> Option<ReqKind> {
        self.pending.get(&id).copied()
    }

    /// Вежливо остановить сервер (shutdown → exit), затем — принудительно.
    pub fn stop(mut self) {
        if self.ready {
            let id = self.next_id;
            self.pending.insert(id, ReqKind::Shutdown);
            let m = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", Json::i(id)), ("method", Json::s("shutdown")), ("params", Json::Null)]);
            self.send(&m);
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(500) {
                if let Ok(Some(m)) = self.rx.recv_timeout(Duration::from_millis(50)) {
                    if m.get("id").and_then(|i| i.as_i64()) == Some(id) {
                        break;
                    }
                }
            }
            let e = Json::obj(vec![("jsonrpc", Json::s("2.0")), ("method", Json::s("exit")), ("params", Json::Null)]);
            self.send(&e);
        }
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(500) {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Жив ли процесс сервера.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris() {
        let p = Path::new("/home/кот/my app/src/main.rs");
        let u = path_to_uri(p);
        assert_eq!(u, "file:///home/%D0%BA%D0%BE%D1%82/my%20app/src/main.rs");
        assert_eq!(uri_to_path(&u), p);
        assert_eq!(uri_to_path("file:///C%3A/work/a.rs"), PathBuf::from("C:/work/a.rs"));
        assert_eq!(uri_to_path("file:///c:/work/a.rs"), PathBuf::from("c:/work/a.rs"));
    }

    #[test]
    fn columns() {
        let l = "let 😀 = ы;";
        assert_eq!(col_to_lsp(l, 6, Encoding::Utf16), 7);
        assert_eq!(col_from_lsp(l, 7, Encoding::Utf16), 6);
        assert_eq!(col_to_lsp(l, 6, Encoding::Utf32), 6);
        assert_eq!(col_to_lsp(l, 9, Encoding::Utf8), 13);
        assert_eq!(col_from_lsp(l, 13, Encoding::Utf8), 9);
    }

    #[test]
    fn snippets() {
        assert_eq!(strip_snippet("push_str(${1:string})$0"), "push_str(string)");
        assert_eq!(strip_snippet("fn ${1:name}($2) {\n    $0\n}"), "fn name() {\n    \n}");
        assert_eq!(strip_snippet("a\\$b"), "a$b");
        assert_eq!(strip_snippet("${1|one,two|}"), "one");
    }

    #[test]
    fn parses_server_replies() {
        let c = json::parse(r#"{"isIncomplete":false,"items":[{"label":"push_str(…)","kind":2,"detail":"fn(&mut self, &str)","insertTextFormat":2,"textEdit":{"range":{"start":{"line":1,"character":6},"end":{"line":1,"character":8}},"newText":"push_str(${1:s})$0"},"sortText":"b"},{"label":"len","sortText":"a"}]}"#).unwrap();
        let v = parse_completion(&c);
        assert_eq!(v[0].label, "len");
        assert_eq!(v[0].insert, "len");
        assert_eq!(v[1].insert, "push_str(s)");
        assert_eq!(v[1].range.unwrap().start, LPos { line: 1, col: 6 });
        let l = json::parse(r#"[{"targetUri":"file:///a/b.rs","targetRange":{"start":{"line":3,"character":0},"end":{"line":9,"character":1}},"targetSelectionRange":{"start":{"line":3,"character":3},"end":{"line":3,"character":7}}}]"#).unwrap();
        let v = parse_locations(&l);
        assert_eq!(v[0].path, PathBuf::from("/a/b.rs"));
        assert_eq!(v[0].range.start, LPos { line: 3, col: 3 });
        let d = json::parse(r#"{"uri":"file:///a/b.rs","diagnostics":[{"range":{"start":{"line":2,"character":4},"end":{"line":2,"character":9}},"severity":1,"code":"E0308","source":"rustc","message":"mismatched types"}]}"#).unwrap();
        let (p, v) = parse_diagnostics(&d);
        assert_eq!(p, PathBuf::from("/a/b.rs"));
        assert_eq!((v[0].severity, v[0].code.as_str()), (Severity::Error, "E0308"));
    }
}
