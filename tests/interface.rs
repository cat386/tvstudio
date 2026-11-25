//! Регрессии доступности диалогов, клавиш и первого экрана.
use tvision::platform::HeadlessBackend;
use tvision::*;
use tvstudio::cmds::*;
use tvstudio::dialogs::{self, Config};
use tvstudio::ide::{menu_bar, status_line, Ide};
use tvstudio::model::{Form, Kind};
use tvstudio::ui_commands;

fn app(width: i32, height: i32) -> Program {
    let mut backend = HeadlessBackend::new(width, height);
    backend.esc_budget = 0;
    Program::new(Box::new(backend), Some(menu_bar()), Some(status_line()))
}

fn render<V: View>(view: V, width: i32, height: i32) -> String {
    let mut app = app(width, height);
    dialogs::insert(&mut app, view);
    app.redraw();
    app.screen_text()
}

#[test]
fn properties_of_every_component_fit_and_roundtrip_on_every_page() {
    for kind in Kind::ALL {
        let mut form = Form::new("audit", "Проверка");
        form.controls.push(form.new_control(kind, 2, 2));
        let (dialog, props) = dialogs::props_dialog(&form, Some(0));
        let mut restored = form.clone();
        dialogs::apply_props(&mut restored, Some(0), &props, &dialog.get_data()).unwrap();
        assert_eq!(restored, form, "{kind:?}");
        for (width, height) in [(80, 25), (100, 30), (120, 40)] {
            for page in 0..dialog.page_count() {
                let (mut d, _) = dialogs::props_dialog(&form, Some(0));
                d.show(page);
                let screen = render(d, width, height);
                assert!(screen.contains("Свойства:"), "{kind:?}: {screen}");
                assert!(screen.contains("Сохранить") && screen.contains("Отмена"), "{kind:?}, page {page}: {screen}");
            }
        }
    }
}

#[test]
fn settings_sections_fit_and_preserve_hidden_values() {
    let cfg =
        Config { theme: 2, cargo: "custom-cargo".into(), lsp_command: "custom-ra".into(), auto_close: false, autosave: false, ..Config::default() };
    for (width, height) in [(80, 25), (100, 30), (120, 40)] {
        for page in 0..4 {
            let mut d = dialogs::options_dialog(&cfg);
            d.show(page);
            let mut back = Config::default();
            dialogs::read_options(&mut back, &d.get_data());
            assert_eq!(back, cfg);
            let screen = render(d, width, height);
            assert!(screen.contains("Параметры среды"), "{screen}");
            assert!(screen.contains("Сохранить") && screen.contains("Отмена"), "page {page}: {screen}");
        }
    }
}

#[test]
fn pages_switch_by_keyboard_without_losing_edits_and_cancel_is_safe() {
    let mut app = app(80, 25);
    let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
    let (mut d, list) = {
        let mut f = Form::new("audit", "Проверка");
        f.controls.push(f.new_control(Kind::Table, 2, 2));
        dialogs::props_dialog(&f, Some(0))
    };
    let mut values = match d.get_data() {
        Value::List(values) => values,
        _ => panic!("список свойств"),
    };
    let help = list.iter().position(|p| *p == tvstudio::props::Prop::HelpCtx).unwrap();
    values[help] = Value::Str("123".into());
    d.set_data(&Value::List(values.clone()));
    let id = dialogs::insert(&mut app, d);
    for key in [KeyEvent::new(Key::PgDn, Mods::CTRL), KeyEvent::new(Key::PgUp, Mods::CTRL)] {
        app.put_event(Event::KeyDown(key));
        app.pump(&mut ide);
    }
    let d = app.find::<dialogs::PagedDialog>(id).unwrap();
    assert_eq!(d.page(), 0);
    assert_eq!(d.get_data(), Value::List(values));

    // Отмена в другом разделе возвращает управление, не применяя настройки.
    app.put_event(Event::KeyDown(KeyEvent::new(Key::PgDn, Mods::CTRL)));
    app.put_event(Event::KeyDown(kb::key(Key::Esc)));
    let cfg = Config::default();
    let (result, _) = dialogs::exec(&mut app, dialogs::options_dialog(&cfg));
    assert_eq!(result, cm::CANCEL);
    assert_eq!(cfg, Config::default());
}

#[test]
fn plain_rust_project_hides_dependency_and_switching_back_restores_it() {
    let cfg = Config::default();
    let mut d = dialogs::new_project_dialog(&cfg, "audit", "Новый проект", true);
    let mut values = match d.get_data() {
        Value::List(values) => values,
        _ => panic!("данные проекта"),
    };
    values[0] = Value::U32(1);
    d.set_data(&Value::List(values.clone()));
    assert!(dialogs::read_new_project(&d.get_data(), true).unwrap().dep.is_none());
    let screen = render(d, 80, 25);
    assert!(!screen.contains(&cfg.dep_value), "{screen}");
    let mut d = dialogs::new_project_dialog(&cfg, "audit", "Новый проект", true);
    values[0] = Value::U32(0);
    d.set_data(&Value::List(values));
    let screen = render(d, 80, 25);
    assert!(screen.contains(&cfg.dep_value), "{screen}");
}

