//! Fremde Änderungen einspielen (Teil von `outbox`): Eltern vor Kindern, Stempel und Werte geprüft, eine Zeile ohne
//! Eltern wartet, statt den Stapel abzubrechen.
use super::{read, table_by_name, Change, TABLES};
use crate::errors::TaskError;
use crate::merge::{merge, Record};
use crate::meta;
use crate::stamp::Table;
use rusqlite::{Connection, ErrorCode};
use serde_json::Value;
use std::collections::BTreeMap;

fn ensure_row(conn: &Connection, table: &Table, c: &Change, now: i64) -> Result<(), TaskError> {
    let text = |f: &str| c.fields.get(f).and_then(Value::as_str).unwrap_or_default().to_string();
    match table.name {
        "task_lists" => {
            conn.execute("INSERT OR IGNORE INTO task_lists (id, name, updated_at) VALUES (?1, '', ?2)", (&c.id, now))?
        }
        "task_sections" => conn.execute(
            "INSERT OR IGNORE INTO task_sections (id, list_id, name, updated_at) VALUES (?1, ?2, '', ?3)",
            (&c.id, text("list_id"), now),
        )?,
        "tasks" => conn.execute(
            "INSERT OR IGNORE INTO tasks (id, list_id, title, updated_at) VALUES (?1, ?2, '', ?3)",
            (&c.id, text("list_id"), now),
        )?,
        "task_alarms" => conn.execute(
            "INSERT OR IGNORE INTO task_alarms (id, task_id, updated_at) VALUES (?1, ?2, ?3)",
            (&c.id, text("task_id"), now),
        )?,
        _ => conn.execute(
            "INSERT OR IGNORE INTO task_completions (id, task_id, done_at, updated_at) VALUES (?1, ?2, 0, ?3)",
            (&c.id, text("task_id"), now),
        )?,
    };
    Ok(())
}

/// Spielt fremde Änderungen ein. Je Feld gewinnt der neuere Stempel; was lokal neuer ist, bleibt `dirty`.
/// Zeilen, deren Eltern (Liste, Abschnitt, Aufgabe) fehlen, warten (siehe [`import_changes_counted`]).
pub fn import_changes(conn: &Connection, changes: &[Change], now_ms: u64) -> Result<(), TaskError> {
    import_changes_counted(conn, changes, now_ms).map(|_| ())
}

/// SQLite-Fehler „Bedingung verletzt“ (fehlende Eltern über den Fremdschlüssel).
fn is_constraint(e: &TaskError) -> bool {
    matches!(e, TaskError::Db(rusqlite::Error::SqliteFailure(f, _)) if f.code == ErrorCode::ConstraintViolation)
}

/// Wie [`import_changes`], gibt die Zahl der wartenden Zeilen zurück: Zeilen, deren Eltern hier fehlen, werden
/// übersprungen statt den ganzen Stapel abzubrechen (Review Kern-Fix N-1). Der Partner sollte sie später erneut
/// schicken (nach den Eltern); alle übrigen Änderungen kommen an.
pub fn import_changes_counted(conn: &Connection, changes: &[Change], now_ms: u64) -> Result<usize, TaskError> {
    let now = (now_ms / 1000) as i64;
    let tx = conn.unchecked_transaction()?;
    let mut sorted: Vec<&Change> = changes.iter().collect();
    // Aufgaben innerhalb des Stapels nach Tiefe der Elternkette, damit Eltern vor Kindern eingespielt werden.
    let parents: BTreeMap<&str, &str> = changes
        .iter()
        .filter(|c| c.table == "tasks")
        .filter_map(|c| Some((c.id.as_str(), c.fields.get("parent_id")?.as_str()?)))
        .collect();
    let depth = |c: &Change| {
        let (mut n, mut cur) = (0usize, c.id.as_str());
        while let Some(p) = parents.get(cur) {
            n = n.saturating_add(1);
            cur = p;
            if n > 1000 {
                break;
            }
        }
        n
    };
    let rank = |c: &Change| {
        let t = TABLES.iter().position(|t| t.name == c.table).unwrap_or(0);
        (t, if c.table == "tasks" { depth(c) } else { 0 })
    };
    sorted.sort_by_key(|c| rank(c));
    let mut waiting = 0usize;
    for c in sorted {
        let table = table_by_name(&c.table)?;
        tx.execute_batch("SAVEPOINT import_row")?;
        match import_one(&tx, table, c, now, now_ms) {
            Ok(()) => tx.execute_batch("RELEASE import_row")?,
            Err(e) if is_constraint(&e) => {
                tx.execute_batch("ROLLBACK TO import_row; RELEASE import_row")?;
                waiting = waiting.saturating_add(1);
            }
            Err(e) => return Err(e),
        }
    }
    tx.commit()?;
    Ok(waiting)
}

/// Eine fremde Zeile: Stempel und Werte prüfen, Zeile anlegen, feldweise einmischen.
fn import_one(tx: &Connection, table: &Table, c: &Change, now: i64, now_ms: u64) -> Result<(), TaskError> {
    let remote = crate::import_check::sanitized(
        table.name,
        Record { fields: c.fields.clone(), clocks: c.clocks.clone() },
        now_ms,
    );
    // Nur geprüfte Stempel bewegen die eigene Uhr (Review Kern-Fix M-1).
    for stamp in remote.clocks.values() {
        meta::observe(tx, stamp)?;
    }
    ensure_row(tx, table, c, now)?;
    let local = read(tx, table, &c.id)?.unwrap_or_default();
    let merged = merge(&local, &remote);
    let local_newer = merged != remote;
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    for (f, v) in &merged.fields {
        if !table.fields.contains(&f.as_str()) {
            continue;
        }
        params.push(match v {
            Value::Null => rusqlite::types::Value::Null,
            Value::Number(n) if n.is_i64() => rusqlite::types::Value::Integer(n.as_i64().unwrap_or(0)),
            Value::Number(n) => rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0)),
            Value::String(s) => rusqlite::types::Value::Text(s.clone()),
            other => rusqlite::types::Value::Text(other.to_string()),
        });
        sets.push(format!("{f} = ?{}", params.len()));
    }
    params.push(rusqlite::types::Value::Text(crate::stamp::clocks_text(&merged.clocks)));
    sets.push(format!("clocks = ?{}", params.len()));
    params.push(rusqlite::types::Value::Integer(now));
    sets.push(format!("updated_at = ?{}", params.len()));
    sets.push(format!("dirty = {}", local_newer as i64));
    params.push(rusqlite::types::Value::Text(c.id.clone()));
    let sql = format!("UPDATE {} SET {} WHERE id = ?{}", table.name, sets.join(", "), params.len());
    tx.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}
