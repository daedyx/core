//! Abgleich ohne Netz: Änderungen ausgeben (`dirty`) und fremde Änderungen feldweise einmischen.
//! Heute nur zwischen zwei Datenbanken in den Tests belegt; der spätere Server-Transport schickt dieselben
//! `Change`-Listen und ruft `mark_synced` nach der Bestätigung. Das Format kennt keine festen Feldnamen.
use super::errors::TaskError;
use super::merge::{merge, Record};
use super::meta;
use super::stamp::{Table, COMPLETIONS, LISTS, SECTIONS, TASKS};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub table: String,
    pub id: String,
    pub fields: BTreeMap<String, Value>,
    pub clocks: BTreeMap<String, String>,
}

/// Reihenfolge so, dass Fremdschlüssel beim Einspielen schon da sind.
const TABLES: [&Table; 4] = [&LISTS, &SECTIONS, &TASKS, &COMPLETIONS];

fn table_by_name(name: &str) -> Result<&'static Table, TaskError> {
    TABLES.iter().copied().find(|t| t.name == name).ok_or(TaskError::Invalid("table"))
}

fn json_of(v: ValueRef) -> Value {
    match v {
        ValueRef::Null | ValueRef::Blob(_) => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
    }
}

fn read(conn: &Connection, table: &Table, id: &str) -> Result<Option<Record>, TaskError> {
    let cols = table.fields.join(", ");
    let sql = format!("SELECT {cols}, clocks FROM {} WHERE id = ?1", table.name);
    Ok(conn
        .query_row(&sql, [id], |r| {
            let mut rec = Record::default();
            let clocks: String = r.get(table.fields.len())?;
            let stamps: BTreeMap<String, String> = serde_json::from_str(&clocks).unwrap_or_default();
            for (i, f) in table.fields.iter().enumerate() {
                if let Some(c) = stamps.get(*f) {
                    rec.set(f, json_of(r.get_ref(i)?), c);
                }
            }
            Ok(rec)
        })
        .optional()?)
}

/// Alle noch nicht abgeglichenen Zeilen (auch Grabsteine) als Änderungen, Listen vor Abschnitten vor Aufgaben vor Verlauf.
pub fn export_dirty(conn: &Connection) -> Result<Vec<Change>, TaskError> {
    let mut out = Vec::new();
    for table in TABLES {
        let order = if table.name == "tasks" { "parent_id IS NOT NULL, id" } else { "id" };
        let ids: Vec<String> = conn
            .prepare(&format!("SELECT id FROM {} WHERE dirty = 1 ORDER BY {order}", table.name))?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for id in ids {
            if let Some(rec) = read(conn, table, &id)? {
                out.push(Change { table: table.name.into(), id, fields: rec.fields, clocks: rec.clocks });
            }
        }
    }
    Ok(out)
}

/// Nach der Bestätigung des Gegenübers: `dirty` löschen, aber nur, wo sich seit dem Ausgeben nichts geändert hat.
pub fn mark_synced(conn: &Connection, sent: &[Change]) -> Result<(), TaskError> {
    for c in sent {
        let table = table_by_name(&c.table)?;
        let now = read(conn, table, &c.id)?;
        if now.is_some_and(|r| r.clocks == c.clocks) {
            conn.execute(&format!("UPDATE {} SET dirty = 0 WHERE id = ?1", table.name), [&c.id])?;
        }
    }
    Ok(())
}

fn ensure_row(conn: &Connection, table: &Table, c: &Change, now: i64) -> Result<(), TaskError> {
    let text = |f: &str| c.fields.get(f).and_then(Value::as_str).unwrap_or_default().to_string();
    match table.name {
        "task_lists" => conn.execute("INSERT OR IGNORE INTO task_lists (id, name, updated_at) VALUES (?1, '', ?2)", (&c.id, now))?,
        "task_sections" => conn.execute(
            "INSERT OR IGNORE INTO task_sections (id, list_id, name, updated_at) VALUES (?1, ?2, '', ?3)",
            (&c.id, text("list_id"), now),
        )?,
        "tasks" => conn.execute(
            "INSERT OR IGNORE INTO tasks (id, list_id, title, updated_at) VALUES (?1, ?2, '', ?3)",
            (&c.id, text("list_id"), now),
        )?,
        _ => conn.execute(
            "INSERT OR IGNORE INTO task_completions (id, task_id, done_at, updated_at) VALUES (?1, ?2, 0, ?3)",
            (&c.id, text("task_id"), now),
        )?,
    };
    Ok(())
}

/// Spielt fremde Änderungen ein. Je Feld gewinnt der neuere Stempel; was lokal neuer ist, bleibt `dirty`.
pub fn import_changes(conn: &Connection, changes: &[Change], now_ms: u64) -> Result<(), TaskError> {
    let now = (now_ms / 1000) as i64;
    let tx = conn.unchecked_transaction()?;
    let mut sorted: Vec<&Change> = changes.iter().collect();
    let rank = |c: &Change| {
        let t = TABLES.iter().position(|t| t.name == c.table).unwrap_or(0);
        (t, c.table == "tasks" && c.fields.get("parent_id").is_some_and(|p| !p.is_null()))
    };
    sorted.sort_by_key(|c| rank(c));
    for c in sorted {
        let table = table_by_name(&c.table)?;
        for stamp in c.clocks.values() {
            meta::observe(&tx, stamp)?;
        }
        ensure_row(&tx, table, c, now)?;
        let remote = Record { fields: c.fields.clone(), clocks: c.clocks.clone() };
        let local = read(&tx, table, &c.id)?.unwrap_or_default();
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
        params.push(rusqlite::types::Value::Text(serde_json::to_string(&merged.clocks).expect("clocks")));
        sets.push(format!("clocks = ?{}", params.len()));
        params.push(rusqlite::types::Value::Integer(now));
        sets.push(format!("updated_at = ?{}", params.len()));
        sets.push(format!("dirty = {}", local_newer as i64));
        params.push(rusqlite::types::Value::Text(c.id.clone()));
        let sql = format!("UPDATE {} SET {} WHERE id = ?{}", table.name, sets.join(", "), params.len());
        tx.execute(&sql, rusqlite::params_from_iter(params))?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[path = "outbox_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "outbox_sections_tests.rs"]
mod sections_tests;
#[cfg(test)]
#[path = "outbox_stand3_tests.rs"]
mod stand3_tests;
