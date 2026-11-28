//! Запуск отладчика (rust-lldb, rust-gdb, lldb, gdb) после сборки.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::model::{cargo_layout, cargo_package_name, BuildOptions, CargoLayout};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebuggerKind {
    RustWrapper,
    Lldb,
    Gdb,
}

/// Какой отладчик выбран и как он запускается.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Debugger {
    pub program: String,
    pub kind: DebuggerKind,
}

fn works(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn stem(cmd: &str) -> String {
    Path::new(cmd).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn classify(program: &str) -> DebuggerKind {
    let s = stem(program);
    if s.contains("gdb") {
        if s.contains("rust") {
            DebuggerKind::RustWrapper
        } else {
            DebuggerKind::Gdb
        }
    } else if s.contains("rust") {
        DebuggerKind::RustWrapper
    } else {
        DebuggerKind::Lldb
    }
}

const AUTO: &[&str] = &["rust-lldb", "rust-gdb", "lldb", "gdb"];

/// Подобрать отладчик: `override_cmd` из настроек или первый из PATH.
pub fn resolve(override_cmd: &str) -> Option<Debugger> {
    if !override_cmd.trim().is_empty() {
        let p = override_cmd.trim();
        return works(p).then(|| Debugger { program: p.to_string(), kind: classify(p) });
    }
    AUTO.iter().find(|c| works(c)).map(|&p| Debugger { program: p.to_string(), kind: classify(p) })
}

fn profile_dir(profile: &str) -> &str {
    match profile.trim() {
        "" | "dev" | "debug" => "debug",
        "release" => "release",
        p => p,
    }
}

/// Каталог `target/…/profile` с учётом `--target`.
pub fn target_exe_dir(workspace: &Path, opts: &BuildOptions) -> PathBuf {
    let mut t = workspace.join("target");
    if !opts.target.trim().is_empty() {
        t.push(opts.target.trim());
    }
    t.push(profile_dir(&opts.profile));
    t
}

fn package_for_run(workspace: &Path, opts: &BuildOptions, layout: &CargoLayout, file: Option<&Path>) -> String {
    if !opts.package.trim().is_empty() {
        return opts.package.trim().to_string();
    }
    if let Some(name) = layout.run_package(file) {
        return name;
    }
    std::fs::read_to_string(workspace.join("Cargo.toml"))
        .ok()
        .and_then(|t| cargo_package_name(&t))
        .unwrap_or_else(|| workspace.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "app".into()))
}

fn exe_name(crate_name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{}.exe", crate_name)
    }
    #[cfg(not(windows))]
    {
        crate_name.to_string()
    }
}

/// Путь к исполняемому файлу после `cargo build` (эвристика по имени пакета).
pub fn binary_path(workspace: &Path, opts: &BuildOptions, file: Option<&Path>) -> Option<PathBuf> {
    let layout = cargo_layout(workspace);
    let name = package_for_run(workspace, opts, &layout, file);
    let dir = target_exe_dir(workspace, opts);
    let direct = dir.join(exe_name(&name));
    if direct.is_file() {
        return Some(direct);
    }
    let underscored = name.replace('-', "_");
    if underscored != name {
        let alt = dir.join(exe_name(&underscored));
        if alt.is_file() {
            return Some(alt);
        }
    }
    None
}

/// Аргументы для `cargo run` (без слова `run`): профиль, пакет, features…
pub fn cargo_run_prefix(opts: &BuildOptions) -> Vec<String> {
    let mut v = opts.cargo_args();
    v.push("-q".into());
    v
}

fn run_lldb(program: &str, binary: &Path, prog_args: &[String]) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = Command::new(program);
    cmd.arg("-o").arg("run");
    cmd.arg("--");
    cmd.arg(binary);
    cmd.args(prog_args);
    cmd.status()
}

fn run_gdb(program: &str, binary: &Path, prog_args: &[String]) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = Command::new(program);
    cmd.arg("-q").arg(binary);
    cmd.arg("-ex").arg("run");
    if !prog_args.is_empty() {
        cmd.arg("-ex").arg(format!("set args {}", shell_join(prog_args)));
    }
    cmd.status()
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.chars().any(|c| c.is_whitespace() || c == '"' || c == '\\') {
                format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pause_return(code: Option<i32>) -> Option<i32> {
    println!(
        "\n— Сеанс отладки завершён (код {}). Нажмите Enter, чтобы вернуться в TV Studio…",
        code.map(|c| c.to_string()).unwrap_or("?".into())
    );
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    code
}

/// Запуск отладчика в терминале (среда скрыта, как у `cargo run`).
pub fn debug_foreground(
    workspace: &Path,
    dbg: &Debugger,
    cargo: &str,
    opts: &BuildOptions,
    prog_args: &[String],
    file: Option<&Path>,
) -> Result<Option<i32>, String> {
    let st = match dbg.kind {
        DebuggerKind::RustWrapper => {
            let prefix = cargo_run_prefix(opts);
            let mut cmd = Command::new(&dbg.program);
            cmd.arg("--").arg(cargo).arg("run");
            cmd.args(&prefix);
            if !prog_args.is_empty() {
                cmd.arg("--");
                cmd.args(prog_args);
            }
            cmd.current_dir(workspace)
                .status()
                .map_err(|e| format!("не удалось запустить {}: {}", dbg.program, e))?
        }
        DebuggerKind::Lldb | DebuggerKind::Gdb => {
            let bin = binary_path(workspace, opts, file).ok_or_else(|| {
                format!(
                    "не найден исполняемый файл в {} — сначала соберите проект (F9)",
                    target_exe_dir(workspace, opts).display()
                )
            })?;
            if dbg.kind == DebuggerKind::Lldb {
                run_lldb(&dbg.program, &bin, prog_args)
            } else {
                run_gdb(&dbg.program, &bin, prog_args)
            }
            .map_err(|e| format!("не удалось запустить {}: {}", dbg.program, e))?
        }
    };
    Ok(pause_return(st.code()))
}

#[cfg(test)]
mod tests;