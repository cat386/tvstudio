//! Подсказки без сервера LSP: слова из файлов проекта.
//!
//! Если rust-analyzer не запущен (не установлен, выключен в настройках или
//! упал), список дополнения всё равно что-то предлагает:
//!
//! * крейты из `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
//!   и `[workspace.dependencies]` (у корня и у участников workspace) — с `_`
//!   вместо `-`, как их пишут в коде, — и `std`, `core`, `alloc`;
//! * модули — имена файлов `.rs` (у `mod.rs` — имя каталога);
//! * имена (идентификаторы от трёх букв, кроме ключевых слов) из всех файлов
//!   `.rs` проекта (`src`, `tests`, `examples`, `benches` корня и участников)
//!   и из открытого текста.
//!
//! Это не разбор кода, а словарь: варианты не знают типов и области
//! видимости, зато работают всегда и сразу.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::lsp::CompletionItem;
use crate::model::cargo_layout;

/// Вид варианта для значка (как в LSP: 1 — текст, 9 — модуль).
const KIND_TEXT: i64 = 1;
const KIND_MODULE: i64 = 9;

/// Не больше стольких файлов и байт на файл — словарь не должен тормозить.
const MAX_FILES: usize = 3000;
const MAX_FILE_BYTES: u64 = 2_000_000;

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "for", "if", "impl", "in", "let",
    "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while",
];

/// Имена (от трёх букв, не ключевые слова) в тексте.
pub fn idents(text: &str, out: &mut BTreeMap<String, i64>) {
    let mut cur = String::new();
    let mut flush = |w: &mut String| {
        if w.chars().count() >= 3 && !w.chars().next().unwrap().is_ascii_digit() && !KEYWORDS.contains(&w.as_str()) {
            out.entry(std::mem::take(w)).or_insert(KIND_TEXT);
        } else {
            w.clear();
        }
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            flush(&mut cur);
        }
    }
    flush(&mut cur);
}

/// Крейты из зависимостей `Cargo.toml` (имена с `_` вместо `-`).
fn crates(cargo_toml: &Path, out: &mut BTreeMap<String, i64>) {
    let Ok(text) = std::fs::read_to_string(cargo_toml) else { return };
    let Ok(j) = crate::toml::parse(&text) else { return };
    let mut tables = vec![j.get("dependencies"), j.get("dev-dependencies"), j.get("build-dependencies")];
    tables.push(j.get("workspace").and_then(|w| w.get("dependencies")));
    for t in tables.into_iter().flatten() {
        if let crate::json::Json::Obj(pairs) = t {
            // `foo = { package = "real-name" }` — в коде пишут ключ, не package.
            for (name, _) in pairs {
                out.insert(name.replace('-', "_"), KIND_MODULE);
            }
        }
    }
}

/// Файлы `.rs` в каталоге (рекурсивно, без `target` и скрытых).
fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// Словарь проекта: крейты, модули и имена из его файлов `.rs`.
pub fn project_words(dir: &Path) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for c in ["std", "core", "alloc"] {
        out.insert(c.to_string(), KIND_MODULE);
    }
    let layout = cargo_layout(dir);
    let roots: Vec<PathBuf> = std::iter::once(dir.to_path_buf()).chain(layout.members.into_iter().map(|m| m.dir)).collect();
    let mut files = Vec::new();
    for r in &roots {
        crates(&r.join("Cargo.toml"), &mut out);
        for sub in ["src", "tests", "examples", "benches"] {
            rs_files(&r.join(sub), &mut files);
        }
        if r.join("build.rs").is_file() {
            files.push(r.join("build.rs"));
        }
    }
    for f in &files {
        // Модуль: имя файла (у mod.rs — каталога); main/lib/build — не модули.
        let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let module = if stem == "mod" { f.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()) } else { Some(stem) };
        if let Some(m) = module.filter(|m| !["main", "lib", "build"].contains(&m.as_str())) {
            out.insert(m, KIND_MODULE);
        }
        if std::fs::metadata(f).map(|m| m.len() <= MAX_FILE_BYTES).unwrap_or(false) {
            if let Ok(t) = std::fs::read_to_string(f) {
                idents(&t, &mut out);
            }
        }
    }
    out
}

/// Варианты дополнения из словаря (крейты и модули — первыми).
pub fn items(words: &BTreeMap<String, i64>) -> Vec<CompletionItem> {
    let mut v: Vec<CompletionItem> = words
        .iter()
        .map(|(w, &kind)| CompletionItem {
            label: w.clone(),
            detail: if kind == KIND_MODULE { "крейт / модуль".into() } else { String::new() },
            kind,
            insert: w.clone(),
            range: None,
            filter: w.clone(),
            sort: format!("{}{}", if kind == KIND_MODULE { 0 } else { 1 }, w.to_lowercase()),
            edits: Vec::new(),
            resolve: None,
        })
        .collect();
    v.sort_by(|a, b| a.sort.cmp(&b.sort));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_of_project() {
        let base = std::env::temp_dir().join(format!("tvs_words_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("src/net")).unwrap();
        std::fs::create_dir_all(base.join("tools/gen/src")).unwrap();
        std::fs::write(
            base.join("Cargo.toml"),
            "[package]\nname = \"app\"\n\n[dependencies]\nserde_json = \"1\"\nmy-lib = { path = \"../x\" }\n\n[dev-dependencies]\npretty-assertions = \"1\"\n\n[workspace]\nmembers = [\"tools/*\"]\n",
        )
        .unwrap();
        std::fs::write(base.join("src/main.rs"), "mod net;\nfn main() { let config_path = load_config(); }\n// x1 ab let\n").unwrap();
        std::fs::write(base.join("src/net/mod.rs"), "pub struct Connection;\n").unwrap();
        std::fs::write(base.join("tools/gen/Cargo.toml"), "[package]\nname = \"gen\"\n[dependencies]\nclap = \"4\"\n").unwrap();
        std::fs::write(base.join("tools/gen/src/codegen.rs"), "pub fn emit_table() {}\n").unwrap();
        let w = project_words(&base);
        for (k, kind) in [
            ("serde_json", KIND_MODULE),
            ("my_lib", KIND_MODULE),
            ("pretty_assertions", KIND_MODULE),
            ("clap", KIND_MODULE),
            ("std", KIND_MODULE),
            ("net", KIND_MODULE),
            ("codegen", KIND_MODULE),
            ("config_path", KIND_TEXT),
            ("load_config", KIND_TEXT),
            ("Connection", KIND_TEXT),
            ("emit_table", KIND_TEXT),
        ] {
            assert_eq!(w.get(k), Some(&kind), "{}", k);
        }
        for no in ["let", "fn", "x1", "ab", "mod"] {
            assert!(!w.contains_key(no), "{}", no);
        }
        let it = items(&w);
        assert_eq!(it[0].kind, KIND_MODULE, "крейты и модули — первыми");
        let _ = std::fs::remove_dir_all(&base);
    }
}
