//! Первый экран с явными действиями вместо пустых окон инструментов.
use crate::cmds::*;
use crate::toolwins::Relay;
use tvision::*;

pub(crate) const OPEN_RECENT: u16 = 1361;
const SELECT_RECENT: u16 = 1360;

pub(crate) struct StartPage {
    win: Window,
    recent: Vec<String>,
    list: ViewId,
}

impl StartPage {
    pub fn new(recent: &[String]) -> Self {
        let mut win = Window::new(Rect::new(0, 0, 76, 21), "TV Studio", 0);
        win.group.vs.options |= of::CENTERED;
        win.flags = 0;
        win.palette = color::palettes::GRAY_DIALOG;
        win.add(StaticText::new(Rect::new(3, 2, 73, 3), "Создайте приложение с формами или откройте исходники Rust."));
        win.add(Button::new(Rect::new(3, 4, 24, 6), "~Н~овый проект", CM_NEW_PROJECT, bf::DEFAULT));
        win.add(Button::new(Rect::new(26, 4, 47, 6), "~О~ткрыть проект", CM_OPEN_PROJECT, bf::NORMAL));
        win.add(Button::new(Rect::new(49, 4, 72, 6), "~К~лонировать", CM_GIT_CLONE, bf::NORMAL));
        win.add(StaticText::new(Rect::new(3, 8, 73, 9), "Недавние проекты — Enter: открыть; F4: полный путь"));
        let scroll = win.add(ScrollBar::new(Rect::new(72, 9, 73, 16)));
        let mut list = ListBox::new(Rect::new(3, 9, 72, 16), 1, Some(scroll));
        list.set_items(if recent.is_empty() {
            vec!["Пока нет проектов".into()]
        } else {
            recent.iter().map(|path| recent_label(path, 66)).collect()
        });
        let list = win.add(list);
        win.add(Relay::new(list, SELECT_RECENT));
        win.add(StaticText::new(Rect::new(3, 18, 73, 19), "F1 — справка  •  F10 — меню  •  Ctrl+O — открыть файл"));
        Self { win, recent: recent.to_vec(), list }
    }
}

impl View for StartPage {
    tvision::impl_group_basics!(win.group);
    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }
    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
    }
    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }
    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }
    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        if matches!(ev, Event::KeyDown(k) if *k == kb::f(4)) {
            if let Some(path) = self.win.group.find::<ListBox>(self.list).and_then(|list| self.recent.get(list.focused)) {
                let mut d = Dialog::centered(70, 11, "Путь к проекту");
                d.win.group.vs.help_ctx = HC_DIALOG;
                let scroll = d.win.standard_scroll_bar(false);
                let mut text = Terminal::new(Rect::new(2, 2, 68, 7), Some(scroll), None, 2);
                text.write_line(&display_path(path));
                text.scroller.delta = Point::new(0, 0);
                let text = d.add(text);
                d.add(Button::new(Rect::new(28, 8, 42, 10), "Закрыть", cm::OK, bf::DEFAULT));
                d.select(text);
                ctx.exec(Box::new(crate::dialogs::flat(d)), None, 0);
            }
            ev.clear();
            return;
        }
        self.win.handle_window_event(ev, ctx);
        if ev.command() == Some(SELECT_RECENT) {
            if let Some(path) = self.win.group.find::<ListBox>(self.list).and_then(|list| self.recent.get(list.focused)) {
                ctx.post_command(OPEN_RECENT, Info::Str(path.clone()));
            }
            ev.clear();
        }
    }
}

/// Имя остаётся видимым, даже если каталоги занимают всю строку.
pub(crate) fn recent_label(path: &str, width: usize) -> String {
    let shown = display_path(path);
    let path = shown.as_str();
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let name = file.strip_suffix(".tvsp").unwrap_or(file);
    let name = middle(name, width / 2);
    let remaining = width.saturating_sub(tvision::text::str_width(&name) + 3);
    format!("{name} — {}", middle(path, remaining))
}

fn display_path(path: &str) -> String {
    path.chars().flat_map(|ch| if ch.is_control() { ch.escape_default().collect::<Vec<_>>() } else { vec![ch] }).collect()
}

fn middle(text: &str, width: usize) -> String {
    if tvision::text::str_width(text) <= width {
        return text.into();
    }
    if width == 0 {
        return String::new();
    }
    let left = (width - 1) / 2;
    let right = width - 1 - left;
    let mut prefix = String::new();
    for ch in text.chars() {
        if tvision::text::str_width(&prefix) + tvision::text::str_width(&ch.to_string()) > left {
            break;
        }
        prefix.push(ch);
    }
    let mut suffix = String::new();
    for ch in text.chars().rev() {
        if tvision::text::str_width(&suffix) + tvision::text::str_width(&ch.to_string()) > right {
            break;
        }
        suffix.insert(0, ch);
    }
    format!("{prefix}…{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_control_characters_remain_visible_on_one_line() {
        let shown = display_path("/tmp/a\nb\tc\r/project.tvsp");
        assert_eq!(shown, "/tmp/a\\nb\\tc\\r/project.tvsp");
        assert!(!shown.chars().any(char::is_control));
    }
    #[test]
    fn recent_paths_keep_name_and_fit_unicode_cells() {
        for path in [
            "D:/long-long-directory/another-long-directory/project/project.tvsp",
            "D:\\Разработка\\каталог\\Приложение.tvsp",
            "D:/資料/長い名前/プロジェクト.tvsp",
        ] {
            for width in [30, 66] {
                let label = recent_label(path, width);
                assert!(tvision::text::str_width(&label) <= width, "{label}");
                assert!(label.contains(" — "));
            }
        }
        assert!(recent_label("D:/very-long-directory/project.tvsp", 30).starts_with("project — "));
    }
}
