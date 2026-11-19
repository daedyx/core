use super::*;
use crate::edit_lists::create_list;
use crate::edit_tasks::{create_task, update_task};
use crate::test_db::fresh;
use serde_json::{json, Map};

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn one(conn: &Connection) -> String {
    let l = create_list(conn, "A", "", 1000).unwrap();
    create_task(conn, &l.id, None, &fields(json!({"title": "x", "due": "2026-11-19"})), 1000).unwrap().id
}

#[test]
fn several_alarms_are_stored_sorted_and_the_first_one_stays_in_remind_minutes() {
    let c = fresh();
    let t = one(&c);
    set_alarms(&c, &t, &[60, 10, 1440], 2000).unwrap();
    assert_eq!(alarms(&c, &t).unwrap(), [10, 60, 1440]);
    assert_eq!(crate::store::task(&c, &t).unwrap().remind_minutes, Some(10), "alte Apps sehen die erste Erinnerung");
}

#[test]
fn replacing_keeps_matching_rows_and_tombstones_the_rest() {
    let c = fresh();
    let t = one(&c);
    set_alarms(&c, &t, &[10, 60], 2000).unwrap();
    set_alarms(&c, &t, &[60, 5], 3000).unwrap();
    assert_eq!(alarms(&c, &t).unwrap(), [5, 60]);
    let (rows, dead): (i64, i64) = c
        .query_row("SELECT count(*), count(deleted_at) FROM task_alarms", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((rows, dead), (3, 1), "60 bleibt, 10 wird Grabstein, 5 neu");
    set_alarms(&c, &t, &[10], 4000).unwrap();
    let n: i64 = c.query_row("SELECT count(*) FROM task_alarms", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3, "10 wird wiederbelebt statt doppelt angelegt");
    assert_eq!(alarms(&c, &t).unwrap(), [10]);
}

#[test]
fn an_empty_list_removes_all_and_clears_remind_minutes() {
    let c = fresh();
    let t = one(&c);
    set_alarms(&c, &t, &[10], 2000).unwrap();
    set_alarms(&c, &t, &[], 3000).unwrap();
    assert!(alarms(&c, &t).unwrap().is_empty());
    assert_eq!(crate::store::task(&c, &t).unwrap().remind_minutes, None);
}

#[test]
fn bad_input_is_refused() {
    let c = fresh();
    let t = one(&c);
    assert!(set_alarms(&c, &t, &[-1], 1).is_err());
    assert!(set_alarms(&c, &t, &[525_601], 1).is_err());
    assert!(set_alarms(&c, &t, &[1; 11].iter().enumerate().map(|(i, _)| i as i64).collect::<Vec<_>>(), 1).is_err());
    assert!(set_alarms(&c, "nope", &[5], 1).is_err());
}

#[test]
fn duplicates_collapse() {
    let c = fresh();
    let t = one(&c);
    set_alarms(&c, &t, &[10, 10, 30], 2000).unwrap();
    assert_eq!(alarms(&c, &t).unwrap(), [10, 30]);
}

#[test]
fn the_single_remind_minutes_field_of_desk_keeps_working_through_the_alarms() {
    let c = fresh();
    let t = one(&c);
    update_task(&c, &t, &fields(json!({"remind_minutes": 30})), 2000).unwrap();
    assert_eq!(alarms(&c, &t).unwrap(), [30]);
    update_task(&c, &t, &fields(json!({"remind_minutes": null})), 3000).unwrap();
    assert!(alarms(&c, &t).unwrap().is_empty());
    let l = create_list(&c, "B", "", 1000).unwrap();
    let n = create_task(&c, &l.id, None, &fields(json!({"title": "y", "remind_minutes": 15})), 1000).unwrap();
    assert_eq!(alarms(&c, &n.id).unwrap(), [15]);
}
