use super::*;

const SAMPLE: &str = r#"//! Пример.

use tvision::*;

/// Поздороваться.
const CM_HELLO: u16 = 100;
const CM_OTHER: u16 = 101;

fn menu() -> Menu {
    let file = Menu::new().item_key("~П~ривет", CM_HELLO, kb::F2, "F2").separator().item_key("~В~ыход", cm::QUIT, kb::ALT_X, "Alt-X");
    Menu::new().submenu("~Ф~айл", file).submenu_key("~≡~", kb::alt(' '), Menu::new().item("~О~ программе", 7)).help(3)
}

fn status() -> Vec<StatusDef> {
    vec![StatusDef::all(vec![StatusItem::new("~F2~ Привет", kb::F2, CM_HELLO), StatusItem::new("", KeyEvent::new(Key::Del, Mods::SHIFT), cm::CUT)])]
}

fn main() {
    let _ = CM_OTHER;
    let _ = (menu(), status());
}

#[cfg(test)]
mod tests {
    fn m() -> tvision::Menu { tvision::Menu::new().submenu("x", tvision::Menu::new()) }
}
"#;

#[test]
fn converts_menu_status_commands() {
    let (out, app, r) = convert_source(SAMPLE, "t");
    assert!(r.menu && r.status, "{:?}", r);
    assert_eq!(r.commands, vec!["CM_HELLO".to_string()]);
    assert!(out.contains("fn menu() -> Menu {\n    \n    ui::menu_bar()\n}") || out.contains("ui::menu_bar()"), "{}", out);
    assert!(!out.contains("let file"), "{}", out);
    assert!(out.contains("ui::status_line()"));
    assert!(!out.contains("const CM_HELLO"));
    assert!(!out.contains("Поздороваться"));
    assert!(out.contains("const CM_OTHER: u16 = 101;"), "чужая константа остаётся");
    assert!(out.contains("mod ui;\n#[allow(unused_imports)]\nuse ui::cmd::*;"));
    assert!(out.contains("tvision::Menu::new().submenu(\"x\""), "тесты не трогаем");
    assert_eq!(app.menu.len(), 2);
    let MenuNode::Sub { title, key, help, .. } = &app.menu[1] else { panic!() };
    assert_eq!((title.as_str(), key.as_str(), *help), ("~≡~", "Alt+Space", 3));
    let MenuNode::Sub { children, .. } = &app.menu[0] else { panic!() };
    assert_eq!(children[2], MenuNode::Item { title: "~В~ыход".into(), command: "cm::QUIT".into(), key: "Alt-X".into(), help: 0 });
    assert_eq!(app.status[1].key, "Shift+Del");
    assert_eq!(app.commands[0].comment, "Поздороваться.");
    assert_eq!(parse_key(&app.status[1].key), Some(KeyEvent::new(Key::Del, Mods::SHIFT)));
}

#[test]
fn relative() {
    assert_eq!(relative_path(Path::new("/a/b/c"), Path::new("/a/x")), PathBuf::from("../../x"));
}
