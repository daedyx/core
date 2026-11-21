//! Stand 3: Farben, Verweise, Heimat und Umzugszeitpunkt wandern feldweise zwischen zwei Datenbanken.
use super::tests::{dump, fields, sync};
use super::*;
use crate::edit_lists::{create_list, set_list_home};
use crate::edit_sections::{create_section, set_section_color};
use crate::edit_tasks::{create_task, update_task};
use crate::store;
use crate::test_db::fresh;
use serde_json::json;

fn link(title: &str) -> Value {
    json!([{"app": "desk", "kind": "mail", "ref": "konto-1/msg-42", "title": title}])
}

#[test]
fn colors_links_and_home_arrive_on_the_other_side() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "Haus", "", 1000).unwrap();
    set_list_home(&a, &l.id, Some("tasks"), Some(1_800_000_000), 1100).unwrap();
    let s = create_section(&a, &l.id, "Offen", 1500).unwrap();
    set_section_color(&a, &s.id, Some("#336699"), 1600).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "Dach", "color": "#AA0000", "links": link("Angebot")})), 2000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(dump(&a), dump(&b));
    let (home, moved): (Option<String>, Option<i64>) =
        b.query_row("SELECT home, moved_at FROM task_lists", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((home.as_deref(), moved), (Some("tasks"), Some(1_800_000_000)));
    assert_eq!(store::section(&b, &s.id).unwrap().color.as_deref(), Some("#336699"));
    let task = store::task(&b, &t.id).unwrap();
    assert_eq!(task.color.as_deref(), Some("#AA0000"));
    assert_eq!(task.links.len(), 1);
    assert_eq!(task.links[0].title, "Angebot");
    assert!(export_dirty(&a).unwrap().is_empty() && export_dirty(&b).unwrap().is_empty());
}

#[test]
fn different_new_fields_changed_at_once_both_survive() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    let t = create_task(&a, &l.id, None, &fields(json!({"title": "x"})), 1000).unwrap();
    sync(&a, &b, 2000);
    update_task(&a, &t.id, &fields(json!({"color": "#112233"})), 5000).unwrap();
    update_task(&b, &t.id, &fields(json!({"links": link("Betreff")})), 6000).unwrap();
    sync(&a, &b, 7000);
    assert_eq!(dump(&a), dump(&b));
    let m = store::task(&a, &t.id).unwrap();
    assert_eq!((m.color.as_deref(), m.links.len()), (Some("#112233"), 1));
}

#[test]
fn the_same_color_set_on_both_sides_goes_to_the_newer_stamp_and_null_resets() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    let s = create_section(&a, &l.id, "S", 1000).unwrap();
    sync(&a, &b, 2000);
    set_section_color(&a, &s.id, Some("#111111"), 5000).unwrap();
    set_section_color(&b, &s.id, Some("#222222"), 6000).unwrap();
    sync(&a, &b, 7000);
    assert_eq!(store::section(&a, &s.id).unwrap().color.as_deref(), Some("#222222"));
    set_section_color(&a, &s.id, None, 8000).unwrap();
    sync(&a, &b, 9000);
    assert_eq!(store::section(&b, &s.id).unwrap().color, None);
    assert_eq!(dump(&a), dump(&b));
}

#[test]
fn home_and_moved_at_are_stamped_separately_and_a_concurrent_move_goes_to_the_newer() {
    let (a, b) = (fresh(), fresh());
    let l = create_list(&a, "L", "", 1000).unwrap();
    sync(&a, &b, 2000);
    set_list_home(&a, &l.id, Some("tasks"), Some(100), 5000).unwrap();
    set_list_home(&b, &l.id, Some("desk"), Some(200), 6000).unwrap();
    sync(&a, &b, 7000);
    assert_eq!(dump(&a), dump(&b));
    let (home, moved): (Option<String>, Option<i64>) =
        a.query_row("SELECT home, moved_at FROM task_lists", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((home.as_deref(), moved), (Some("desk"), Some(200)));
}

#[test]
fn a_database_importing_stand_3_fields_from_a_peer_keeps_unknown_nothing_lost() {
    // Ein Gegenüber im Stand 3 schickt Felder; die Spalten sind in jeder migrierten Datenbank da.
    let b = fresh();
    let stamp = "000000000001-0000-z".to_string();
    let fields_of = |pairs: &[(&str, Value)]| pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<BTreeMap<_, _>>();
    let clocks = |keys: &[&str]| keys.iter().map(|k| (k.to_string(), stamp.clone())).collect::<BTreeMap<_, _>>();
    let list = Change { table: "task_lists".into(), id: "l".into(), fields: fields_of(&[("name", json!("L")), ("home", json!("tasks"))]), clocks: clocks(&["name", "home"]) };
    import_changes(&b, &[list], 1).unwrap();
    let home: Option<String> = b.query_row("SELECT home FROM task_lists WHERE id = 'l'", [], |r| r.get(0)).unwrap();
    assert_eq!(home.as_deref(), Some("tasks"));
}
