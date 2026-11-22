use super::*;
use crate::model::about_form;

#[test]
fn props_dialog_roundtrip() {
    let mut f = about_form("x");
    let (d, list) = props_dialog(&f, Some(1));
    let mut data = strs(&d.get_data());
    assert_eq!(data.len(), list.len());
    // Меняем надпись кнопки (свойство Text = индекс 5).
    let ti = list.iter().position(|p| *p == Prop::Text).unwrap();
    data[ti] = Value::Str("~З~акрыть".into());
    apply_props(&mut f, Some(1), &list, &Value::List(data.clone())).unwrap();
    assert_eq!(f.controls[1].text, "~З~акрыть");
    // Ошибка — форма не меняется.
    let wi = list.iter().position(|p| *p == Prop::W).unwrap();
    data[wi] = Value::Str("широко".into());
    let before = f.clone();
    assert!(apply_props(&mut f, Some(1), &list, &Value::List(data)).is_err());
    assert_eq!(f, before);
}

#[test]
fn new_form_has_buttons() {
    let f = read_new_form(
        &Value::List(vec![Value::Str("edit_user".into()), Value::Str("Пользователь".into()), Value::Str("40".into()), Value::Str("10".into())]),
        &[],
    )
    .unwrap();
    assert_eq!(f.controls.len(), 2);
    assert!(f.check().is_empty(), "{:?}", f.check());
}

#[test]
fn new_form_as_window() {
    let v = |c: u32| {
        Value::List(vec![Value::Str("notes".into()), Value::Str("Заметки".into()), Value::Str("40".into()), Value::Str("10".into()), Value::U32(c)])
    };
    let f = read_new_form(&v(1), &[]).unwrap();
    assert_eq!(f.class, FormClass::Window);
    assert_eq!(f.controls.len(), 1);
    assert_eq!(f.controls[0].command, "cm::CLOSE");
    assert!(f.check().is_empty(), "{:?}", f.check());
    assert_eq!(read_new_form(&v(0), &[]).unwrap().class, FormClass::Dialog);
    // Класс сохраняется в .tvf и читается обратно.
    let back = Form::from_json(&f.to_json()).unwrap();
    assert_eq!(back, f);
    // Свойства формы: у окна есть номер и растяжение.
    let list = crate::props::props_for(&f, None);
    assert!(list.contains(&Prop::FormNumber) && list.contains(&Prop::FormResizable));
    let mut g = f.clone();
    crate::props::set(&mut g, None, Prop::FormClass, "диалог").unwrap();
    assert_eq!(g.class, FormClass::Dialog);
    assert!(!crate::props::props_for(&g, None).contains(&Prop::FormNumber));
    assert!(crate::props::set(&mut g, None, Prop::FormClass, "кнопка").is_err());
}

/// Сервер для Cargo.toml: по умолчанию crates-lsp; taplo (прежний
/// сервер) заменяется на crates-lsp, своё значение и «выключен» — остаются.
#[test]
fn toml_server_setting_migrates() {
    let load = |v: Option<&str>| {
        let mut pairs = vec![("theme", Json::Num(1.0))];
        if let Some(v) = v {
            pairs.push(("toml_lsp_command", Json::s(v)));
        }
        Config::from_json(&Json::obj(pairs)).toml_lsp_command
    };
    assert_eq!(load(None), "crates-lsp");
    assert_eq!(load(Some("taplo lsp stdio")), "crates-lsp", "taplo заменён");
    assert_eq!(load(Some("taplo lsp stdio --colors never")), "crates-lsp", "taplo заменён");
    assert_eq!(load(Some("/opt/bin/crates-lsp")), "/opt/bin/crates-lsp", "своё — как есть");
    assert_eq!(load(Some("")), "", "выключен");
}

/// Параметры среды: что показал диалог, то и прочиталось (в том числе
/// сервер Cargo.toml; пустое поле — выключен).
#[test]
fn options_dialog_round_trip() {
    for toml in [TOML_LSP_DEFAULT, "/opt/bin/crates-lsp", ""] {
        let cfg = Config {
            toml_lsp_command: toml.into(),
            lsp_command: "ra".into(),
            hl_col: false,
            auto_close: false,
            auto_complete: false,
            theme: 1,
            ..Config::default()
        };
        let d = options_dialog(&cfg);
        let mut back = Config::default();
        read_options(&mut back, &d.get_data());
        assert_eq!(back, cfg);
    }
}

/// «Новый проект»: тип читается первым; программе на Rust tvision-rs
/// не нужен. Импорт (без вопроса о типе) — всегда tvision-rs.
#[test]
fn build_options_dialog_roundtrip() {
    let o = BuildOptions {
        profile: "bench-fast".into(),
        features: "a, b".into(),
        all_features: false,
        no_default_features: true,
        target: "x86_64-unknown-linux-musl".into(),
        package: "app".into(),
        extra: "--locked".into(),
        run_args: "--x \"y z\"".into(),
    };
    let d = build_options_dialog(&o, &["app".into(), "lib".into()]);
    let r = read_build_options(&d.get_data()).unwrap();
    assert_eq!(r, BuildOptions { features: "a, b".into(), ..o.clone() });
    // Release и Debug — переключателем; пустое имя своего профиля — ошибка.
    let mut v = strs(&d.get_data());
    v[0] = Value::U32(1);
    assert_eq!(read_build_options(&Value::List(v.clone())).unwrap().profile, "release");
    v[0] = Value::U32(0);
    assert_eq!(read_build_options(&Value::List(v.clone())).unwrap().profile, "");
    v[0] = Value::U32(2);
    v[1] = Value::Str(String::new());
    assert!(read_build_options(&Value::List(v)).is_err());
}

#[test]
fn new_project_dialog_kinds() {
    let cfg = Config { dep_value: String::new(), ..Config::default() };
    let mut d = new_project_dialog(&cfg, "plain", "Новый проект", true);
    let mut v = strs(&d.get_data());
    v[0] = Value::U32(1);
    d.set_data(&Value::List(v));
    let np = read_new_project(&d.get_data(), true).unwrap();
    assert_eq!((np.kind, np.name.as_str(), np.dep.is_none()), (ProjectKind::Rust, "plain", true));
    // Тип по умолчанию — tvision-rs; без строки tvision-rs — ошибка.
    let d = new_project_dialog(&cfg, "app", "Новый проект", true);
    assert!(read_new_project(&d.get_data(), true).unwrap_err().contains("tvision-rs"));
    let d = new_project_dialog(&Config::default(), "app", "Импорт", false);
    let np = read_new_project(&d.get_data(), false).unwrap();
    assert_eq!(np.kind, ProjectKind::Tvision);
    assert!(np.dep.is_some());
}
