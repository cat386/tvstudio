//! Урок 1. Самая простая программа: пустой рабочий стол и строка состояния.
//!
//! Запуск: `cargo run --example learn_01_hello`. Выход — Alt+X или щелчок
//! по надписи «Alt-X Выход» внизу экрана.

// Подключаем всё нужное из библиотеки одной строкой.
use tvision::*;

// Меню, строка состояния и команды — из модели проекта (ui.rs генерирует TV Studio).
mod ui;

fn main() -> std::io::Result<()> {
    // Строка состояния — нижняя строка экрана. В ней один пункт:
    // надпись «Alt-X Выход», клавиша Alt+X и команда «выйти из программы».
    let status = ui::status_line();

    // Создаём программу: она займёт весь терминал.
    // Первый аргумент — меню (его пока нет), второй — строка состояния.
    let mut app = Program::with_terminal(None, Some(status))?;

    // Запускаем. `run` возвращается, когда пользователь выходит из программы;
    // терминал при этом возвращается в обычный вид.
    app.run(&mut program::NoHandler);
    Ok(())
}

#[cfg(test)]
mod tests {
    use tvision::platform::HeadlessBackend;
    use tvision::*;

    #[test]
    fn alt_x_quits() {
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::ALT_X);
        let status = vec![StatusDef::all(vec![StatusItem::new("~Alt-X~ Выход", kb::ALT_X, cm::QUIT)])];
        let mut app = Program::new(Box::new(b), None, Some(status));
        app.pump(&mut program::NoHandler);
        assert!(!app.is_running());
        assert!(app.screen_text().contains("Alt-X Выход"));
    }
}