#[test]
fn start_page_and_entire_menu_fit_in_80_columns() {
    let mut app = app(80, 25);
    let mut ide = Ide::new(Config { lsp_enabled: false, ..Config::default() }, true);
    ide.startup(&mut app, vec![]);
    app.redraw();
    let screen = app.screen_text();
    let menu = screen.lines().next().unwrap();
    for title in ["Файл", "Правка", "Код", "Форма", "Проект", "Сборка", "Окно", "Справка"] {
        assert!(menu.contains(title), "{menu}");
    }
    for title in ["Новый проект", "Открыть проект", "Клонировать", "Недавние проекты"] {
        assert!(screen.contains(title), "{screen}");
    }
    assert!(ide.inspector.is_none() && ide.messages.is_none());
    assert!(!app.command_enabled(CM_SHOW_INSPECTOR));
    // Открытие нового файла убирает первый экран и возвращает инструменты.
    app.put_event(Event::command_event(cm::NEW));
    app.pump(&mut ide);
    assert_eq!(Ide::windows::<tvstudio::codeedit::CodeWindow>(&app).len(), 1);
    assert!(!app.screen_text().contains("Недавние проекты"));
}

#[test]
fn global_shortcuts_are_unambiguous_and_help_uses_the_same_registry() {
    let specs = ui_commands::commands();
    for (i, a) in specs.iter().enumerate() {
        for b in &specs[i + 1..] {
            if a.key == b.key {
                assert!(!a.context.is_empty() && !b.context.is_empty() && a.context != b.context, "{} / {}", a.title, b.title);
            }
        }
    }
    assert_eq!(ui_commands::spec(cm::REPLACE).key, kb::ctrl('H'));
    assert_eq!(ui_commands::spec(cm::CLOSE).key, kb::ALT_F3);
    assert_eq!(ui_commands::spec(CM_FORM_TEST).key, kb::shift_f(9));
    assert_eq!(ui_commands::spec(CM_DS_DUP).key, kb::ctrl('D'));
    let help = ui_commands::help_keys();
    assert!(help.contains("Ctrl+H") && help.contains("Shift+F9"));
    assert!(!help.contains('~'));
    HelpFile::parse(&tvstudio::ide::HELP_SOURCE.replace("{{COMMAND_KEYS}}", &help)).unwrap();
}

#[test]
fn menu_mnemonics_do_not_hide_other_items() {
    fn check(menu: &Menu) {
        let mut used = std::collections::HashMap::new();
        for item in &menu.items {
            if let Some(letter) = tvision::text::hotkey(item.name()) {
                let letter = letter.to_lowercase().collect::<String>();
                assert!(used.insert(letter, item.name()).is_none(), "повтор горячей буквы: {}", item.name());
            }
            if let MenuItem::SubMenu { menu, .. } = item {
                check(menu);
            }
        }
    }
    check(&menu_bar());
}

#[test]
fn property_sections_are_named_and_active_section_survives_field_focus() {
    let mut form = Form::new("audit", "Test");
    form.controls.push(form.new_control(Kind::Table, 2, 2));
    let (mut dialog, _) = dialogs::props_dialog(&form, Some(0));
    assert_eq!(dialog.page_count(), 3);
    for (page, title) in ["Основное", "Данные", "Поведение"].iter().enumerate() {
        dialog.show(page);
        let tabs: Vec<_> = dialog
            .dialog
            .win
            .group
            .children
            .iter()
            .filter_map(|v| v.as_any().downcast_ref::<Button>())
            .filter(|b| b.title.starts_with("● "))
            .collect();
        assert_eq!(tabs.len(), 1);
        assert!(tabs[0].title.contains(title));
        assert_ne!(dialog.dialog.win.group.current_view().unwrap().vs().id, tabs[0].vs.id);
    }
    let mut form = Form::new("audit", "Test");
    form.controls.push(form.new_control(Kind::Line, 2, 2));
    let (dialog, _) = dialogs::props_dialog(&form, Some(0));
    assert_eq!(dialog.page_count(), 1);
    let screen = render(dialog, 80, 25);
    assert!(!screen.contains("PgUp") && !screen.contains("пункту в строке"), "{screen}");
}

#[test]
fn settings_page_buttons_and_dialog_help_fit_in_80_columns() {
    for page in 0..4 {
        let mut dialog = dialogs::options_dialog(&Config::default());
        dialog.show(page);
        let screen = render(dialog, 80, 25);
        for text in ["Общие", "Инструменты", "Серверы", "Редактор", "Сохранить", "Отмена"] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
        assert!(!screen.contains("F9 Собрать"), "{screen}");
    }
}
