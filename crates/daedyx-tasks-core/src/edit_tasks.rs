//! Aufgaben anlegen, ändern, verschieben, löschen. Unteraufgaben: nur eine Ebene.
use super::errors::TaskError;
use super::stamp::{apply, TASKS};
use super::store;
use super::types::Task;
use super::validate::{check_color, check_links};
use crate::ids::new_id;
use rusqlite::Connection;
use serde_json::{json, Map, Value};

/// Prüft ein Feld einer Änderung. Gibt den bereinigten Wert zurück.
fn check(field: &str, v: &Value) -> Result<Value, TaskError> {
    let bad = |what: &'static str| Err(TaskError::Invalid(what));
    match field {
        "title" => match v.as_str().map(str::trim) {
            Some(s) if !s.is_empty() && s.chars().count() <= 500 => Ok(json!(s)),
            _ => bad("title"),
        },
        "notes" => if v.is_string() { Ok(v.clone()) } else { bad("notes") },
        "priority" => match v.as_i64() {
            Some(p) if (0..=3).contains(&p) => Ok(v.clone()),
            _ => bad("priority"),
        },
        "due_has_time" => if v.is_boolean() { Ok(json!(v.as_bool() == Some(true))) } else { bad("due_has_time") },
        "due" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s) if s.parse::<i64>().is_ok() || chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() => {
                Ok(v.clone())
            }
            _ => bad("due"),
        },
        "remind_minutes" => match v {
            Value::Null => Ok(Value::Null),
            Value::Number(n) if n.as_i64().is_some_and(|m| (0..=525_600).contains(&m)) => Ok(v.clone()),
            _ => bad("remind_minutes"),
        },
        "repeat_rule" => match v {
            Value::Null => Ok(Value::Null),
            Value::Object(_) if serde_json::from_value::<super::types::RepeatRule>(v.clone()).is_ok_and(|r| r.interval >= 1) => {
                Ok(v.clone())
            }
            _ => bad("repeat_rule"),
        },
        "done_at" => match v {
            Value::Null => Ok(Value::Null),
            Value::Number(n) if n.is_i64() => Ok(v.clone()),
            _ => bad("done_at"),
        },
        "position" => if v.is_number() { Ok(v.clone()) } else { bad("position") },
        "color" => check_color(v),
        "links" => check_links(v),
        _ => bad("field"),
    }
}

/// Prüft den Abschnitt für eine Aufgabe in `list_id`: gleiche Liste, Unteraufgaben ohne Abschnitt.
fn check_section(conn: &Connection, list_id: &str, parent: Option<&str>, section: Option<&str>) -> Result<(), TaskError> {
    let Some(s) = section else { return Ok(()) };
    let sec = store::section(conn, s)?;
    if parent.is_some() || sec.list_id != list_id {
        return Err(TaskError::Invalid("section"));
    }
    Ok(())
}

/// Neue Aufgabe am Ende der Liste (oder unter einer Oberaufgabe). `fields` darf Titel, Notiz, Fälligkeit, Priorität,
/// Wiederholung, Erinnerung, Farbe und Verweise enthalten; `title` ist Pflicht.
pub fn create_task(
    conn: &Connection,
    list_id: &str,
    parent_id: Option<&str>,
    fields: &Map<String, Value>,
    now_ms: u64,
) -> Result<Task, TaskError> {
    store::list(conn, list_id)?;
    check_parent(conn, list_id, None, parent_id)?;
    let section = match fields.get("section_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        Some(_) => return Err(TaskError::Invalid("section")),
    };
    check_section(conn, list_id, parent_id, section)?;
    let mut changes = vec![("title", check("title", fields.get("title").unwrap_or(&Value::Null))?)];
    for f in ["notes", "due", "due_has_time", "priority", "repeat_rule", "remind_minutes", "color", "links"] {
        if let Some(v) = fields.get(f) {
            changes.push((f, check(f, v)?));
        }
    }
    let id = new_id();
    let pos = match parent_id {
        Some(p) => store::max_position(conn, "tasks", "list_id = ?1 AND parent_id = ?2", &[&list_id, &p])?,
        None => store::max_position(conn, "tasks", "list_id = ?1 AND parent_id IS NULL", &[&list_id])?,
    } + 1.0;
    conn.execute(
        "INSERT INTO tasks (id, list_id, parent_id, title, updated_at) VALUES (?1, ?2, ?3, '', ?4)",
        (&id, list_id, parent_id, (now_ms / 1000) as i64),
    )?;
    changes.push(("position", json!(pos)));
    changes.push(("list_id", json!(list_id)));
    changes.push(("parent_id", json!(parent_id)));
    if section.is_some() {
        changes.push(("section_id", json!(section)));
    }
    apply(conn, &TASKS, &id, &changes, now_ms)?;
    store::task(conn, &id)
}

