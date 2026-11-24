use super::*;
use crate::edit_done::toggle_done;
use crate::edit_lists::{create_list, delete_list};
use crate::edit_tasks::{create_task, delete_task, update_task};
use crate::test_db::fresh;
use chrono_tz::Europe::Berlin;
use serde_json::{json, Map};

pub(super) fn fields(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// Austausch in beide Richtungen, bis beide Seiten nichts mehr zu melden haben.
pub(super) fn sync(a: &Connection, b: &Connection, now_ms: u64) {
    for _ in 0..3 {
        let from_a = export_dirty(a).unwrap();
        import_changes(b, &from_a, now_ms).unwrap();
        mark_synced(a, &from_a).unwrap();
        let from_b = export_dirty(b).unwrap();
        import_changes(a, &from_b, now_ms).unwrap();
        mark_synced(b, &from_b).unwrap();
    }
}

/// Alle Zeilen der drei Tabellen (ohne `updated_at`/`dirty`) in fester Reihenfolge, zum Vergleichen.
pub(super) fn dump(conn: &Connection) -> Vec<Change> {
    let mut all = Vec::new();
    for table in TABLES {
        let ids: Vec<String> = conn
            .prepare(&format!("SELECT id FROM {} ORDER BY id", table.name))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for id in ids {
            let rec = read(conn, table, &id).unwrap().unwrap();
            all.push(Change { table: table.name.into(), id, fields: rec.fields, clocks: rec.clocks });
        }
    }
    all
}

fn title_of(conn: &Connection, id: &str) -> String {
    conn.query_row("SELECT title FROM tasks WHERE id = ?1", [id], |r| r.get(0)).unwrap()
}

#[test]
fn a_new_list_with_tasks_arrives_complete_and_both_sides_end_clean() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haushalt", "blue", 1000).unwrap();
    let t =
        create_task(&a, &l.id, None, &fields(json!({"title": "Einkaufen", "due": "2026-12-01", "priority": 2})), 2000)
            .unwrap();
    create_task(&a, &l.id, Some(&t.id), &fields(json!({"title": "Milch"})), 3000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    assert_eq!(crate::store::tasks_in_list(&b, &l.id).unwrap().len(), 2);
    assert!(export_dirty(&a).unwrap().is_empty() && export_dirty(&b).unwrap().is_empty());
}

#[test]
fn concurrent_changes_to_different_fields_both_survive_and_the_same_field_goes_to_the_newer() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "Start"})), 1000).unwrap();
    sync(&a, &b, 2000);
    update_task(&a, &t.id, &fields(json!({"priority": 3, "title": "von A"})), 5000).unwrap();
    update_task(&b, &t.id, &fields(json!({"notes": "von B", "title": "von B später"})), 6000).unwrap();
    sync(&a, &b, 7000);
    assert_eq!(dump(&a), dump(&b));
    let merged = crate::store::task(&a, &t.id).unwrap();
    assert_eq!((merged.priority, merged.notes.as_str()), (3, "von B"));
    assert_eq!(title_of(&a, &t.id), "von B später");
}

#[test]
fn a_delete_on_one_side_wins_over_an_older_edit_on_the_other() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "x"})), 1000).unwrap();
    sync(&a, &b, 2000);
    update_task(&b, &t.id, &fields(json!({"title": "bearbeitet"})), 3000).unwrap();
    delete_task(&a, &t.id, 4000).unwrap();
    sync(&a, &b, 5000);
    assert_eq!(dump(&a), dump(&b));
    assert!(crate::store::tasks_in_list(&b, &l.id).unwrap().is_empty());
}

#[test]
fn deleting_a_list_reaches_the_other_side_with_its_tasks() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    create_task(&a, &l.id, None, &fields(json!({"title": "x"})), 1000).unwrap();
    sync(&a, &b, 2000);
    delete_list(&b, &l.id, 3000).unwrap();
    sync(&a, &b, 4000);
    assert!(crate::store::lists(&a).unwrap().is_empty());
    assert_eq!(dump(&a), dump(&b));
}

#[test]
fn completions_of_a_repeating_task_travel_along() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    let t = create_task(
        &a,
        &l.id,
        None,
        &fields(json!({"title": "gießen", "due": "2026-11-19", "repeat_rule": {"kind": "weekly", "interval": 1}})),
        1000,
    )
    .unwrap();
    toggle_done(&a, &t.id, 1_795_089_600_000, Berlin).unwrap();
    sync(&a, &b, 2_000_000_000_000);
    assert_eq!(dump(&a), dump(&b));
    let n: i64 = b.query_row("SELECT count(*) FROM task_completions", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    assert_eq!(crate::store::task(&b, &t.id).unwrap().due.as_deref(), Some("2026-11-26"));
}

#[test]
fn a_change_made_after_export_stays_dirty_when_the_old_export_is_acknowledged() {
    let a = fresh();
    let l = create_list(&a, "L", "", 1000).unwrap();
    let sent = export_dirty(&a).unwrap();
    crate::edit_lists::rename_list(&a, &l.id, "Neu", 2000).unwrap();
    mark_synced(&a, &sent).unwrap();
    assert_eq!(export_dirty(&a).unwrap().len(), 1);
}

#[test]
fn after_importing_a_newer_stamp_local_edits_get_even_newer_stamps() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 9_000_000).unwrap();
    sync(&a, &b, 9_000_000);
    // Uhr von B geht weit nach: die Änderung muss trotzdem nach der von A kommen
    crate::edit_lists::rename_list(&b, &l.id, "von B", 1000).unwrap();
    sync(&a, &b, 9_100_000);
    assert_eq!(crate::store::list(&a, &l.id).unwrap().name, "von B");
}

#[test]
fn bad_input_is_refused_or_waits_and_leaves_nothing_behind() {
    let b = fresh();
    let bad = Change { table: "mail".into(), id: "x".into(), fields: BTreeMap::new(), clocks: BTreeMap::new() };
    assert!(import_changes(&b, &[bad], 1).is_err());
    let orphan = Change {
        table: "tasks".into(),
        id: "t".into(),
        fields: BTreeMap::from([("list_id".to_string(), json!("fehlt")), ("title".to_string(), json!("x"))]),
        clocks: BTreeMap::from([
            ("list_id".to_string(), "000000000001-0000-z".to_string()),
            ("title".to_string(), "000000000001-0000-z".to_string()),
        ]),
    };
    assert_eq!(import_changes_counted(&b, &[orphan], 1).unwrap(), 1, "Waise wartet (Review Kern-Fix N-1)");
    let n: i64 = b.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}
