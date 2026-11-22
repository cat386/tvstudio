use super::*;
use tvision::platform::HeadlessBackend;

#[test]
fn click_coordinates() {
    // Меню нет — рабочий стол начинается со строки 0 экрана.
    // Окно — (3, 1) на столе; линейка — (1, 1) в окне; значит, её
    // (0, 0) на экране — (4, 2). Щелчок в (10, 5) — это (6, 3) у линейки.
    let mut b = HeadlessBackend::new(80, 25);
    b.push_click(10, 5);
    let mut app = Program::new(Box::new(b), None, Some(status()));
    open_window(&mut app);
    app.pump(&mut App);
    assert!(app.screen_text().contains("свои: (6, 3)   в окне: (7, 4)"), "{}", app.screen_text());
}

#[test]
fn dialog_inside_frame() {
    let d = layout_dialog();
    let s = d.win.group.vs.size;
    let inside = Rect::new(1, 1, s.x - 1, s.y - 1);
    for c in &d.win.group.children {
        let b = c.vs().bounds();
        assert_eq!(inside.intersect(&b), b, "{:?} залезает на рамку", b);
    }
}
