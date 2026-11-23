//! Mehrere Erinnerungen je Aufgabe (`task_alarms`, Minuten vor der Fälligkeit). `tasks.remind_minutes` bleibt als
//! „erste Erinnerung“ erhalten, damit Desk und ältere Apps weiter funktionieren.
use super::errors::TaskError;
use super::stamp::{apply, ALARMS, TASKS};
use super::store;
use crate::ids::new_id;
use rusqlite::Connection;
use serde_json::{json, Value};

pub const MAX_ALARMS: usize = 10;
const MAX_MINUTES: i64 = 525_600;

/// Minuten vor der Fälligkeit, aufsteigend (die kleinste zuerst).
pub fn alarms(conn: &Connection, task_id: &str) -> Result<Vec<i64>, TaskError> {
    let mut st = conn.prepare(
        "SELECT minutes_before FROM task_alarms WHERE task_id = ?1 AND deleted_at IS NULL ORDER BY minutes_before",
    )?;
    Ok(st.query_map([task_id], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

/// Ersetzt die Erinnerungen der Aufgabe. Gleiche Werte bleiben (und werden bei Bedarf wiederbelebt), Überzählige werden
/// Grabstein, Neue angelegt. `remind_minutes` zeigt danach auf die kleinste Zahl (oder NULL).
pub fn set_alarms(conn: &Connection, task_id: &str, minutes: &[i64], now_ms: u64) -> Result<(), TaskError> {
    store::task(conn, task_id)?;
    let mut want: Vec<i64> = minutes.to_vec();
    want.sort_unstable();
    want.dedup();
    if want.len() > MAX_ALARMS || want.iter().any(|m| !(0..=MAX_MINUTES).contains(m)) {
        return Err(TaskError::Invalid("alarms"));
    }
    let rows: Vec<(String, i64, bool)> = conn
        .prepare("SELECT id, minutes_before, deleted_at IS NULL FROM task_alarms WHERE task_id = ?1")?
        .query_map([task_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let gone = json!((now_ms / 1000) as i64);
    for (id, m, live) in &rows {
        match (want.contains(m), *live) {
            (false, true) => apply(conn, &ALARMS, id, &[("deleted_at", gone.clone())], now_ms)?,
            (true, false) => apply(conn, &ALARMS, id, &[("deleted_at", Value::Null)], now_ms)?,
            _ => {}
        }
    }
    for m in &want {
        if rows.iter().any(|(_, rm, _)| rm == m) {
            continue;
        }
        let id = new_id();
        conn.execute(
            "INSERT INTO task_alarms (id, task_id, minutes_before, updated_at) VALUES (?1, ?2, ?3, ?4)",
            (&id, task_id, m, (now_ms / 1000) as i64),
        )?;
        apply(conn, &ALARMS, &id, &[("task_id", json!(task_id)), ("minutes_before", json!(m))], now_ms)?;
    }
    let first = want.first().map_or(Value::Null, |m| json!(m));
    if store::task(conn, task_id)?.remind_minutes != want.first().copied() {
        apply(conn, &TASKS, task_id, &[("remind_minutes", first)], now_ms)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "edit_alarms_tests.rs"]
mod tests;
