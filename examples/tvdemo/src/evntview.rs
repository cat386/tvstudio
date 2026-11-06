//! Окно событий (`evntview.cpp`): показывает каждое событие программы.
//! Повторная команда «Event Viewer» (Alt+0) останавливает/возобновляет вывод.

use tvision::*;

const TITLES: [&str; 2] = ["Event Viewer", "Event Viewer (Stopped)"];

pub struct EventViewer {
    pub win: Window,
    terminal: ViewId,
    pub stopped: bool,
    count: u64,
}

impl EventViewer {
    pub fn new(bounds: Rect, buf_lines: usize) -> Self {
        let mut win = Window::new(bounds, TITLES[0], 0);
        let sb = win.standard_scroll_bar(true);
        let r = win.group.vs.extent().grown(-1, -1);
        let terminal = win.add(Terminal::new(r, None, Some(sb), buf_lines));
        EventViewer { win, terminal, stopped: false, count: 0 }
    }

    /// Остановить/возобновить вывод (`toggle`).
    pub fn toggle(&mut self) {
        self.stopped = !self.stopped;
        self.win.title = TITLES[self.stopped as usize].into();
    }

    /// Текстовое описание события (`printEvent`).
    pub fn describe(ev: &Event) -> String {
        match ev {
            Event::KeyDown(k) => format!("KeyDown {{ key: {:?}, mods: {} }}", k.key, mods(k.mods)),
            Event::MouseDown(m) => format!("MouseDown {{ pos: ({}, {}), buttons: {}, double: {} }}", m.pos.x, m.pos.y, m.buttons, m.double),
            Event::MouseUp(m) => format!("MouseUp {{ pos: ({}, {}), buttons: {} }}", m.pos.x, m.pos.y, m.buttons),
            Event::MouseMove(m) => format!("MouseMove {{ pos: ({}, {}), buttons: {} }}", m.pos.x, m.pos.y, m.buttons),
            Event::MouseAuto(m) => format!("MouseAuto {{ pos: ({}, {}) }}", m.pos.x, m.pos.y),
            Event::MouseWheel(m) => format!("MouseWheel {{ pos: ({}, {}), wheel: {} }}", m.pos.x, m.pos.y, m.wheel),
            Event::Command(m) => format!("Command {{ command: {}, info: {:?} }}", m.command, m.info),
            Event::Broadcast(m) => format!("Broadcast {{ command: {}, info: {:?} }}", m.command, m.info),
            Event::Nothing => "Nothing".into(),
        }
    }
}

fn mods(m: Mods) -> String {
    let mut v = Vec::new();
    if m.ctrl {
        v.push("Ctrl");
    }
    if m.alt {
        v.push("Alt");
    }
    if m.shift {
        v.push("Shift");
    }
    if v.is_empty() {
        "-".into()
    } else {
        v.join("+")
    }
}

impl View for EventViewer {
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
    }

    /// Вызывается программой для каждого события (`Program::event_monitor`).
    fn monitor_event(&mut self, ev: &Event) {
        if self.stopped || matches!(ev, Event::MouseAuto(_)) {
            return;
        }
        self.count += 1;
        let text = format!("#{} {}", self.count, Self::describe(ev));
        let id = self.terminal;
        if let Some(t) = self.win.group.find_mut::<Terminal>(id) {
            t.write_line(&text);
        }
    }
}
