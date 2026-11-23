use super::*;
use crate::edit_lists::{create_list, delete_list};
use crate::edit_tasks::{create_task, create_task_depth, delete_task, move_task_depth};
use crate::outbox::{export_dirty, import_changes, mark_synced};
use crate::store;
use crate::test_db::fresh;
use serde_json::{json, Map, Value};

fn f(title: &str) -> Map<String, Value> {
    json!({"title": title}).as_object().unwrap().clone()
}

/// Kette A > B > C > D (Ebenen 0 bis 3) in einer Liste.
fn chain(conn: &Connection) -> (String, Vec<String>) {
    let l = create_list(conn, "L", "", 1000).unwrap().id;
    let mut ids: Vec<String> = Vec::new();
    for (at, t) in (1000_u64..).zip(["A", "B", "C", "D"]) {
        let parent = ids.last().cloned();
        ids.push(create_task_depth(conn, &l, parent.as_deref(), &f(t), at, 6).unwrap().id);
    }
    (l, ids)
}

#[test]
fn depth_and_height_count_levels() {
    let c = fresh();
    let (_, ids) = chain(&c);
    assert_eq!(depth_of(&c, &ids[0]).unwrap(), 0);
    assert_eq!(depth_of(&c, &ids[3]).unwrap(), 3);
    assert_eq!(height_of(&c, &ids[0]).unwrap(), 3);
    assert_eq!(height_of(&c, &ids[3]).unwrap(), 0);
}

#[test]
fn the_default_stays_one_level_for_desk() {
    let c = fresh();
    let l = create_list(&c, "L", "", 1000).unwrap().id;
    let a = create_task(&c, &l, None, &f("A"), 1000).unwrap();
    let b = create_task(&c, &l, Some(&a.id), &f("B"), 1000).unwrap();
    assert!(matches!(create_task(&c, &l, Some(&b.id), &f("C"), 1000), Err(crate::TaskError::TooDeep)));
}

#[test]
fn the_limit_is_enforced_per_call() {
    let c = fresh();
    let l = create_list(&c, "L", "", 1000).unwrap().id;
    let mut parent: Option<String> = None;
    for i in 0..3 {
        let t = create_task_depth(&c, &l, parent.as_deref(), &f("x"), 1000 + i, 2).unwrap();
        parent = Some(t.id);
    }
    assert!(matches!(
        create_task_depth(&c, &l, parent.as_deref(), &f("zu tief"), 2000, 2),
        Err(crate::TaskError::TooDeep)
    ));
}

#[test]
fn moving_a_subtree_checks_its_height_and_forbids_cycles() {
    let c = fresh();
    let (l, ids) = chain(&c);
    let other = create_task(&c, &l, None, &f("Z"), 2000).unwrap().id;
    // A (Höhe 3) unter Z (Ebene 0): Tiefe 4 > 3
    assert!(matches!(
        move_task_depth(&c, &ids[0], &l, Some(&other), None, 1.0, 3000, 3),
        Err(crate::TaskError::TooDeep)
    ));
    assert!(move_task_depth(&c, &ids[0], &l, Some(&other), None, 1.0, 3000, 4).is_ok());
    // Zyklus: Z unter seinen eigenen Nachfahren D
    assert!(matches!(
        move_task_depth(&c, &other, &l, Some(&ids[3]), None, 1.0, 4000, 9),
        Err(crate::TaskError::Invalid("parent"))
    ));
    assert!(matches!(
        move_task_depth(&c, &ids[1], &l, Some(&ids[1]), None, 1.0, 4000, 9),
        Err(crate::TaskError::Invalid("parent"))
    ));
    assert!(matches!(
        move_task_depth(&c, &ids[0], &l, Some(&ids[3]), None, 1.0, 4000, 9),
        Err(crate::TaskError::Invalid("parent"))
    ));
}

#[test]
fn moving_to_another_list_takes_the_whole_subtree_along() {
    let c = fresh();
    let (l, ids) = chain(&c);
    let l2 = create_list(&c, "M", "", 2000).unwrap().id;
    move_task_depth(&c, &ids[1], &l2, None, None, 1.0, 3000, 6).unwrap();
    for id in &ids[1..] {
        assert_eq!(store::task(&c, id).unwrap().list_id, l2, "{id}");
    }
    assert_eq!(store::task(&c, &ids[0]).unwrap().list_id, l);
}

#[test]
fn deleting_removes_all_descendants_as_tombstones() {
    let c = fresh();
    let (l, ids) = chain(&c);
    delete_task(&c, &ids[1], 5000).unwrap();
    assert_eq!(store::tasks_in_list(&c, &l).unwrap().len(), 1);
    let (n, dirty): (i64, i64) = c
        .query_row("SELECT count(*), min(dirty) FROM tasks WHERE deleted_at IS NOT NULL", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((n, dirty), (3, 1));
}

#[test]
fn tasks_in_list_walks_the_tree_depth_first() {
    let c = fresh();
    let l = create_list(&c, "L", "", 1000).unwrap().id;
    let a = create_task_depth(&c, &l, None, &f("A"), 1000, 6).unwrap().id;
    let a1 = create_task_depth(&c, &l, Some(&a), &f("A1"), 1000, 6).unwrap().id;
    create_task_depth(&c, &l, Some(&a1), &f("A1a"), 1000, 6).unwrap();
    create_task_depth(&c, &l, Some(&a), &f("A2"), 1000, 6).unwrap();
    create_task_depth(&c, &l, None, &f("B"), 1000, 6).unwrap();
    let titles: Vec<String> = store::tasks_in_list(&c, &l).unwrap().into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["A", "A1", "A1a", "A2", "B"]);
}

#[test]
fn deleting_a_list_tombstones_deep_tasks_too() {
    let c = fresh();
    let (l, _) = chain(&c);
    delete_list(&c, &l, 5000).unwrap();
    let n: i64 = c.query_row("SELECT count(*) FROM tasks WHERE deleted_at IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn a_deep_chain_syncs_in_the_right_order_even_when_the_batch_is_shuffled() {
    let (a, b) = (fresh(), fresh());
    let (_, ids) = chain(&a);
    let mut changes = export_dirty(&a).unwrap();
    // Export liefert Eltern vor Kindern; der Import muss auch eine durchmischte Folge verkraften.
    let order: Vec<&str> = changes.iter().filter(|c| c.table == "tasks").map(|c| c.id.as_str()).collect();
    assert_eq!(order, ids.iter().map(String::as_str).collect::<Vec<_>>());
    changes.reverse();
    import_changes(&b, &changes, 9000).unwrap();
    assert_eq!(store::task(&b, &ids[3]).unwrap().parent_id.as_deref(), Some(ids[2].as_str()));
    mark_synced(&a, &export_dirty(&a).unwrap()).unwrap();
}
