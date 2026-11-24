//! Der Abgleich nimmt nur Werte an, die auch die Eingabe annähme (Review Tasks T0 M-1, Nacht-Review N-1):
//! ein ungültiger fremder Wert wird verworfen, der lokale bleibt, gültige Felder derselben Änderung kommen an.
use super::tests::fields;
use super::*;
use crate::edit_alarms::{alarms, set_alarms};
use crate::edit_lists::create_list;
use crate::edit_tasks::create_task;
use crate::store::task;
use crate::test_db::fresh;
use serde_json::json;

/// 10 000 000 ms: später als alle lokalen Stempel der Tests, aber nicht mehr als einen Tag nach dem Einspielen.
const LATER: &str = "000000989680-0000-z";

fn setup() -> (Connection, String) {
    let conn = fresh();
    let l = create_list(&conn, "L", "", 1000).unwrap().id;
    let f = json!({"title": "Termin", "due": "1795096800", "due_has_time": true, "priority": 1});
    let t = create_task(&conn, &l, None, &fields(f), 1000).unwrap().id;
    set_alarms(&conn, &t, &[30], 1000).unwrap();
    (conn, t)
}

fn change(table: &str, id: &str, items: &[(&str, Value)]) -> Change {
    let mut c = Change { table: table.into(), id: id.into(), fields: BTreeMap::new(), clocks: BTreeMap::new() };
    for (f, v) in items {
        c.fields.insert((*f).into(), v.clone());
        c.clocks.insert((*f).into(), LATER.into());
    }
    c
}

#[test]
fn invalid_remote_task_fields_are_dropped_and_valid_ones_arrive() {
    let (conn, t) = setup();
    let before = task(&conn, &t).unwrap();
    for (f, v) in [
        ("due", json!("-9223372036854775808")),
        ("due", json!("9223372036854775807")),
        ("due", json!("-1")),
        ("due", json!("morgen")),
        ("due", json!(5)),
        ("remind_minutes", json!(i64::MAX)),
        ("remind_minutes", json!(-1)),
        ("priority", json!(99)),
        ("due_has_time", json!(7)),
        ("done_at", json!("gestern")),
        ("repeat_rule", json!(r#"{"kind":"weekly","interval":1,"weekdays":[9]}"#)),
        ("repeat_rule", json!(r#"{"kind":"daily","interval":0}"#)),
        ("repeat_rule", json!("kaputt")),
    ] {
        let c = change("tasks", &t, &[(f, v.clone()), ("title", json!("Neu"))]);
        import_changes(&conn, &[c], 5000).unwrap();
        let now = task(&conn, &t).unwrap();
        assert_eq!(now.title, "Neu", "{f} = {v}: gültiges Feld kommt an");
        let mut same = now.clone();
        same.title.clone_from(&before.title);
        assert_eq!(same, before, "{f} = {v}: ungültiges Feld verworfen");
    }
}

#[test]
fn valid_remote_values_still_arrive() {
    let (conn, t) = setup();
    let rule = r#"{"kind":"weekly","interval":2,"weekdays":[1,5],"fromDone":false}"#;
    let c = change(
        "tasks",
        &t,
        &[
            ("due", json!("253402300799")),
            ("remind_minutes", json!(525_600)),
            ("priority", json!(3)),
            ("repeat_rule", json!(rule)),
            ("done_at", Value::Null),
        ],
    );
    import_changes(&conn, &[c], 5000).unwrap();
    let now = task(&conn, &t).unwrap();
    assert_eq!((now.due.as_deref(), now.remind_minutes, now.priority), (Some("253402300799"), Some(525_600), 3));
    assert!(now.repeat.is_some_and(|r| r.weekdays == [1, 5]));
}

#[test]
fn absurd_remote_alarm_minutes_are_dropped() {
    let (conn, t) = setup();
    let id: String = conn.query_row("SELECT id FROM task_alarms WHERE task_id = ?1", [&t], |r| r.get(0)).unwrap();
    for m in [json!(i64::MAX), json!(i64::MIN), json!(-1), json!(525_601), json!("abc")] {
        import_changes(&conn, &[change("task_alarms", &id, &[("minutes_before", m.clone())])], 5000).unwrap();
        assert_eq!(alarms(&conn, &t).unwrap(), [30], "{m}");
    }
    import_changes(&conn, &[change("task_alarms", &id, &[("minutes_before", json!(45))])], 5000).unwrap();
    assert_eq!(alarms(&conn, &t).unwrap(), [45]);
}

#[test]
fn a_new_remote_task_with_an_absurd_due_arrives_without_it() {
    let (conn, t) = setup();
    let list = task(&conn, &t).unwrap().list_id;
    let c = change("tasks", "fremd", &[("list_id", json!(list)), ("title", json!("Fremd")), ("due", json!("-1"))]);
    import_changes(&conn, &[c], 5000).unwrap();
    let got = task(&conn, "fremd").unwrap();
    assert_eq!((got.title.as_str(), got.due), ("Fremd", None));
}