/// Ändert Felder (nur die in `patch` genannten). Fälligkeit mit Uhrzeit: `due` = UTC-Sekunden als Text.
pub fn update_task(conn: &Connection, id: &str, patch: &Map<String, Value>, now_ms: u64) -> Result<Task, TaskError> {
    store::task(conn, id)?;
    let mut changes = Vec::new();
    for (f, v) in patch {
        let name = match f.as_str() {
            "title" | "notes" | "due" | "due_has_time" | "priority" | "repeat_rule" | "remind_minutes" | "done_at" | "color" | "links" => f.as_str(),
            _ => return Err(TaskError::Invalid("field")),
        };
        changes.push((name, check(name, v)?));
    }
    apply(conn, &TASKS, id, &changes, now_ms)?;
    store::task(conn, id)
}

fn check_parent(conn: &Connection, list_id: &str, moving: Option<&str>, parent: Option<&str>) -> Result<(), TaskError> {
    let Some(p) = parent else { return Ok(()) };
    let parent = store::task(conn, p)?;
    if parent.list_id != list_id || moving == Some(p) {
        return Err(TaskError::Invalid("parent"));
    }
    if parent.parent_id.is_some() {
        return Err(TaskError::TooDeep);
    }
    if let Some(m) = moving {
        let kids: i64 = conn.query_row("SELECT count(*) FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL", [m], |r| r.get(0))?;
        if kids > 0 {
            return Err(TaskError::TooDeep);
        }
    }
    Ok(())
}

/// Verschiebt in eine andere Liste und/oder unter eine andere Oberaufgabe und setzt die Position (Bruchzahl).
/// Unteraufgaben folgen ihrer Oberaufgabe in die neue Liste. `section_id` ist immer der gewünschte Abschnitt
/// (`None` = „Ohne Abschnitt“) und muss zur Zielliste gehören; Unteraufgaben haben keinen.
pub fn move_task(
    conn: &Connection,
    id: &str,
    list_id: &str,
    parent_id: Option<&str>,
    section_id: Option<&str>,
    position: f64,
    now_ms: u64,
) -> Result<Task, TaskError> {
    let t = store::task(conn, id)?;
    store::list(conn, list_id)?;
    check_parent(conn, list_id, Some(id), parent_id)?;
    check_section(conn, list_id, parent_id, section_id)?;
    if !position.is_finite() {
        return Err(TaskError::Invalid("position"));
    }
    let changes = [
        ("list_id", json!(list_id)),
        ("parent_id", json!(parent_id)),
        ("section_id", json!(section_id)),
        ("position", json!(position)),
    ];
    apply(conn, &TASKS, id, &changes, now_ms)?;
    if t.list_id != list_id {
        let kids: Vec<String> = conn
            .prepare("SELECT id FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL")?
            .query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for k in kids {
            apply(conn, &TASKS, &k, &[("list_id", json!(list_id))], now_ms)?;
        }
    }
    store::task(conn, id)
}

/// Löscht die Aufgabe und ihre Unteraufgaben (Grabsteine).
pub fn delete_task(conn: &Connection, id: &str, now_ms: u64) -> Result<(), TaskError> {
    store::task(conn, id)?;
    let gone = json!((now_ms / 1000) as i64);
    let kids: Vec<String> = conn
        .prepare("SELECT id FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL")?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for k in kids {
        apply(conn, &TASKS, &k, &[("deleted_at", gone.clone())], now_ms)?;
    }
    apply(conn, &TASKS, id, &[("deleted_at", gone)], now_ms)
}

#[cfg(test)]
#[path = "edit_tasks_tests.rs"]
mod tests;
