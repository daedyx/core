//! Fälligkeit mit Uhrzeit nur 1970 bis 9999 (Review Tasks T0 M-1): sonst laufen spätere Rechnungen über.
use super::*;
use crate::edit_lists::create_list;
use crate::test_db::fresh;

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

const BAD: [&str; 8] = [
    "-9223372036854775808",
    "9223372036854775807",
    "-1",
    "253402300800",
    " 5",
    "1969-12-31",
    "0000-01-01",
    "+10000-01-01",
];

#[test]
fn due_outside_1970_to_9999_is_refused_on_create_and_update() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    let ok = create_task(&conn, &l, None, &fields(json!({"title": "x", "due": "2026-12-01"})), 1000).unwrap();
    for due in BAD {
        let f = fields(json!({"title": "x", "due": due, "due_has_time": true}));
        assert!(matches!(create_task(&conn, &l, None, &f, 1000), Err(TaskError::Invalid("due"))), "{due}");
        let p = fields(json!({"due": due}));
        assert!(matches!(update_task(&conn, &ok.id, &p, 2000), Err(TaskError::Invalid("due"))), "{due}");
    }
    assert_eq!(store::task(&conn, &ok.id).unwrap(), ok, "abgelehnte Änderungen lassen die Aufgabe unverändert");
}

#[test]
fn the_edges_and_plain_dates_are_still_accepted() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    for due in ["0", "253402300799", "1795096800", "2026-12-01", "1970-01-01", "9999-12-31"] {
        let f = fields(json!({"title": "x", "due": due, "due_has_time": due.len() != 10}));
        assert_eq!(create_task(&conn, &l, None, &f, 1000).unwrap().due.as_deref(), Some(due));
    }
}
