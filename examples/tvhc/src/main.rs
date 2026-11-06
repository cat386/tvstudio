//! Порт утилиты `tvhc` (Turbo Vision Help Compiler).
//!
//! Оригинал компилировал исходник справки в двоичный `.hlp` и генерировал
//! заголовок `.h` с константами `hcXXX`. Библиотека на Rust читает исходник
//! справки напрямую, поэтому утилита только проверяет его и создаёт файл
//! с константами контекстов на Rust.
//!
//! Запуск: `cargo run --example tvhc -- справка.txt [константы.rs]`

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tvision::HelpFile;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Turbo Vision Help Compiler (Rust)\n\n  Синтаксис: tvhc <исходник справки> [<файл констант .rs>]");
        return ExitCode::from(1);
    }
    let src = Path::new(&args[0]);
    let out = args.get(1).map(PathBuf::from).unwrap_or_else(|| src.with_extension("rs"));

    let hf = match HelpFile::load(src) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Ошибка: {}: {}", src.display(), e);
            return ExitCode::from(2);
        }
    };
    for w in &hf.warnings {
        eprintln!("Предупреждение: {}", w);
    }
    if let Err(e) = std::fs::write(&out, hf.context_constants()) {
        eprintln!("Ошибка записи {}: {}", out.display(), e);
        return ExitCode::from(3);
    }
    println!("Тем: {}, символов: {}. Константы записаны в {}.", hf.topics.len(), hf.symbols.len(), out.display());
    ExitCode::SUCCESS
}
