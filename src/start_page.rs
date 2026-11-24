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
        win.add(StaticText::new(Rect::new(3, 8, 73, 9), "Недавние проекты — Enter или двойной щелчок, чтобы открыть"));
        let scroll = win.add(ScrollBar::new(Rect::new(72, 9, 73, 16)));
        let mut list = ListBox::new(Rect::new(3, 9, 72, 16), 1, Some(scroll));
        list.set_items(if recent.is_empty() { vec!["Пока нет проектов".into()] } else { recent.to_vec() });
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
        self.win.handle_window_event(ev, ctx);
        if ev.command() == Some(SELECT_RECENT) {
            if let Some(path) = self.win.group.find::<ListBox>(self.list).and_then(|list| self.recent.get(list.focused)) {
                ctx.post_command(OPEN_RECENT, Info::Str(path.clone()));
            }
            ev.clear();
        }
    }
}
