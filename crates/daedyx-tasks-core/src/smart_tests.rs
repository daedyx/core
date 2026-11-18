use super::*;
use crate::edit_done::toggle_done;
use crate::edit_lists::{create_list, delete_list};
use crate::edit_tasks::create_task;
use crate::test_db::fresh;
use chrono_tz::Europe::Berlin;
use serde_json::{json, Map, Value};

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 19).unwrap()
}

fn titles(v: Vec<Task>) -> Vec<String> {
    v.into_iter().map(|t| t.title).collect()
}

/// Liste mit: gestern fällig, heute fällig (hoch), heute 23:30 Ortszeit, übermorgen, ohne Datum, erledigt.
fn setup() -> (Connection, String) {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    let mk = |f: Value| create_task(&conn, &l, None, &fields(f), 1000).unwrap();
    mk(json!({"title": "gestern", "due": "2026-11-18"}));
    mk(json!({"title": "heute niedrig", "due": "2026-11-19", "priority": 1}));
    mk(json!({"title": "heute hoch", "due": "2026-11-19", "priority": 3}));
    // 23:30 Berlin am 19.11. = 22:30 UTC
    mk(json!({"title": "spät", "due": "1795127400", "due_has_time": true}));
    mk(json!({"title": "übermorgen", "due": "2026-11-21"}));
    mk(json!({"title": "ohne Datum"}));
    let done = mk(json!({"title": "fertig", "due": "2026-11-19"}));
    toggle_done(&conn, &done.id, 1_795_089_600_000, Berlin).unwrap();
    (conn, l)
}

#[test]
fn today_overdue_and_planned() {
    let (conn, _) = setup();
    let q = |v| titles(query(&conn, v, None, today(), Berlin).unwrap());
    assert_eq!(q(View::Today), ["heute hoch", "heute niedrig", "spät"]);
    assert_eq!(q(View::Overdue), ["gestern"]);
    assert_eq!(q(View::Planned), ["gestern", "heute hoch", "heute niedrig", "spät", "übermorgen"]);
}

#[test]
fn all_is_open_only_and_done_is_done_only() {
    let (conn, _) = setup();
    let all = titles(query(&conn, View::All, None, today(), Berlin).unwrap());
    assert_eq!(all.len(), 6);
    assert!(!all.contains(&"fertig".to_string()));
    assert_eq!(titles(query(&conn, View::Done, None, today(), Berlin).unwrap()), ["fertig"]);
}

#[test]
fn list_view_needs_a_list_and_deleted_lists_vanish_everywhere() {
    let (conn, l) = setup();
    assert!(matches!(query(&conn, View::List, None, today(), Berlin), Err(TaskError::Invalid(_))));
    assert_eq!(query(&conn, View::List, Some(&l), today(), Berlin).unwrap().len(), 7);
    delete_list(&conn, &l, 5000).unwrap();
    assert!(query(&conn, View::All, None, today(), Berlin).unwrap().is_empty());
    assert!(search(&conn, "heute", 10).unwrap().is_empty());
}

#[test]
fn counts_match_the_views() {
    let (conn, _) = setup();
    let c = counts(&conn, today(), Berlin).unwrap();
    assert_eq!(c.map(|(_, n)| n), [3, 5, 1, 6]);
}

#[test]
fn search_finds_title_and_notes_case_insensitively_open_first() {
    let (conn, l) = setup();
    create_task(&conn, &l, None, &fields(json!({"title": "Zahnarzt", "notes": "Termin 100% sicher_"})), 2000).unwrap();
    assert_eq!(titles(search(&conn, "HEUTE", 10).unwrap()), ["heute niedrig", "heute hoch"]);
    assert_eq!(titles(search(&conn, "100%", 10).unwrap()), ["Zahnarzt"]);
    assert!(search(&conn, "%", 10).unwrap().len() == 1, "Platzhalter wird nicht als Muster gedeutet");
    assert_eq!(search(&conn, "e", 2).unwrap().len(), 2);
    assert!(search(&conn, "  ", 10).unwrap().is_empty());
    assert_eq!(titles(search(&conn, "fertig", 10).unwrap()), ["fertig"]);
}
