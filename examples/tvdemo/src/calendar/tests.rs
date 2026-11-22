use super::*;

#[test]
fn weekdays() {
    assert_eq!(day_of_week(1, 1, 2024), 1); // понедельник
    assert_eq!(day_of_week(22, 10, 2026), 4); // четверг
}
