//! Импорт программ на tvision-rs в проекты TV Studio.
//!
//! ```text
//! tvs-import ПРОГРАММА.rs КАТАЛОГ ИМЯ [tvision-rs]
//!     — одна программа → проект КАТАЛОГ/ИМЯ; tvision-rs — ссылка git
//!       или путь к библиотеке (по умолчанию git cat386/tvision-rs)
//! tvs-import --examples ПУТЬ_К_TVISION_RS КАТАЛОГ
//!     — все примеры tvision-rs, каждый в КАТАЛОГ/<имя примера>
//! tvs-import --forms ПРОЕКТ...
//!     — найти диалоги в коде проектов и добавить их формами (forms/*.tvf)
//! ```

use std::path::{Path, PathBuf};

use tvstudio::importer::{import_program, relative_path};
use tvstudio::model::Dependency;

fn abs(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| tvision::widgets::editor::absolute(p))
}

/// Зависимость для проекта `parent/name` на библиотеку в `lib`: относительный путь.
fn path_dep(parent: &Path, name: &str, lib: &Path) -> Dependency {
    let rel = relative_path(&abs(parent).join(name), &abs(lib));
    Dependency::Path(rel.display().to_string().replace('\\', "/"))
}

fn import_one(main: &Path, parent: &Path, name: &str, dep: &Dependency) -> bool {
    match import_program(main, parent, name, dep) {
        Ok((p, r)) => {
            println!("✓ {:<16} → {}\n    {}", name, p.dir.display(), r.summary().replace('\n', "\n    "));
            true
        }
        Err(e) => {
            eprintln!("✗ {}: {}", name, e);
            false
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|a| a == "--examples").unwrap_or(false) && args.len() == 3 {
        let lib = PathBuf::from(&args[1]);
        let out = PathBuf::from(&args[2]);
        let _ = std::fs::create_dir_all(&out);
        let ex = lib.join("examples");
        let mut list: Vec<(String, PathBuf)> = Vec::new();
        for e in std::fs::read_dir(&ex).expect("каталог examples").flatten() {
            let p = e.path();
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if p.extension().map(|x| x == "rs").unwrap_or(false) {
                list.push((stem, p));
            } else if p.is_dir() && p.join("main.rs").exists() {
                list.push((stem, p.join("main.rs")));
            }
        }
        list.sort();
        let mut ok = 0;
        for (name, main) in &list {
            if import_one(main, &out, name, &path_dep(&out, name, &lib)) {
                ok += 1;
            }
        }
        println!("Импортировано {} из {}", ok, list.len());
        std::process::exit(if ok == list.len() { 0 } else { 1 });
    }
    if args.first().map(|a| a == "--forms").unwrap_or(false) && args.len() > 1 {
        let mut ok = true;
        for dir in &args[1..] {
            let dir = PathBuf::from(dir);
            let file = std::fs::read_dir(&dir)
                .ok()
                .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.extension().map(|x| x == "tvsp").unwrap_or(false)));
            let Some(file) = file else {
                eprintln!("✗ {}: нет файла проекта .tvsp", dir.display());
                ok = false;
                continue;
            };
            match tvstudio::model::Project::load(&file).and_then(|mut p| tvstudio::formextract::extract_into_project(&mut p).map(|r| (p, r))) {
                Ok((p, r)) => println!("✓ {:<16} {}", p.name, r.summary().replace('\n', "\n  ")),
                Err(e) => {
                    eprintln!("✗ {}: {}", dir.display(), e);
                    ok = false;
                }
            }
        }
        std::process::exit(if ok { 0 } else { 1 });
    }
    if args.len() < 3 {
        eprintln!("tvs-import ПРОГРАММА.rs КАТАЛОГ ИМЯ [tvision-rs]\ntvs-import --examples ПУТЬ_К_TVISION_RS КАТАЛОГ\ntvs-import --forms ПРОЕКТ...");
        std::process::exit(2);
    }
    let main = PathBuf::from(&args[0]);
    let parent = PathBuf::from(&args[1]);
    let name = &args[2];
    let dep = match args.get(3) {
        Some(s) => match Dependency::detect(s) {
            Some(Dependency::Path(p)) => path_dep(&parent, name, Path::new(&p)),
            Some(d) => d,
            None => Dependency::Git("https://github.com/cat386/tvision-rs".into()),
        },
        None => Dependency::Git("https://github.com/cat386/tvision-rs".into()),
    };
    std::process::exit(if import_one(&main, &parent, name, &dep) { 0 } else { 1 });
}
