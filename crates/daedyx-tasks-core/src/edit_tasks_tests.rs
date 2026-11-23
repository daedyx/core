use super::*;
use crate::edit_lists::create_list;
use crate::test_db::fresh;

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn setup() -> (Connection, String) {
    let conn = fresh();
    let l = create_list(&conn, "Haushalt", "", 1000).unwrap();
    (conn, l.id)
}

#[test]
fn creates_tasks_at_the_end_with_all_given_fields() {
    let (conn, l) = setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": " Müll rausbringen "})), 2000).unwrap();
    let b = create_task(
        &conn,
        &l,
        None,
        &fields(
            json!({"title": "Steuer", "notes": "Belege", "due": "2026-12-01", "priority": 3, "remind_minutes": 30}),
        ),
        3000,
    )
    .unwrap();
    assert_eq!(a.title, "Müll rausbringen");
    assert!(a.position < b.position);
    assert_eq!(
        (b.due.as_deref(), b.priority, b.remind_minutes, b.notes.as_str()),
        (Some("2026-12-01"), 3, Some(30), "Belege")
    );
    assert!(b.assignee_id.is_none() && b.created_by.is_none());
}

#[test]
fn rejects_bad_input() {
    let (conn, l) = setup();
    for bad in [
        json!({}),
        json!({"title": "  "}),
        json!({"title": "x", "priority": 4}),
        json!({"title": "x", "due": "morgen"}),
        json!({"title": "x", "remind_minutes": -5}),
        json!({"title": "x", "repeat_rule": {"kind": "daily", "interval": 0}}),
    ] {
        assert!(matches!(create_task(&conn, &l, None, &fields(bad), 1), Err(TaskError::Invalid(_))));
    }
    assert!(matches!(
        create_task(&conn, "nope", None, &fields(json!({"title": "x"})), 1),
        Err(TaskError::ListNotFound)
    ));
}

#[test]
fn only_one_level_of_subtasks() {
    let (conn, l) = setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A"})), 1000).unwrap();
    let b = create_task(&conn, &l, Some(&a.id), &fields(json!({"title": "B"})), 1000).unwrap();
    assert_eq!(b.parent_id.as_deref(), Some(a.id.as_str()));
    assert!(matches!(
        create_task(&conn, &l, Some(&b.id), &fields(json!({"title": "C"})), 1000),
        Err(TaskError::TooDeep)
    ));
    let c = create_task(&conn, &l, None, &fields(json!({"title": "C"})), 1000).unwrap();
    // eine Aufgabe mit Unteraufgaben darf nicht selbst Unteraufgabe werden
    assert!(matches!(move_task(&conn, &a.id, &l, Some(&c.id), None, 1.0, 2000), Err(TaskError::TooDeep)));
    assert!(matches!(move_task(&conn, &c.id, &l, Some(&c.id), None, 1.0, 2000), Err(TaskError::Invalid(_))));
}

#[test]
fn update_changes_only_named_fields_and_stamps_them() {
    let (conn, l) = setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A", "priority": 1})), 1000).unwrap();
    let u =
        update_task(&conn, &a.id, &fields(json!({"title": "Neu", "due": "2026-12-24", "due_has_time": false})), 2000)
            .unwrap();
    assert_eq!((u.title.as_str(), u.priority, u.due.as_deref()), ("Neu", 1, Some("2026-12-24")));
    let clocks: String = conn.query_row("SELECT clocks FROM tasks WHERE id = ?1", [&a.id], |r| r.get(0)).unwrap();
    let c: std::collections::BTreeMap<String, String> = serde_json::from_str(&clocks).unwrap();
    assert!(c["title"] > c["priority"]);
    assert!(matches!(update_task(&conn, &a.id, &fields(json!({"list_id": "x"})), 3000), Err(TaskError::Invalid(_))));
    assert!(matches!(
        update_task(&conn, &a.id, &fields(json!({"assignee_id": "p"})), 3000),
        Err(TaskError::Invalid(_))
    ));
}

#[test]
fn move_changes_list_and_takes_subtasks_along() {
    let (conn, l) = setup();
    let l2 = create_list(&conn, "Arbeit", "", 1000).unwrap().id;
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A"})), 1000).unwrap();
    let k = create_task(&conn, &l, Some(&a.id), &fields(json!({"title": "K"})), 1000).unwrap();
    let m = move_task(&conn, &a.id, &l2, None, None, 2.5, 2000).unwrap();
    assert_eq!((m.list_id.as_str(), m.position), (l2.as_str(), 2.5));
    assert_eq!(store::task(&conn, &k.id).unwrap().list_id, l2);
    assert!(matches!(move_task(&conn, &a.id, "nope", None, None, 1.0, 2000), Err(TaskError::ListNotFound)));
}

