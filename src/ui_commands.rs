//! Общие подписи и сочетания клавиш меню, контекста и справки.
use crate::cmds::*;
use tvision::*;

pub struct CommandSpec {
    pub id: u16,
    pub title: &'static str,
    pub key: KeyEvent,
    pub shortcut: &'static str,
    /// F12 имеет намеренно разные действия в дизайнере и редакторе.
    pub context: &'static str,
}

pub fn commands() -> Vec<CommandSpec> {
    let ctrl = |c| kb::ctrl(c);
    let mut result = Vec::new();
    let mut add = |id, title, key, shortcut, context| result.push(CommandSpec { id, title, key, shortcut, context });
    add(cm::OPEN, "Открыть фа~й~л...", ctrl('O'), "Ctrl+O", "");
    add(cm::SAVE, "~С~охранить", kb::F2, "F2", "");
    add(cm::SAVE_ALL, "Сохранить ~в~сё", kb::shift_f(2), "Shift+F2", "");
    add(cm::UNDO, "~О~тменить", ctrl('Z'), "Ctrl+Z", "");
    add(cm::REDO, "~П~овторить", KeyEvent::new(Key::Char('z'), Mods { ctrl: true, shift: true, alt: false }), "Ctrl+Shift+Z", "");
    add(cm::CUT, "~В~ырезать", ctrl('X'), "Ctrl+X", "");
    add(cm::COPY, "~К~опировать", ctrl('C'), "Ctrl+C", "");
    add(cm::PASTE, "Вст~а~вить", ctrl('V'), "Ctrl+V", "");
    add(cm::CLEAR, "~У~далить выделенное", kb::key(Key::Del), "Del", "");
    add(cm::SELECT_ALL, "Выделить вс~ё~", ctrl('A'), "Ctrl+A", "");
    add(cm::FIND, "~Н~айти...", kb::f(3), "F3", "");
    add(cm::REPLACE, "~З~аменить...", ctrl('H'), "Ctrl+H", "");
    add(cm::SEARCH_AGAIN, "Найти ~с~ледующее", ctrl('L'), "Ctrl+L", "");
    add(cm::CLOSE, "~З~акрыть окно", kb::ALT_F3, "Alt+F3", "");
    add(CM_DS_DUP, "Ду~б~лировать", ctrl('D'), "Ctrl+D", "дизайнер");
    add(CM_DS_PROPS, "~С~войства...", kb::ctrl_f(2), "Ctrl+F2", "");
    add(CM_FORM_TEST, "~П~редпросмотр формы", kb::shift_f(9), "Shift+F9", "");
    add(CM_FORM_CODE, "Код ~ф~ормы", kb::f(12), "F12", "дизайнер");
    add(CM_GOTO_DEF, "К ~о~пределению", kb::f(12), "F12", "редактор");
    add(CM_FIND_REFS, "Найти сс~ы~лки", kb::shift_f(12), "Shift+F12", "");
    add(CM_COMPLETE, "~Д~ополнить", KeyEvent::new(Key::Char(' '), Mods::CTRL), "Ctrl+Space", "");
    add(CM_NAV_BACK, "Н~а~зад", KeyEvent::new(Key::Left, Mods::ALT), "Alt+←", "");
    add(CM_CODE_ACTIONS, "~Б~ыстрые исправления", KeyEvent::new(Key::Enter, Mods::ALT), "Alt+Enter", "");
    add(CM_GOTO, "~П~ерейти к строке...", kb::alt('G'), "Alt+G", "");
    add(CM_CHECK, "~П~роверить код", kb::alt_f(9), "Alt+F9", "");
    add(CM_BUILD, "~С~обрать", kb::f(9), "F9", "");
    add(CM_RUN, "~З~апустить приложение", kb::ctrl_f(9), "Ctrl+F9", "");
    add(CM_DEBUG, "~О~тладка", kb::shift_f(5), "Shift+F5", "");
    add(CM_NEXT_ERR, "Следующая о~ш~ибка", kb::f(8), "F8", "");
    add(CM_PREV_ERR, "Предыдущая ош~и~бка", kb::shift_f(8), "Shift+F8", "");
    add(CM_SHOW_PROBLEMS, "О~ш~ибки и предупреждения", kb::ctrl_f(8), "Ctrl+F8", "");
    add(CM_SHOW_INSPECTOR, "~И~нспектор", kb::f(11), "F11", "");
    result
}

pub fn spec(id: u16) -> CommandSpec {
    commands().into_iter().find(|c| c.id == id).expect("команда объявлена в ui_commands")
}

pub trait MenuCommands {
    fn command(self, id: u16) -> Self;
}

impl MenuCommands for Menu {
    fn command(self, id: u16) -> Self {
        let c = spec(id);
        self.item_key(c.title, id, c.key, c.shortcut)
    }
}

pub fn help_keys() -> String {
    commands()
        .iter()
        .map(|c| {
            format!(
                "  {:<13} {}{}\n",
                c.shortcut,
                c.title.replace('~', ""),
                if c.context.is_empty() { String::new() } else { format!(" ({})", c.context) }
            )
        })
        .collect()
}

pub fn status(id: u16) -> StatusItem {
    let command = spec(id);
    StatusItem::new(&format!("~{}~ {}", command.shortcut, command.title.replace('~', "")), command.key, id)
}
