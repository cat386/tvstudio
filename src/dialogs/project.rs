use tvision::*;

pub struct ProjectDialog {
    pub dialog: Dialog,
    kind: Option<ViewId>,
    dependency: Vec<ViewId>,
}

impl ProjectDialog {
    pub(super) fn new(mut dialog: Dialog, kind: Option<ViewId>, dependency: Vec<ViewId>) -> Self {
        dialog.win.group.vs.help_ctx = crate::cmds::HC_DIALOG;
        let mut result = Self { dialog, kind, dependency };
        result.refresh();
        result
    }
    fn refresh(&mut self) {
        let visible = self.kind.and_then(|id| self.dialog.win.group.find::<RadioButtons>(id)).map(|kind| kind.value == 0).unwrap_or(true);
        for id in &self.dependency {
            if let Some(view) = self.dialog.win.group.view_mut(*id) {
                view.set_state(sf::VISIBLE, visible);
            }
        }
    }
}

impl View for ProjectDialog {
    tvision::impl_group_core!(dialog.win.group);
    fn has_data(&self) -> bool {
        self.dialog.has_data()
    }
    fn get_data(&self) -> Value {
        self.dialog.get_data()
    }
    fn set_data(&mut self, value: &Value) {
        self.dialog.set_data(value);
        self.refresh();
    }
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
        self.dialog.handle_event(ev, ctx);
        self.refresh();
    }
}
