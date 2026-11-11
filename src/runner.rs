//! Запуск cargo в фоне: вывод построчно приходит через канал, среда
//! забирает его в простое и показывает в окне сообщений. Интерфейс при
//! этом не замирает, сборку можно остановить.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Что прислал фоновый процесс.
#[derive(Debug, Clone, PartialEq)]
pub enum JobMsg {
    Line(String),
    /// Процесс завершился: код возврата (None — убит или не запустился).
    Done(Option<i32>),
}

/// Что делать после успешной сборки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Nothing,
    Run,
}

/// Фоновая задача cargo.
pub struct Job {
    rx: Receiver<JobMsg>,
    child: Arc<Mutex<Option<Child>>>,
    /// Описание («Сборка», «Проверка»…).
    pub what: String,
    pub started: Instant,
    pub after: After,
    pub finished: bool,
}

fn pump<R: Read + Send + 'static>(r: R, tx: Sender<JobMsg>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut br = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match br.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let s = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(JobMsg::Line(s)).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

impl Job {
    /// Запустить `program args…` в каталоге `dir`.
    pub fn start(dir: &Path, program: &str, args: &[&str], what: &str, after: After) -> Result<Job, String> {
        let mut cmd = Command::new(program);
        cmd.args(args).current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // Цветной вывод в окно сообщений не нужен.
        cmd.env("CARGO_TERM_COLOR", "never");
        let mut child = cmd.spawn().map_err(|e| format!("не удалось запустить «{}»: {}", program, e))?;
        let (tx, rx) = channel();
        let out = child.stdout.take().map(|o| pump(o, tx.clone()));
        let err = child.stderr.take().map(|e| pump(e, tx.clone()));
        let child = Arc::new(Mutex::new(Some(child)));
        let ch = child.clone();
        std::thread::spawn(move || {
            // Сначала дочитываем вывод, потом ждём завершения.
            if let Some(h) = out {
                let _ = h.join();
            }
            if let Some(h) = err {
                let _ = h.join();
            }
            let code = loop {
                let mut g = ch.lock().unwrap();
                match g.as_mut() {
                    None => break None,
                    Some(c) => match c.try_wait() {
                        Ok(Some(st)) => break st.code(),
                        Ok(None) => {}
                        Err(_) => break None,
                    },
                }
                drop(g);
                std::thread::sleep(Duration::from_millis(30));
            };
            let _ = tx.send(JobMsg::Done(code));
        });
        Ok(Job { rx, child, what: what.into(), started: Instant::now(), after, finished: false })
    }

    /// Забрать накопившийся вывод.
    pub fn poll(&mut self) -> Vec<JobMsg> {
        let mut v = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            if matches!(m, JobMsg::Done(_)) {
                self.finished = true;
            }
            v.push(m);
        }
        v
    }

    /// Остановить процесс.
    pub fn kill(&self) {
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }

    /// Дождаться завершения (для тестов): весь вывод.
    pub fn wait_all(&mut self, timeout: Duration) -> Vec<JobMsg> {
        let t0 = Instant::now();
        let mut all = Vec::new();
        while !self.finished && t0.elapsed() < timeout {
            all.extend(self.poll());
            std::thread::sleep(Duration::from_millis(10));
        }
        all.extend(self.poll());
        all
    }
}

/// Запустить программу проекта в терминале (среда в это время скрыта):
/// `cargo run`, затем «нажмите Enter». Возвращает код завершения.
pub fn run_foreground(dir: &Path, cargo: &str, extra: &[&str]) -> Option<i32> {
    let mut args = vec!["run", "-q"];
    args.extend_from_slice(extra);
    let st = Command::new(cargo).args(&args).current_dir(dir).status();
    let code = st.as_ref().ok().and_then(|s| s.code());
    match &st {
        Ok(_) => println!(
            "\n— Программа завершилась (код {}). Нажмите Enter, чтобы вернуться в TV Studio…",
            code.map(|c| c.to_string()).unwrap_or("?".into())
        ),
        Err(e) => println!("\n— Не удалось запустить cargo: {}. Нажмите Enter…", e),
    }
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    code
}

#[cfg(test)]
mod tests {
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
}
