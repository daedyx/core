//! Mehrere Erinnerungen wandern feldweise zwischen zwei Datenbanken.
use super::tests::{dump, fields, sync};
use super::*;
use crate::edit_alarms::{alarms, set_alarms};
use crate::edit_lists::create_list;
use crate::edit_tasks::create_task;
use crate::test_db::fresh;
use serde_json::json;

fn setup(a: &Connection) -> String {
    let l = create_list(a, "L", "", 1000).unwrap();
    create_task(a, &l.id, None, &fields(json!({"title": "Termin", "due": "2026-12-01"})), 1000).unwrap().id
}

#[test]
fn alarms_arrive_with_their_task_and_both_sides_end_clean() {
    let (a, b) = (fresh(), fresh());
    let t = setup(&a);
    set_alarms(&a, &t, &[15, 120], 2000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    assert_eq!(alarms(&b, &t).unwrap(), [15, 120]);
    assert!(export_dirty(&a).unwrap().is_empty() && export_dirty(&b).unwrap().is_empty());
}

#[test]
fn removing_an_alarm_on_one_side_reaches_the_other() {
    let (a, b) = (fresh(), fresh());
    let t = setup(&a);
    set_alarms(&a, &t, &[15, 120], 2000).unwrap();
    sync(&a, &b, 3000);
    set_alarms(&b, &t, &[120], 4000).unwrap();
    sync(&a, &b, 5000);
    assert_eq!(alarms(&a, &t).unwrap(), [120]);
    assert_eq!(dump(&a), dump(&b));
}

#[test]
fn different_alarms_added_on_both_sides_at_once_are_all_kept() {
    let (a, b) = (fresh(), fresh());
    let t = setup(&a);
    sync(&a, &b, 2000);
    set_alarms(&a, &t, &[10], 3000).unwrap();
    set_alarms(&b, &t, &[60], 3500).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    assert_eq!(alarms(&a, &t).unwrap(), [10, 60], "beide Zeilen bleiben, jede hat ihre eigene Identität");
}

#[test]
fn the_same_alarm_row_changed_on_both_sides_goes_to_the_newer_stamp() {
    let (a, b) = (fresh(), fresh());
    let t = setup(&a);
    set_alarms(&a, &t, &[10], 2000).unwrap();
    sync(&a, &b, 3000);
    set_alarms(&a, &t, &[], 4000).unwrap();
    set_alarms(&b, &t, &[10], 5000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
}

#[test]
fn an_alarm_for_a_task_not_yet_known_waits_like_any_orphan() {
    let b = fresh();
    let stamp = "000000000001-0000-z".to_string();
    let c = Change {
        table: "task_alarms".into(),
        id: "a1".into(),
        fields: BTreeMap::from([("task_id".to_string(), json!("fehlt")), ("minutes_before".to_string(), json!(5))]),
        clocks: BTreeMap::from([("task_id".to_string(), stamp.clone()), ("minutes_before".to_string(), stamp)]),
    };
    assert_eq!(import_changes_counted(&b, &[c], 1).unwrap(), 1, "wartet, bricht den Stapel nicht ab");
    let n: i64 = b.query_row("SELECT count(*) FROM task_alarms", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}
