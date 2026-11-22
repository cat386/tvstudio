use super::*;

#[test]
fn silent_git_process_returns_timeout() {
    let mut process = Command::new(std::env::current_exe().unwrap());
    process.args(["--exact", "git::timeout_tests::silent_child", "--nocapture"]).env("TVSTUDIO_TEST_SILENT_GIT", "1");
    let start = Instant::now();
    let result = run_with_idle_timeout(process, Duration::from_secs(1));
    assert!(result.unwrap_err().contains("Git не прислал данных за 1 сек."));
    assert!(start.elapsed() < Duration::from_secs(8));
}

#[test]
fn silent_child() {
    if std::env::var_os("TVSTUDIO_TEST_SILENT_GIT").is_some() {
        thread::sleep(Duration::from_secs(30));
    }
}
