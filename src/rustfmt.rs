//! Форматирование исходного кода Rust программой rustfmt.
//!
//! Текст подаётся rustfmt через stdin, результат читается из stdout —
//! файл на диске не трогается (его потом сохраняет редактор). rustfmt
//! запускается в каталоге файла, поэтому находит `rustfmt.toml` /
//! `.rustfmt.toml` проекта; редакция языка берётся из ближайшего
//! `Cargo.toml` (иначе 2021).

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Редакция Rust из ближайшего (вверх по каталогам) `Cargo.toml`.
pub fn edition_for(file: &Path) -> String {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if let Ok(t) = std::fs::read_to_string(d.join("Cargo.toml")) {
            for line in t.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("edition") {
                    let v = rest.trim_start().trim_start_matches('=').trim().trim_matches('"');
                    if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) {
                        return v.to_string();
                    }
                }
            }
            return "2021".into();
        }
        dir = d.parent();
    }
    "2021".into()
}

/// Отформатировать текст `src` файла `file` командой `cmd` (обычно `rustfmt`).
/// Ошибка — текст rustfmt (например, синтаксическая ошибка в коде).
pub fn format(src: &str, file: &Path, cmd: &str) -> Result<String, String> {
    let mut parts = cmd.split_whitespace();
    let prog = parts.next().ok_or("не задана программа форматирования")?;
    let dir = file.parent().filter(|d| d.is_dir()).map(|d| d.to_path_buf()).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let mut child = Command::new(prog)
        .args(parts)
        .args(["--edition", &edition_for(file), "--emit", "stdout", "--quiet"])
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("не удалось запустить «{}»: {}", cmd, e))?;
    // Пишем в отдельном потоке: большой файл иначе мог бы упереться в
    // заполненный канал вывода.
    let mut stdin = child.stdin.take().ok_or("нет stdin")?;
    let text = src.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(text.as_bytes());
    });
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let _ = writer.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() || out.is_empty() && !src.trim().is_empty() {
        let msg = err.lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join("\n");
        return Err(if msg.is_empty() { format!("rustfmt завершился с кодом {:?}", status.code()) } else { msg });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_rustfmt() -> bool {
        Command::new("rustfmt").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    #[test]
    fn formats_and_reports_errors() {
        if !have_rustfmt() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("tvs_fmt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\nedition = \"2021\"\n").unwrap();
        let f = dir.join("a.rs");
        assert_eq!(edition_for(&f), "2021");
        let out = format("fn  main( ){let x=1;println!(\"{}\",x);}", &f, "rustfmt").unwrap();
        assert_eq!(out, "fn main() {\n    let x = 1;\n    println!(\"{}\", x);\n}\n");
        assert!(format("fn main( {", &f, "rustfmt").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
