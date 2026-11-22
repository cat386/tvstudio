use super::*;

/// stdout и stderr читаются построчно (UTF-8, как пишут cargo и rustc),
/// «\r\n» на конце строки отбрасывается, код завершения доходит.
/// На Windows — латиница: встроенный `echo` у cmd пишет в канал в
/// кодировке консоли (CP866), а не в UTF-8, — это особенность cmd, не среды.
#[test]
fn captures_output() {
    let (prog, args, out, err): (&str, Vec<&str>, &str, &str) = if cfg!(windows) {
        ("cmd", vec!["/C", "echo hello&& echo oops 1>&2&& exit 3"], "hello", "oops")
    } else {
        ("sh", vec!["-c", "echo привет; echo ошибка >&2; exit 3"], "привет", "ошибка")
    };
    let mut j = Job::start(Path::new("."), prog, &args, "тест", After::Nothing).unwrap();
    let all = j.wait_all(Duration::from_secs(10));
    assert!(all.contains(&JobMsg::Line(out.into())), "{:?}", all);
    assert!(all.iter().any(|m| matches!(m, JobMsg::Line(s) if s.trim_end() == err)), "{:?}", all);
    assert_eq!(all.last(), Some(&JobMsg::Done(Some(3))), "{:?}", all);
}
