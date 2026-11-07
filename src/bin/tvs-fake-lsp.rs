//! Учебный сервер LSP для тестов TV Studio.
//!
//! Говорит на том же протоколе, что rust-analyzer (JSON-RPC через
//! stdin/stdout, заголовок Content-Length), но «понимает» код очень
//! просто — этого достаточно, чтобы проверить клиент и среду:
//! * диагностика: слово `ОШИБКА` в строке — ошибка, `warn_me` — предупреждение;
//! * дополнение: всегда `push_str` (сниппет с textEdit), `len`, `capacity`;
//! * определение: строка `fn <слово>` в любом открытом документе;
//! * ссылки: все вхождения слова целиком во всех открытых документах.
//!
//! Перед ответом на initialize сервер сам спрашивает клиента
//! (`workspace/configuration`, `window/workDoneProgress/create`) и
//! сообщает ход «индексации» — клиент обязан это пережить.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

use tvstudio::json::{self, Json};

fn read<R: BufRead>(r: &mut R) -> Option<Json> {
    let mut len = 0usize;
    loop {
        let mut l = String::new();
        if r.read_line(&mut l).ok()? == 0 {
            return None;
        }
        let t = l.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok()?;
        }
    }
    let mut b = vec![0; len];
    r.read_exact(&mut b).ok()?;
    json::parse(&String::from_utf8_lossy(&b)).ok()
}

fn send(m: Json) {
    let body = m.to_compact();
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = out.flush();
}

