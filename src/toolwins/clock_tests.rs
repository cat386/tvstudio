use super::*;
use tvision::platform::{HeadlessBackend, RawEvent};
use tvision::program::NoHandler;

fn clock_at_right(program: &Program) -> bool {
    let text = program.screen_text();
    let row: Vec<_> = text.lines().next().unwrap().chars().collect();
    let end = &row[row.len() - 9..];
    end[2] == ':' && end[5] == ':' && end[8] == ' ' && [0, 1, 3, 4, 6, 7].into_iter().all(|i| end[i].is_ascii_digit())
}

#[test]
fn menu_clock_follows_resize_without_covering_items() {
    let menu = crate::ide::menu_bar();
    let (x, length) = *menu.bar_positions().last().unwrap();
    let mut backend = HeadlessBackend::new(110, 25);
    backend.input.push_back(RawEvent::Resize(Point::new(x + length + MenuClock::CLOCK_WIDTH + 2, 25)));
    backend.input.push_back(RawEvent::Resize(Point::new(x + length + 4, 25)));
    backend.input.push_back(RawEvent::Resize(Point::new(125, 25)));
    let mut app = Program::new(Box::new(backend), Some(menu), None);
    MenuClock::install(&mut app);
    let clock = app.group.children.iter_mut().find_map(|view| view.as_any_mut().downcast_mut::<MenuClock>()).unwrap();
    clock.keyboard.get_mut().set_test_label("РУС");
    app.redraw();
    assert!(clock_at_right(&app));
    assert!(app.screen_text().lines().next().unwrap().contains("РУС  "));
    app.step(&mut NoHandler {});
    app.redraw();
    assert!(clock_at_right(&app));
    assert!(!app.screen_text().lines().next().unwrap().contains("РУС"));
    app.step(&mut NoHandler {});
    app.redraw();
    assert!(!clock_at_right(&app));
    assert!(app.screen_text().lines().next().unwrap().contains("Справка"));
    app.step(&mut NoHandler {});
    app.redraw();
    assert!(clock_at_right(&app));
}
