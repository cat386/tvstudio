//! Страницы диалога сохраняют данные скрытых полей и общие кнопки действий.
use tvision::*;

const PAGE_COMMAND: u16 = 1350;

pub struct PagedDialog {
    pub dialog: Dialog,
    pages: Vec<Vec<ViewId>>,
    selected: usize,
    tabs: Vec<(ViewId, String)>,
}

impl PagedDialog {
    pub fn new(mut dialog: Dialog, pages: Vec<(&str, Vec<ViewId>)>) -> Self {
        assert!(!pages.is_empty());
        let mut tabs = Vec::new();
        let mut x = 2;
        for (i, (title, _)) in pages.iter().enumerate() {
            if pages.len() == 1 {
                break;
            }
            let width = tvision::text::cstr_width(title) as i32 + 6;
            let id = dialog.add(Button::new(Rect::new(x, 1, x + width, 3), title, PAGE_COMMAND + i as u16, bf::NORMAL));
            tabs.push((id, title.to_string()));
            x += width + 1;
        }
        let mut result = Self { dialog, pages: pages.into_iter().map(|(_, ids)| ids).collect(), selected: 0, tabs };
        result.show(0);
        result
    }

    pub fn page(&self) -> usize {
        self.selected
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn show(&mut self, page: usize) {
        if page >= self.pages.len() {
            return;
        }
        self.selected = page;
        for (i, (id, title)) in self.tabs.iter().enumerate() {
            if let Some(button) = self.dialog.win.group.find_mut::<Button>(*id) {
                button.title = format!("{}{}", if i == page { "● " } else { "  " }, title);
            }
        }
        for (i, ids) in self.pages.iter().enumerate() {
            for id in ids {
                if let Some(v) = self.dialog.win.group.view_mut(*id) {
                    v.set_state(sf::VISIBLE, i == page);
                }
            }
        }
        if let Some(id) = self.pages[page].iter().copied().find(|id| self.dialog.win.group.view(*id).map(|v| v.vs().is_selectable()).unwrap_or(false))
        {
            self.dialog.select(id);
        }
    }
}

impl View for PagedDialog {
    tvision::impl_group_basics!(dialog.win.group);
    fn as_window(&self) -> Option<&Window> {
        Some(&self.dialog.win)
    }
    fn as_window_mut(&mut self) -> Option<&mut Window> {
        Some(&mut self.dialog.win)
    }
    fn draw(&self, dc: &mut DrawCtx) {
        self.dialog.draw(dc);
    }
    fn palette(&self) -> &'static [u8] {
        self.dialog.palette()
    }
    fn set_state(&mut self, flag: u16, enable: bool) {
        self.dialog.set_state(flag, enable);
    }
    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.dialog.valid(command, ctx)
    }
    fn size_limits(&self, owner: Point) -> (Point, Point) {
        self.dialog.size_limits(owner)
    }
    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        let page = match ev {
            Event::Command(m) if (PAGE_COMMAND..PAGE_COMMAND + self.pages.len() as u16).contains(&m.command) => {
                Some((m.command - PAGE_COMMAND) as usize)
            }
            Event::KeyDown(k) if k.mods == Mods::CTRL && k.key == Key::PgDn => Some((self.selected + 1) % self.pages.len()),
            Event::KeyDown(k) if k.mods == Mods::CTRL && k.key == Key::PgUp => Some((self.selected + self.pages.len() - 1) % self.pages.len()),
            _ => None,
        };
        if let Some(page) = page {
            self.show(page);
            ev.clear();
            return;
        }
        self.dialog.handle_event(ev, ctx);
    }
}

pub(super) fn ids_since(dialog: &Dialog, start: usize) -> Vec<ViewId> {
    dialog.win.group.children[start..].iter().map(|v| v.vs().id).collect()
}
