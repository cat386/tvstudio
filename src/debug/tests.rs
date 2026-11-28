use super::*;
use crate::model::Project;

#[test]
fn classifies_debuggers() {
    assert_eq!(classify("rust-lldb"), DebuggerKind::RustWrapper);
    assert_eq!(classify("rust-gdb"), DebuggerKind::RustWrapper);
    assert_eq!(classify("/opt/llvm/bin/lldb"), DebuggerKind::Lldb);
    assert_eq!(classify("gdb"), DebuggerKind::Gdb);
}

#[test]
fn target_dir_respects_profile() {
    let dir = std::env::temp_dir().join(format!("tvs_dbg_{}", std::process::id()));
    let o = BuildOptions { profile: "release".into(), ..Default::default() };
    let t = target_exe_dir(&dir, &o);
    assert!(t.ends_with("target/release") || t.to_string_lossy().ends_with("target\\release"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn binary_path_after_layout() {
    let dir = std::env::temp_dir().join(format!("tvs_dbg_bin_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tv = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tvision-rs");
    let p = Project::create(&dir, "dbg_demo", &crate::model::Dependency::Path(tv.display().to_string())).unwrap();
    let exe_dir = target_exe_dir(&p.dir, &BuildOptions::default());
    std::fs::create_dir_all(&exe_dir).unwrap();
    #[cfg(windows)]
    let exe = exe_dir.join("dbg_demo.exe");
    #[cfg(not(windows))]
    let exe = exe_dir.join("dbg_demo");
    std::fs::write(&exe, b"").unwrap();
    let found = binary_path(&p.dir, &BuildOptions::default(), None).unwrap();
    assert_eq!(found, exe);
    let _ = std::fs::remove_dir_all(&dir);
}