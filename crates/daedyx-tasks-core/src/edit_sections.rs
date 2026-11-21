//! Abschnitte (Board-Spalten) einer Liste: anlegen, umbenennen, sortieren, löschen (Grabstein).
use super::edit_lists::clean_name;
use super::errors::TaskError;
use super::stamp::{apply, SECTIONS, TASKS};
use super::store;
use super::types::Section;
use super::validate::check_color;
use crate::ids::new_id;
use rusqlite::Connection;
use serde_json::json;

pub fn create_section(conn: &Connection, list_id: &str, name: &str, now_ms: u64) -> Result<Section, TaskError> {
    store::list(conn, list_id)?;
    let name = clean_name(name)?;
    let id = new_id();
    let pos = store::max_position(conn, "task_sections", "list_id = ?1", &[&list_id])? + 1.0;
    conn.execute(
        "INSERT INTO task_sections (id, list_id, name, updated_at) VALUES (?1, ?2, '', ?3)",
        (&id, list_id, (now_ms / 1000) as i64),
    )?;
    apply(conn, &SECTIONS, &id, &[("list_id", json!(list_id)), ("name", json!(name)), ("position", json!(pos))], now_ms)?;
    store::section(conn, &id)
}

pub fn rename_section(conn: &Connection, id: &str, name: &str, now_ms: u64) -> Result<(), TaskError> {
    store::section(conn, id)?;
    apply(conn, &SECTIONS, id, &[("name", json!(clean_name(name)?))], now_ms)
}

/// Farbe des Abschnitts (`#RRGGBB`) oder `None` für den Standard.
pub fn set_section_color(conn: &Connection, id: &str, color: Option<&str>, now_ms: u64) -> Result<(), TaskError> {
    store::section(conn, id)?;
    apply(conn, &SECTIONS, id, &[("color", check_color(&json!(color))?)], now_ms)
}

/// Neue Reihenfolge: Position = Index in `ids`; fremde oder unbekannte IDs werden übersprungen.
pub fn reorder_sections(conn: &Connection, list_id: &str, ids: &[String], now_ms: u64) -> Result<(), TaskError> {
    store::list(conn, list_id)?;
    for (i, id) in ids.iter().enumerate() {
        if store::section(conn, id).is_ok_and(|s| s.list_id == list_id) {
            apply(conn, &SECTIONS, id, &[("position", json!(i as f64 + 1.0))], now_ms)?;
        }
    }
    Ok(())
}

/// Löscht den Abschnitt (Grabstein). Seine Karten fallen auf „Ohne Abschnitt“ zurück (gestempelt).
pub fn delete_section(conn: &Connection, id: &str, now_ms: u64) -> Result<(), TaskError> {
    store::section(conn, id)?;
    let cards: Vec<String> = conn
        .prepare("SELECT id FROM tasks WHERE section_id = ?1 AND deleted_at IS NULL")?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for c in cards {
        apply(conn, &TASKS, &c, &[("section_id", json!(null))], now_ms)?;
    }
    apply(conn, &SECTIONS, id, &[("deleted_at", json!((now_ms / 1000) as i64))], now_ms)
}

#[cfg(test)]
#[path = "edit_sections_tests.rs"]
mod tests;
