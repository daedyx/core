//! Lesen: Listen und Aufgaben (ohne Gelöschte).
use super::errors::TaskError;
use super::types::{RepeatRule, Section, Task, TaskList};
use rusqlite::{Connection, OptionalExtension, Row};

pub const TASK_COLS: &str = "id, list_id, parent_id, title, notes, due, due_has_time, priority, done_at, position, \
     repeat_rule, remind_minutes, assignee_id, created_by, section_id";

pub fn task_from_row(r: &Row) -> rusqlite::Result<Task> {
    let repeat: Option<String> = r.get(10)?;
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
    })
}

fn list_from_row(r: &Row) -> rusqlite::Result<TaskList> {
    Ok(TaskList { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, position: r.get(3)?, owner_id: r.get(4)? })
}

const LIST_COLS: &str = "id, name, color, position, owner_id";

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
    let mut out = Vec::with_capacity(all.len());
    for top in all.iter().filter(|t| t.parent_id.is_none()) {
        out.push(top.clone());
        out.extend(all.iter().filter(|t| t.parent_id.as_deref() == Some(top.id.as_str())).cloned());
    }
    Ok(out)
}

pub fn max_position(conn: &Connection, table: &str, scope_sql: &str, scope: &[&dyn rusqlite::ToSql]) -> Result<f64, TaskError> {
    let sql = format!("SELECT COALESCE(MAX(position), 0) FROM {table} WHERE deleted_at IS NULL AND {scope_sql}");
    Ok(conn.query_row(&sql, scope, |r| r.get(0))?)
}

fn section_from_row(r: &Row) -> rusqlite::Result<Section> {
    Ok(Section { id: r.get(0)?, list_id: r.get(1)?, name: r.get(2)?, position: r.get(3)? })
}

const SECTION_COLS: &str = "id, list_id, name, position";

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
