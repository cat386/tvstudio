//! Урок 6. Анкета: флажки, переключатели, список и обмен данными.
//!
//! Запуск: `cargo run --example learn_06_form`. F2 — анкета, Alt+X — выход.

use tvision::*;

// Меню, строка состояния, команды и формы — из модели проекта
// (ui.rs и forms/ генерирует TV Studio).
mod forms;
mod ui;
#[allow(unused_imports)]
use ui::cmd::*;


const DRINKS: [&str; 3] = ["Чай", "Кофе", "Сок"];
const HOBBIES: [&str; 4] = ["Книги", "Спорт", "Музыка", "Игры"];
const CITIES: [&str; 6] = ["Москва", "Тула", "Казань", "Тверь", "Сочи", "Омск"];

/// Данные анкеты — обычная структура Rust.
#[derive(Clone, Debug, Default, PartialEq)]
struct Profile {
    name: String,
    age: String,
    drink: u32,   // номер выбранного переключателя: 0 — чай, 1 — кофе, 2 — сок
    hobbies: u32, // флажки: бит 0 — книги, бит 1 — спорт, …
    city: i64,    // номер строки в списке городов
}

impl Profile {
    /// Превратить в данные для диалога. Порядок — как порядок `add`
    /// элементов, у которых есть данные (подписи и кнопки не считаются).
    fn to_value(&self) -> Value {
        Value::List(vec![
            Value::Str(self.name.clone()), // строка ввода «Имя»
            Value::Str(self.age.clone()),  // строка ввода «Возраст»
            Value::U32(self.drink),        // переключатели
            Value::U32(self.hobbies),      // флажки
            Value::Int(self.city),         // список
        ])
    }

    /// Обратно: из данных диалога.
    fn from_value(v: &Value) -> Profile {
        let l = v.as_list().unwrap_or(&[]);
        Profile {
            name: l.first().and_then(|x| x.as_str()).unwrap_or("").to_string(),
            age: l.get(1).and_then(|x| x.as_str()).unwrap_or("").to_string(),
            drink: l.get(2).and_then(|x| x.as_u32()).unwrap_or(0),
            hobbies: l.get(3).and_then(|x| x.as_u32()).unwrap_or(0),
            city: l.get(4).and_then(|x| x.as_int()).unwrap_or(0),
        }
    }

    /// Описание словами — для окна сообщения.
    fn describe(&self) -> String {
        let hobbies: Vec<&str> = (0..HOBBIES.len()).filter(|i| self.hobbies & (1 << i) != 0).map(|i| HOBBIES[i]).collect();
        format!(
            "Имя: {}\nВозраст: {}\nНапиток: {}\nУвлечения: {}\nГород: {}",
            self.name,
            self.age,
            DRINKS[self.drink as usize % DRINKS.len()],
            if hobbies.is_empty() { "нет".to_string() } else { hobbies.join(", ") },
            CITIES[self.city as usize % CITIES.len()]
        )
    }
}

/// Диалог «Анкета» нарисован в дизайнере TV Studio (форма forms/anketa.tvf,
/// код — src/forms/anketa.rs): строки ввода, переключатели, флажки, список.
fn form_dialog(p: &Profile) -> Dialog {
    let (mut d, ids) = forms::anketa::build();
    // Заполнить все элементы разом — значениями из структуры.
    d.set_data(&p.to_value());
    d.select(ids.name); // курсор — в поле «Имя»
    d
}

struct App {
    profile: Profile,
}

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, ev: &mut Event) {
        if ev.command() != Some(CM_FORM) {
            return;
        }
        ev.clear();
        let (cmd, d) = app.exec_dialog(form_dialog(&self.profile));
        if cmd == cm::OK {
            // Прочитать все элементы разом.
            self.profile = Profile::from_value(&d.get_data());
            message_box_rect(app, Rect::new(0, 0, 50, 13), &self.profile.describe(), mf::INFORMATION | mf::OK_BUTTON, true);
        }
    }
}

fn status() -> Vec<StatusDef> {
    ui::status_line()
}

fn main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let mut app = Program::with_terminal(None, Some(status()))?;
    let start = Profile { name: "Аня".into(), age: "30".into(), drink: 1, hobbies: 0b0101, city: 2 };
    app.put_event(Event::command_event(CM_FORM));
    app.run(&mut App { profile: start });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvision::platform::HeadlessBackend;

    #[test]
    fn data_roundtrip() {
        let p = Profile { name: "Боря".into(), age: "41".into(), drink: 2, hobbies: 0b1010, city: 4 };
        let d = form_dialog(&p);
        assert_eq!(Profile::from_value(&d.get_data()), p);
        assert!(p.describe().contains("Спорт, Игры"));
    }

    #[test]
    fn age_is_checked() {
        // Возраст 200: OK не закрывает диалог (сообщение об ошибке и сам
        // диалог закроет Esc тестового терминала).
        let mut b = HeadlessBackend::new(80, 25);
        b.push_key(kb::TAB); // в поле «Возраст»
        for c in "200".chars() {
            b.push_key(KeyEvent::new(Key::Char(c), Mods::NONE));
        }
        b.push_key(kb::ENTER);
        let mut app = Program::new(Box::new(b), None, None);
        let (cmd, _) = app.exec_dialog(form_dialog(&Profile::default()));
        assert_eq!(cmd, cm::CANCEL);
    }
}