fn reply(id: &Json, result: Json) {
    send(Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", id.clone()), ("result", result)]));
}

fn notify(method: &str, params: Json) {
    send(Json::obj(vec![("jsonrpc", Json::s("2.0")), ("method", Json::s(method)), ("params", params)]));
}

fn pos(line: usize, col: usize) -> Json {
    Json::obj(vec![("line", Json::i(line as i64)), ("character", Json::i(col as i64))])
}

fn range(line: usize, a: usize, b: usize) -> Json {
    Json::obj(vec![("start", pos(line, a)), ("end", pos(line, b))])
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Слово в позиции (столбцы — в символах: сервер выбирает utf-32).
fn word_at(text: &str, line: usize, col: usize) -> String {
    let l: Vec<char> = text.lines().nth(line).unwrap_or("").chars().collect();
    let mut a = col.min(l.len());
    while a > 0 && is_ident(l[a - 1]) {
        a -= 1;
    }
    let mut b = col.min(l.len());
    while b < l.len() && is_ident(l[b]) {
        b += 1;
    }
    l[a..b].iter().collect()
}

/// Все вхождения слова целиком: (строка, столбец).
fn occurrences(text: &str, word: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let w: Vec<char> = word.chars().collect();
    if w.is_empty() {
        return out;
    }
    for (li, line) in text.lines().enumerate() {
        let c: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i + w.len() <= c.len() {
            if c[i..i + w.len()] == w[..] && (i == 0 || !is_ident(c[i - 1])) && (i + w.len() == c.len() || !is_ident(c[i + w.len()])) {
                out.push((li, i));
                i += w.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

fn publish(uri: &str, text: &str) {
    let mut diags = Vec::new();
    for (word, sev, msg) in [("ОШИБКА", 1, "найдена ОШИБКА"), ("warn_me", 2, "предупреждение warn_me")] {
        for (l, c) in occurrences(text, word) {
            diags.push(Json::obj(vec![
                ("range", range(l, c, c + word.chars().count())),
                ("severity", Json::i(sev)),
                ("code", Json::s("E9999")),
                ("source", Json::s("fake")),
                ("message", Json::s(msg)),
            ]));
        }
    }
    notify("textDocument/publishDiagnostics", Json::obj(vec![("uri", Json::s(uri)), ("diagnostics", Json::Arr(diags))]));
}

fn main() {
    let mut docs: HashMap<String, String> = HashMap::new();
    let mut r = BufReader::new(std::io::stdin().lock());
    let mut next = 1000;
    while let Some(m) = read(&mut r) {
        let method = m.str("method");
        let id = m.get("id").cloned();
        let p = m.get("params").cloned().unwrap_or(Json::Null);
        let doc_uri = p.get("textDocument").map(|t| t.str("uri")).unwrap_or_default();
        let at = p.get("position").map(|x| (x.int("line", 0) as usize, x.int("character", 0) as usize)).unwrap_or((0, 0));
        match method.as_str() {
            "initialize" => {
                // Вопросы к клиенту до ответа: клиент должен на них ответить.
                for (meth, params) in [
                    ("workspace/configuration", json::parse(r#"{"items":[{"section":"rust-analyzer"}]}"#).unwrap()),
                    ("window/workDoneProgress/create", json::parse(r#"{"token":"idx"}"#).unwrap()),
                ] {
                    next += 1;
                    send(Json::obj(vec![("jsonrpc", Json::s("2.0")), ("id", Json::i(next)), ("method", Json::s(meth)), ("params", params)]));
                }
                notify("$/progress", json::parse(r#"{"token":"idx","value":{"kind":"begin","title":"Indexing","percentage":0}}"#).unwrap());
                notify("$/progress", json::parse(r#"{"token":"idx","value":{"kind":"end"}}"#).unwrap());
                let offered = p.get("capabilities").and_then(|c| c.get("general")).map(|g| g.strings("positionEncodings")).unwrap_or_default();
                let enc = if offered.iter().any(|e| e == "utf-32") { "utf-32" } else { "utf-16" };
                let caps = json::parse(&format!(
                    r#"{{"positionEncoding":"{}","textDocumentSync":1,"completionProvider":{{"triggerCharacters":[".",":"]}},"definitionProvider":true,"referencesProvider":true}}"#,
                    enc
                ))
                .unwrap();
                reply(
                    id.as_ref().unwrap(),
                    Json::obj(vec![
                        ("capabilities", caps),
                        ("serverInfo", Json::obj(vec![("name", Json::s("fake-lsp")), ("version", Json::s("1.0"))])),
                    ]),
                );
            }
            "textDocument/didOpen" => {
                let t = p.get("textDocument").cloned().unwrap_or(Json::Null);
                docs.insert(t.str("uri"), t.str("text"));
                publish(&t.str("uri"), &t.str("text"));
            }
            "textDocument/didChange" => {
                let text = p.arr("contentChanges").last().map(|c| c.str("text")).unwrap_or_default();
                docs.insert(doc_uri.clone(), text.clone());
                publish(&doc_uri, &text);
            }
            "textDocument/didClose" => {
                docs.remove(&doc_uri);
            }
            "textDocument/completion" => {
                let text = docs.get(&doc_uri).cloned().unwrap_or_default();
                // Только левая часть слова (до курсора) — её заменяет textEdit.
                let l: Vec<char> = text.lines().nth(at.0).unwrap_or("").chars().collect();
                let mut a = at.1.min(l.len());
                while a > 0 && is_ident(l[a - 1]) {
                    a -= 1;
                }
                let items = Json::Arr(vec![
                    Json::obj(vec![
                        ("label", Json::s("push_str(…)")),
                        ("filterText", Json::s("push_str")),
                        ("kind", Json::i(2)),
                        ("detail", Json::s("fn(&mut self, &str)")),
                        ("insertTextFormat", Json::i(2)),
                        ("sortText", Json::s("1")),
                        ("textEdit", Json::obj(vec![("range", range(at.0, a, at.1)), ("newText", Json::s("push_str(${1:s})$0"))])),
                    ]),
                    Json::obj(vec![("label", Json::s("len")), ("kind", Json::i(2)), ("insertText", Json::s("len()")), ("sortText", Json::s("2"))]),
                    Json::obj(vec![("label", Json::s("capacity")), ("kind", Json::i(2)), ("sortText", Json::s("3"))]),
                ]);
                reply(id.as_ref().unwrap(), Json::obj(vec![("isIncomplete", Json::Bool(false)), ("items", items)]));
            }
            "textDocument/definition" => {
                let text = docs.get(&doc_uri).cloned().unwrap_or_default();
                let w = word_at(&text, at.0, at.1);
                let mut res = Json::Null;
                for (uri, t) in &docs {
                    for (l, c) in occurrences(t, &w) {
                        let line = t.lines().nth(l).unwrap_or("");
                        let before: String = line.chars().take(c).collect();
                        if before.trim_end().ends_with("fn") {
                            res = Json::obj(vec![("uri", Json::s(uri)), ("range", range(l, c, c + w.chars().count()))]);
                        }
                    }
                }
                reply(id.as_ref().unwrap(), res);
            }
            "textDocument/references" => {
                let text = docs.get(&doc_uri).cloned().unwrap_or_default();
                let w = word_at(&text, at.0, at.1);
                let mut v = Vec::new();
                let mut uris: Vec<&String> = docs.keys().collect();
                uris.sort();
                for uri in uris {
                    for (l, c) in occurrences(&docs[uri], &w) {
                        v.push(Json::obj(vec![("uri", Json::s(uri)), ("range", range(l, c, c + w.chars().count()))]));
                    }
                }
                reply(id.as_ref().unwrap(), Json::Arr(v));
            }
            "shutdown" => reply(id.as_ref().unwrap(), Json::Null),
            "exit" => return,
            _ => {
                // Неизвестный запрос — пустой ответ; ответы клиента на наши вопросы — пропускаем.
                if let (Some(id), false) = (&id, method.is_empty()) {
                    reply(id, Json::Null);
                }
            }
        }
    }
}
