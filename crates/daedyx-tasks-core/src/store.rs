//! Lesen: Listen und Aufgaben (ohne Gelöschte).
use super::errors::TaskError;
use super::types::{RepeatRule, Section, Task, TaskList};
use rusqlite::{Connection, OptionalExtension, Row};

pub const TASK_COLS: &str = "id, list_id, parent_id, title, notes, due, due_has_time, priority, done_at, position, \
     repeat_rule, remind_minutes, assignee_id, created_by, section_id, color, links";

pub fn task_from_row(r: &Row) -> rusqlite::Result<Task> {
    let repeat: Option<String> = r.get(10)?;
    let links: Option<String> = r.get(16)?;
    Ok(Task {
        id: r.get(0)?,
        list_id: r.get(1)?,
        parent_id: r.get(2)?,
        title: r.get(3)?,
        notes: r.get(4)?,
        due: r.get(5)?,
        due_has_time: r.get::<_, i64>(6)? != 0,
        priority: r.get::<_, i64>(7)? as u8,
        done_at: r.get(8)?,
        position: r.get(9)?,
        repeat: repeat.and_then(|s| serde_json::from_str::<RepeatRule>(&s).ok()),
        remind_minutes: r.get(11)?,
        assignee_id: r.get(12)?,
        created_by: r.get(13)?,
        section_id: r.get(14)?,
        color: r.get(15)?,
        links: links.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
    })
}

fn list_from_row(r: &Row) -> rusqlite::Result<TaskList> {
    Ok(TaskList {
        id: r.get(0)?,
        name: r.get(1)?,
        color: r.get(2)?,
        position: r.get(3)?,
        owner_id: r.get(4)?,
        home: r.get(5)?,
        moved_at: r.get(6)?,
    })
}

const LIST_COLS: &str = "id, name, color, position, owner_id, home, moved_at";

pub fn lists(conn: &Connection) -> Result<Vec<TaskList>, TaskError> {
    let mut st = conn.prepare(&format!(
        "SELECT {LIST_COLS} FROM task_lists WHERE deleted_at IS NULL ORDER BY position, name COLLATE NOCASE"
    ))?;
    let rows = st.query_map([], list_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list(conn: &Connection, id: &str) -> Result<TaskList, TaskError> {
    conn.query_row(
        &format!("SELECT {LIST_COLS} FROM task_lists WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        list_from_row,
    )
    .optional()?
    .ok_or(TaskError::ListNotFound)
}

pub fn task(conn: &Connection, id: &str) -> Result<Task, TaskError> {
    conn.query_row(
        &format!("SELECT {TASK_COLS} FROM tasks WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        task_from_row,
    )
    .optional()?
    .ok_or(TaskError::TaskNotFound)
}

/// Alle Aufgaben einer Liste, Unteraufgaben direkt nach ihrer Oberaufgabe (Handreihenfolge).
pub fn tasks_in_list(conn: &Connection, list_id: &str) -> Result<Vec<Task>, TaskError> {
    let mut st = conn.prepare(&format!(
        "SELECT {TASK_COLS} FROM tasks WHERE list_id = ?1 AND deleted_at IS NULL ORDER BY position, id"
    ))?;
    let all = st.query_map([list_id], task_from_row)?.collect::<Result<Vec<_>, _>>()?;
    let mut by_parent: std::collections::HashMap<Option<&str>, Vec<&Task>> = std::collections::HashMap::new();
    for t in &all {
        by_parent.entry(t.parent_id.as_deref()).or_default().push(t);
    }
    // Tiefensuche in Handreihenfolge: jede Aufgabe direkt vor ihren Unteraufgaben.
    let mut out = Vec::with_capacity(all.len());
    let mut stack: Vec<&Task> = by_parent.get(&None).map(|v| v.iter().rev().copied().collect()).unwrap_or_default();
    while let Some(t) = stack.pop() {
        out.push(t.clone());
        if let Some(kids) = by_parent.get(&Some(t.id.as_str())) {
            stack.extend(kids.iter().rev().copied());
        }
    }
    Ok(out)
}

pub fn max_position(conn: &Connection, table: &str, scope_sql: &str, scope: &[&dyn rusqlite::ToSql]) -> Result<f64, TaskError> {
    let sql = format!("SELECT COALESCE(MAX(position), 0) FROM {table} WHERE deleted_at IS NULL AND {scope_sql}");
    Ok(conn.query_row(&sql, scope, |r| r.get(0))?)
}

fn section_from_row(r: &Row) -> rusqlite::Result<Section> {
    Ok(Section { id: r.get(0)?, list_id: r.get(1)?, name: r.get(2)?, position: r.get(3)?, color: r.get(4)? })
}

const SECTION_COLS: &str = "id, list_id, name, position, color";

/// Abschnitte einer Liste in Handreihenfolge (ohne Gelöschte).
pub fn sections(conn: &Connection, list_id: &str) -> Result<Vec<Section>, TaskError> {
    let mut st = conn.prepare(&format!(
        "SELECT {SECTION_COLS} FROM task_sections WHERE list_id = ?1 AND deleted_at IS NULL ORDER BY position, id"
    ))?;
    let rows = st.query_map([list_id], section_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn section(conn: &Connection, id: &str) -> Result<Section, TaskError> {
    conn.query_row(
        &format!("SELECT {SECTION_COLS} FROM task_sections WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        section_from_row,
    )
    .optional()?
    .ok_or(TaskError::SectionNotFound)
}