#[test]
fn delete_removes_task_and_subtasks_but_keeps_tombstones() {
    let (conn, l) = setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A"})), 1000).unwrap();
    create_task(&conn, &l, Some(&a.id), &fields(json!({"title": "K"})), 1000).unwrap();
    delete_task(&conn, &a.id, 3000).unwrap();
    assert!(store::tasks_in_list(&conn, &l).unwrap().is_empty());
    let n: i64 = conn
        .query_row("SELECT count(*) FROM tasks WHERE deleted_at IS NOT NULL AND dirty = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 2);
    assert!(matches!(delete_task(&conn, &a.id, 4000), Err(TaskError::TaskNotFound)));
}

#[test]
fn tasks_in_list_puts_subtasks_after_their_parent() {
    let (conn, l) = setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A"})), 1000).unwrap();
    let b = create_task(&conn, &l, None, &fields(json!({"title": "B"})), 1000).unwrap();
    create_task(&conn, &l, Some(&a.id), &fields(json!({"title": "A1"})), 1000).unwrap();
    let titles: Vec<_> = store::tasks_in_list(&conn, &l).unwrap().into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["A", "A1", "B"]);
    let _ = b;
}

fn section_setup() -> (Connection, String, String, String) {
    let (conn, l) = setup();
    let s1 = crate::edit_sections::create_section(&conn, &l, "Offen", 1000).unwrap().id;
    let s2 = crate::edit_sections::create_section(&conn, &l, "Läuft", 1000).unwrap().id;
    (conn, l, s1, s2)
}

#[test]
fn a_new_task_can_start_in_a_section_of_its_list() {
    let (conn, l, s1, _) = section_setup();
    let t = create_task(&conn, &l, None, &fields(json!({"title": "A", "section_id": s1})), 2000).unwrap();
    assert_eq!(t.section_id.as_deref(), Some(s1.as_str()));
    let plain = create_task(&conn, &l, None, &fields(json!({"title": "B"})), 2000).unwrap();
    assert!(plain.section_id.is_none());
}

#[test]
fn a_section_of_another_list_or_for_a_subtask_is_refused() {
    let (conn, l, s1, _) = section_setup();
    let l2 = create_list(&conn, "Arbeit", "", 1000).unwrap().id;
    assert!(matches!(
        create_task(&conn, &l2, None, &fields(json!({"title": "A", "section_id": s1})), 1),
        Err(TaskError::Invalid("section"))
    ));
    assert!(matches!(
        create_task(&conn, &l, None, &fields(json!({"title": "A", "section_id": "nope"})), 1),
        Err(TaskError::SectionNotFound)
    ));
    let p = create_task(&conn, &l, None, &fields(json!({"title": "P"})), 1).unwrap();
    assert!(matches!(
        create_task(&conn, &l, Some(&p.id), &fields(json!({"title": "K", "section_id": s1})), 1),
        Err(TaskError::Invalid("section"))
    ));
}

#[test]
fn moving_sets_the_section_and_a_change_of_list_drops_it() {
    let (conn, l, s1, s2) = section_setup();
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A"})), 2000).unwrap();
    let m = move_task(&conn, &a.id, &l, None, Some(&s2), 3.0, 3000).unwrap();
    assert_eq!((m.section_id.as_deref(), m.position), (Some(s2.as_str()), 3.0));
    let back = move_task(&conn, &a.id, &l, None, None, 4.0, 4000).unwrap();
    assert!(back.section_id.is_none());
    move_task(&conn, &a.id, &l, None, Some(&s1), 5.0, 5000).unwrap();
    let l2 = create_list(&conn, "Arbeit", "", 1000).unwrap().id;
    assert!(matches!(move_task(&conn, &a.id, &l2, None, Some(&s1), 1.0, 6000), Err(TaskError::Invalid("section"))));
    let gone = move_task(&conn, &a.id, &l2, None, None, 1.0, 6000).unwrap();
    assert!(gone.section_id.is_none());
}

#[test]
fn subtasks_never_carry_a_section_and_follow_their_parent() {
    let (conn, l, s1, _) = section_setup();
    let p = create_task(&conn, &l, None, &fields(json!({"title": "P", "section_id": s1})), 2000).unwrap();
    let k = create_task(&conn, &l, Some(&p.id), &fields(json!({"title": "K"})), 2000).unwrap();
    assert!(k.section_id.is_none());
    let a = create_task(&conn, &l, None, &fields(json!({"title": "A", "section_id": s1})), 2000).unwrap();
    let m = move_task(&conn, &a.id, &l, Some(&p.id), Some(&s1), 1.0, 3000);
    assert!(matches!(m, Err(TaskError::Invalid("section"))));
    let m = move_task(&conn, &a.id, &l, Some(&p.id), None, 1.0, 3000).unwrap();
    assert!(m.section_id.is_none());
}

#[test]
fn the_month_day_anchor_is_accepted_between_1_and_31_only() {
    let (conn, l) = setup();
    let ok = create_task(
        &conn,
        &l,
        None,
        &fields(json!({"title": "Miete", "due": "2026-01-31", "repeat_rule": {"kind": "monthly", "interval": 1, "monthDay": 31}})),
        1000,
    )
    .unwrap();
    assert_eq!(ok.repeat.unwrap().month_day, Some(31));
    for bad in [0, 32, 255] {
        let r = create_task(
            &conn,
            &l,
            None,
            &fields(json!({"title": "x", "repeat_rule": {"kind": "monthly", "interval": 1, "monthDay": bad}})),
            1000,
        );
        assert!(r.is_err(), "{bad}");
    }
}
