use super::tests::{dump, fields, sync};
use super::*;
use crate::edit_lists::{create_list, delete_list};
use crate::edit_sections::{create_section, delete_section, rename_section};
use crate::edit_tasks::{create_task, move_task};
use crate::store;
use crate::test_db::fresh;
use serde_json::json;

#[test]
fn sections_and_their_cards_arrive_in_the_right_order() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haus", "", 1000).unwrap();
    let s = create_section(&a, &l.id, "Offen", 1500).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "Dach", "section_id": s.id})), 2000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    assert_eq!(store::sections(&b, &l.id).unwrap().len(), 1);
    assert_eq!(store::task(&b, &t.id).unwrap().section_id.as_deref(), Some(s.id.as_str()));
    assert!(export_dirty(&a).unwrap().is_empty() && export_dirty(&b).unwrap().is_empty());
}

#[test]
fn renaming_on_both_sides_at_once_keeps_the_newer_name() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haus", "", 1000).unwrap();
    let s = create_section(&a, &l.id, "Offen", 1500).unwrap();
    sync(&a, &b, 5000);
    rename_section(&a, &s.id, "Von A", 6000).unwrap();
    rename_section(&b, &s.id, "Von B", 7000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(store::section(&a, &s.id).unwrap().name, "Von B");
    assert_eq!(dump(&a), dump(&b));
}

#[test]
fn deleting_a_section_against_moving_a_card_into_it_ends_without_a_dangling_section() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haus", "", 1000).unwrap();
    let s = create_section(&a, &l.id, "Offen", 1500).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "Dach"})), 2000).unwrap();
    sync(&a, &b, 5000);
    delete_section(&a, &s.id, 6000).unwrap();
    move_task(&b, &t.id, &l.id, None, Some(&s.id), 1.0, 7000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    assert!(store::sections(&a, &l.id).unwrap().is_empty());
    let card = store::task(&a, &t.id).unwrap();
    assert_eq!(card.section_id.as_deref(), Some(s.id.as_str()), "gleichstand gewinnt der neuere Stempel");
    assert!(store::section(&a, &s.id).is_err());
}

#[test]
fn deleting_a_list_reaches_the_other_side_with_its_sections() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haus", "", 1000).unwrap();
    create_section(&a, &l.id, "Offen", 1500).unwrap();
    sync(&a, &b, 5000);
    delete_list(&a, &l.id, 6000).unwrap();
    sync(&a, &b, 9000);
    let n: i64 = b.query_row("SELECT count(*) FROM task_sections WHERE deleted_at IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    assert_eq!(dump(&a), dump(&b));
}
