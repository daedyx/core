//! Stempel und Eltern im Abgleich (Review Kern-Fix M-1, N-1): ein fremder Stempel in falscher Form oder weit in der
//! Zukunft wird samt Feld verworfen und bewegt die eigene Uhr nicht; eine Zeile ohne Eltern hält den Stapel nicht auf.
use super::tests::fields;
use super::*;
use crate::edit_lists::create_list;
use crate::edit_tasks::{create_task, update_task};
use crate::meta::next_stamp;
use crate::store::task;
use crate::test_db::fresh;
use serde_json::json;

const NOW_MS: u64 = 5_000_000;
const DAY_MS: u64 = 86_400_000;

fn setup() -> (Connection, String, String) {
    let conn = fresh();
    let l = create_list(&conn, "L", "", 1000).unwrap().id;
    let t = create_task(&conn, &l, None, &fields(json!({"title": "Lokal"})), 1000).unwrap().id;
    (conn, l, t)
}

fn change(table: &str, id: &str, items: &[(&str, Value)], stamp: &str) -> Change {
    let mut c = Change { table: table.into(), id: id.into(), fields: BTreeMap::new(), clocks: BTreeMap::new() };
    for (f, v) in items {
        c.fields.insert((*f).into(), v.clone());
        c.clocks.insert((*f).into(), stamp.into());
    }
    c
}

fn clock(conn: &Connection) -> String {
    conn.query_row("SELECT value FROM task_meta WHERE key = 'clock'", [], |r| r.get(0)).unwrap()
}

#[test]
fn malformed_or_far_future_stamps_are_dropped_and_do_not_move_the_clock() {
    let far = format!("{:012x}-0000-x", NOW_MS.saturating_add(DAY_MS).saturating_add(1));
    for bad in ["zzzzzzzzzzzz-0000-x", "ffffffffffff-ffff-x", "1000000000000-0000-x", "00000000ff-0000-x", &far] {
        let (conn, _, t) = setup();
        let before = clock(&conn);
        let c = change("tasks", &t, &[("title", json!("Fremd"))], bad);
        import_changes(&conn, std::slice::from_ref(&c), NOW_MS).unwrap();
        assert_eq!(task(&conn, &t).unwrap().title, "Lokal", "{bad}");
        assert_eq!(clock(&conn), before, "{bad}: Uhr unverändert");
        update_task(&conn, &t, &fields(json!({"title": "Lokal neu"})), 6000).unwrap();
        import_changes(&conn, &[c], NOW_MS).unwrap();
        assert_eq!(task(&conn, &t).unwrap().title, "Lokal neu", "{bad}: lokale Änderung bleibt");
        let (a, b) = (next_stamp(&conn, 3_000_000).unwrap(), next_stamp(&conn, 3_000_001).unwrap());
        assert!(a < b, "{bad}: Stempel streng aufsteigend");
    }
}

#[test]
fn a_well_formed_stamp_up_to_one_day_ahead_is_accepted() {
    let (conn, _, t) = setup();
    let edge = format!("{:012x}-0000-x", NOW_MS.saturating_add(DAY_MS));
    import_changes(&conn, &[change("tasks", &t, &[("title", json!("Fremd"))], &edge)], NOW_MS).unwrap();
    assert_eq!(task(&conn, &t).unwrap().title, "Fremd");
    assert!(next_stamp(&conn, 1000).unwrap() > edge);
}

#[test]
fn a_full_counter_does_not_freeze_the_clock() {
    let (conn, _, t) = setup();
    let full = format!("{NOW_MS:012x}-ffff-x");
    import_changes(&conn, &[change("tasks", &t, &[("title", json!("Fremd"))], &full)], NOW_MS).unwrap();
    let a = next_stamp(&conn, NOW_MS).unwrap();
    let b = next_stamp(&conn, NOW_MS).unwrap();
    assert!(full < a && a < b, "{full} < {a} < {b}");
}

#[test]
fn rows_with_missing_parents_wait_and_valid_changes_still_arrive() {
    let (conn, l, t) = setup();
    let stamp = format!("{NOW_MS:012x}-0000-x");
    let batch = [
        change("tasks", &t, &[("title", json!("Gültig"))], &stamp),
        change("tasks", "ohne-liste", &[("list_id", json!("gibt-es-nicht")), ("title", json!("A"))], &stamp),
        change("task_sections", "ohne-liste-s", &[("list_id", json!("gibt-es-nicht")), ("name", json!("S"))], &stamp),
        change(
            "tasks",
            "ohne-eltern",
            &[("list_id", json!(l)), ("parent_id", json!("fehlt")), ("title", json!("B"))],
            &stamp,
        ),
        change(
            "tasks",
            "ohne-abschnitt",
            &[("list_id", json!(l)), ("section_id", json!("fehlt")), ("title", json!("C"))],
            &stamp,
        ),
        change("task_alarms", "ohne-aufgabe", &[("task_id", json!("fehlt")), ("minutes_before", json!(5))], &stamp),
        change("task_completions", "ohne-aufgabe-v", &[("task_id", json!("fehlt")), ("done_at", json!(1))], &stamp),
    ];
    assert_eq!(import_changes_counted(&conn, &batch, NOW_MS).unwrap(), 6);
    assert_eq!(task(&conn, &t).unwrap().title, "Gültig");
    for id in ["ohne-liste", "ohne-eltern", "ohne-abschnitt"] {
        assert!(task(&conn, id).is_err(), "{id} wartet");
    }
    let n: i64 = conn.query_row("SELECT count(*) FROM task_alarms", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    import_changes(&conn, &batch, NOW_MS).unwrap();
}
