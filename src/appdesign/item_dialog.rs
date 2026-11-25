//! Вложенная правка: проверка до закрытия и поля выбранного действия.
use super::*;

type Validate = Box<dyn Fn(&Value) -> Result<(), String>>;
pub(super) type ActionFields = (ViewId, Vec<ViewId>, Vec<ViewId>);

pub(super) struct ItemDialog {
    pub dialog: Dialog,
    validate: Validate,
    error: ViewId,
    action: Option<ActionFields>,
}

impl ItemDialog {
    pub fn new(mut dialog: Dialog, validate: Validate, action: Option<ActionFields>) -> Self {
        let size = dialog.win.group.vs.size;
        // Ошибка заменяет пояснение, не закрывая форму и не меняя данные.
        let error = dialog.add(StaticText::new(Rect::new(2, size.y - 5, size.x - 2, size.y - 3), ""));
        dialog.win.group.view_mut(error).unwrap().set_state(sf::VISIBLE, false);
        let mut result = Self { dialog, validate, error, action };
        result.refresh();
        result
    }

    fn refresh(&mut self) {
        if let Some((radio, form, text)) = &self.action {
            let value = self.dialog.win.group.find::<RadioButtons>(*radio).unwrap().value;
            for (ids, visible) in [(form, value == 1), (text, value == 2)] {
                for id in ids {
                    self.dialog.win.group.view_mut(*id).unwrap().set_state(sf::VISIBLE, visible);
                }
            }
        }
    }
}

impl View for ItemDialog {
    tvision::impl_group_core!(dialog.win.group);
    fn has_data(&self) -> bool {
        true
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
    fn draw(&self, dc: &mut DrawCtx) {
        self.dialog.draw(dc);
    }
    fn palette(&self) -> &'static [u8] {
        self.dialog.palette()
    }
    fn set_state(&mut self, flag: u16, enable: bool) {
        self.dialog.set_state(flag, enable);
    }
    fn size_limits(&self, owner: Point) -> (Point, Point) {
        self.dialog.size_limits(owner)
    }
    fn handle_event(&mut self, event: &mut Event, ctx: &mut Ctx) {
        self.dialog.handle_event(event, ctx);
        self.refresh();
    }
    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        if command == cm::OK {
            if let Err(message) = (self.validate)(&self.get_data()) {
                let error = self.dialog.win.group.find_mut::<StaticText>(self.error).unwrap();
                error.text = message;
                error.set_state(sf::VISIBLE, true);
                return false;
            }
        }
        self.dialog.valid(command, ctx)
    }
}
