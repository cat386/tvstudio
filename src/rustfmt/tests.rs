use super::*;

fn have_rustfmt() -> bool {
    Command::new("rustfmt").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Переводы строк результата — как у исходного текста.
#[test]
fn keeps_newline_style() {
    assert_eq!(same_newlines("a\nb", "x\r\ny\r\n".into()), "x\ny\n");
    assert_eq!(same_newlines("ab", "x\r\n".into()), "x\n");
    assert_eq!(same_newlines("a\r\nb", "x\r\ny\r\n".into()), "x\r\ny\r\n");
    assert_eq!(same_newlines("a\nb", "x\ny\n".into()), "x\ny\n");
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
    // Многострочный текст с \n — результат тоже с \n (на любой ОС).
    let out = format("fn main() {\nlet x=1;\n}\n", &f, "rustfmt").unwrap();
    assert_eq!(out, "fn main() {\n    let x = 1;\n}\n");
    let _ = std::fs::remove_dir_all(&dir);
}
