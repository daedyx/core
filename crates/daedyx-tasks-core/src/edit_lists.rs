//! Listen anlegen, ändern, löschen (Grabstein), sortieren.
use super::errors::TaskError;
use super::stamp::{apply, LISTS, SECTIONS, TASKS};
use super::store;
use super::types::TaskList;
use super::validate::check_home;
use crate::ids::new_id;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn clean_name(name: &str) -> Result<String, TaskError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 200 {
        return Err(TaskError::Invalid("name"));
    }
    Ok(n.to_string())
}

pub fn create_list(conn: &Connection, name: &str, color: &str, now_ms: u64) -> Result<TaskList, TaskError> {
    let name = clean_name(name)?;
    let id = new_id();
    let pos = store::max_position(conn, "task_lists", "1 = 1", &[])? + 1.0;
    conn.execute(
        "INSERT INTO task_lists (id, name, updated_at) VALUES (?1, ?2, ?3)",
        (&id, &name, (now_ms / 1000) as i64),
    )?;
    apply(conn, &LISTS, &id, &[("name", json!(name)), ("color", json!(color)), ("position", json!(pos))], now_ms)?;
    store::list(conn, &id)
}

pub fn rename_list(conn: &Connection, id: &str, name: &str, now_ms: u64) -> Result<(), TaskError> {
    store::list(conn, id)?;
    apply(conn, &LISTS, id, &[("name", json!(clean_name(name)?))], now_ms)
}

pub fn set_list_color(conn: &Connection, id: &str, color: &str, now_ms: u64) -> Result<(), TaskError> {
    store::list(conn, id)?;
    apply(conn, &LISTS, id, &[("color", json!(color))], now_ms)
}

/// Setzt Heimat-App (`desk`/`tasks`/None) und Umzugszeitpunkt (UTC-Sekunden) der Liste; beide Felder einzeln gestempelt.
pub fn set_list_home(
    conn: &Connection,
    id: &str,
    home: Option<&str>,
    moved_at: Option<i64>,
    now_ms: u64,
) -> Result<(), TaskError> {
    store::list(conn, id)?;
    let home = check_home(home)?;
    apply(conn, &LISTS, id, &[("home", home), ("moved_at", json!(moved_at))], now_ms)
}

/// Neue Reihenfolge: Position = Index in `ids` (unbekannte IDs werden übersprungen).
pub fn reorder_lists(conn: &Connection, ids: &[String], now_ms: u64) -> Result<(), TaskError> {
    for (i, id) in ids.iter().enumerate() {
        if store::list(conn, id).is_ok() {
            apply(conn, &LISTS, id, &[("position", json!(i as f64 + 1.0))], now_ms)?;
        }
    }
    Ok(())
}

/// Löscht die Liste samt ihren Aufgaben (alle als Grabstein, damit der Abgleich das Löschen kennt).
pub fn delete_list(conn: &Connection, id: &str, now_ms: u64) -> Result<(), TaskError> {
    store::list(conn, id)?;
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM tasks WHERE list_id = ?1 AND deleted_at IS NULL")?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let gone: Value = json!((now_ms / 1000) as i64);
    for t in ids {
        apply(conn, &TASKS, &t, &[("deleted_at", gone.clone())], now_ms)?;
    }
    for s in store::sections(conn, id)? {
        apply(conn, &SECTIONS, &s.id, &[("deleted_at", gone.clone())], now_ms)?;
    }
    apply(conn, &LISTS, id, &[("deleted_at", gone)], now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db::fresh;

    #[test]
    fn creates_lists_in_order_and_trims_names() {
        let conn = fresh();
        let a = create_list(&conn, "  Haushalt ", "", 1000).unwrap();
        let b = create_list(&conn, "Arbeit", "blue", 2000).unwrap();
        assert_eq!(a.name, "Haushalt");
        assert!(a.position < b.position);
        let names: Vec<_> = store::lists(&conn).unwrap().into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["Haushalt", "Arbeit"]);
        assert!(create_list(&conn, "   ", "", 1).is_err());
    }

    #[test]
    fn rename_color_and_reorder() {
        let conn = fresh();
        let a = create_list(&conn, "A", "", 1000).unwrap();
        let b = create_list(&conn, "B", "", 1000).unwrap();
        rename_list(&conn, &a.id, "Alpha", 2000).unwrap();
        set_list_color(&conn, &a.id, "red", 2000).unwrap();
        reorder_lists(&conn, &[b.id.clone(), a.id.clone(), "unbekannt".into()], 3000).unwrap();
        let ls = store::lists(&conn).unwrap();
        assert_eq!((ls[0].name.as_str(), ls[1].name.as_str(), ls[1].color.as_str()), ("B", "Alpha", "red"));
    }

    #[test]
    fn deleting_a_list_keeps_tombstones_for_the_list_and_its_tasks() {
        let conn = fresh();
        let l = create_list(&conn, "A", "", 1000).unwrap();
        conn.execute("INSERT INTO tasks (id, list_id, title, updated_at) VALUES ('t1', ?1, 'x', 0)", [&l.id]).unwrap();
        delete_list(&conn, &l.id, 5000).unwrap();
        assert!(store::lists(&conn).unwrap().is_empty());
        assert!(matches!(store::list(&conn, &l.id), Err(TaskError::ListNotFound)));
        let (n, dirty): (i64, i64) = conn
            .query_row("SELECT count(*), min(dirty) FROM tasks WHERE deleted_at IS NOT NULL", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((n, dirty), (1, 1));
        assert!(matches!(delete_list(&conn, &l.id, 6000), Err(TaskError::ListNotFound)));
    }
}
