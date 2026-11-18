use super::*;
use crate::edit_lists::{create_list, delete_list};
use crate::test_db::fresh;

fn names(conn: &Connection, list: &str) -> Vec<String> {
    store::sections(conn, list).unwrap().into_iter().map(|s| s.name).collect()
}

#[test]
fn creates_sections_in_order_and_trims_names() {
    let conn = fresh();
    let l = create_list(&conn, "Haus", "", 1000).unwrap();
    let a = create_section(&conn, &l.id, "  Offen ", 1000).unwrap();
    let b = create_section(&conn, &l.id, "Erledigt", 1000).unwrap();
    assert_eq!(a.name, "Offen");
    assert!(a.position < b.position);
    assert_eq!(names(&conn, &l.id), ["Offen", "Erledigt"]);
    let dirty: i64 = conn.query_row("SELECT min(dirty) FROM task_sections", [], |r| r.get(0)).unwrap();
    assert_eq!(dirty, 1);
}

#[test]
fn rejects_empty_names_and_unknown_lists() {
    let conn = fresh();
    let l = create_list(&conn, "Haus", "", 1000).unwrap();
    assert!(matches!(create_section(&conn, &l.id, "  ", 1), Err(TaskError::Invalid("name"))));
    assert!(matches!(create_section(&conn, "nope", "X", 1), Err(TaskError::ListNotFound)));
    let s = create_section(&conn, &l.id, "A", 1).unwrap();
    assert!(rename_section(&conn, &s.id, "", 2).is_err());
    assert!(matches!(rename_section(&conn, "nope", "X", 2), Err(TaskError::SectionNotFound)));
}

#[test]
fn rename_and_reorder_ignore_foreign_sections() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap();
    let other = create_list(&conn, "B", "", 1000).unwrap();
    let a = create_section(&conn, &l.id, "Eins", 1000).unwrap();
    let b = create_section(&conn, &l.id, "Zwei", 1000).unwrap();
    let foreign = create_section(&conn, &other.id, "Fremd", 1000).unwrap();
    rename_section(&conn, &a.id, "Alpha", 2000).unwrap();
    reorder_sections(&conn, &l.id, &[b.id.clone(), foreign.id.clone(), a.id.clone(), "unbekannt".into()], 3000).unwrap();
    assert_eq!(names(&conn, &l.id), ["Zwei", "Alpha"]);
    assert_eq!(names(&conn, &other.id), ["Fremd"]);
    assert_eq!(store::section(&conn, &foreign.id).unwrap().position, 1.0);
}

#[test]
fn deleting_a_section_returns_its_cards_to_no_section_and_keeps_a_tombstone() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap();
    let s = create_section(&conn, &l.id, "Offen", 1000).unwrap();
    conn.execute("INSERT INTO tasks (id, list_id, section_id, title, updated_at) VALUES ('t1', ?1, ?2, 'x', 0)", [&l.id, &s.id]).unwrap();
    delete_section(&conn, &s.id, 5000).unwrap();
    assert!(names(&conn, &l.id).is_empty());
    assert!(matches!(store::section(&conn, &s.id), Err(TaskError::SectionNotFound)));
    assert!(store::task(&conn, "t1").unwrap().section_id.is_none());
    let (clocks, dirty): (String, i64) = conn.query_row("SELECT clocks, dirty FROM tasks WHERE id='t1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert!(clocks.contains("section_id") && dirty == 1);
    let del: Option<i64> = conn.query_row("SELECT deleted_at FROM task_sections", [], |r| r.get(0)).unwrap();
    assert_eq!(del, Some(5));
    assert!(matches!(delete_section(&conn, &s.id, 6000), Err(TaskError::SectionNotFound)));
}

#[test]
fn deleting_a_list_tombstones_its_sections() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap();
    create_section(&conn, &l.id, "Offen", 1000).unwrap();
    delete_list(&conn, &l.id, 5000).unwrap();
    let n: i64 = conn.query_row("SELECT count(*) FROM task_sections WHERE deleted_at IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}
