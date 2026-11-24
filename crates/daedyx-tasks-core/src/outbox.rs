//! Abgleich ohne Netz: Änderungen ausgeben (`dirty`) und fremde Änderungen feldweise einmischen.
//! Heute nur zwischen zwei Datenbanken in den Tests belegt; der spätere Server-Transport schickt dieselben
//! `Change`-Listen und ruft `mark_synced` nach der Bestätigung. Das Format kennt keine festen Feldnamen.
use super::errors::TaskError;
use super::merge::Record;
use super::stamp::{Table, ALARMS, COMPLETIONS, LISTS, SECTIONS, TASKS};
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
const TABLES: [&Table; 5] = [&LISTS, &SECTIONS, &TASKS, &ALARMS, &COMPLETIONS];

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

/// Alle noch nicht abgeglichenen Zeilen (auch Grabsteine) als Änderungen, Listen vor Abschnitten vor Aufgaben vor Erinnerungen vor Verlauf.
pub fn export_dirty(conn: &Connection) -> Result<Vec<Change>, TaskError> {
    let mut out = Vec::new();
    for table in TABLES {
        // Aufgaben: Eltern vor Kindern, beliebig tief (Ebene über die Elternkette).
        let sql = if table.name == "tasks" {
            "WITH RECURSIVE d(id, n) AS (SELECT id, 0 FROM tasks WHERE parent_id IS NULL \
             UNION ALL SELECT t.id, d.n + 1 FROM tasks t JOIN d ON t.parent_id = d.id WHERE d.n < 1000) \
             SELECT tasks.id FROM tasks JOIN d ON d.id = tasks.id WHERE tasks.dirty = 1 ORDER BY d.n, tasks.id"
                .to_string()
        } else {
            format!("SELECT id FROM {} WHERE dirty = 1 ORDER BY id", table.name)
        };
        let ids: Vec<String> = conn.prepare(&sql)?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
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

#[path = "outbox_import.rs"]
mod import;
pub use import::{import_changes, import_changes_counted};

#[cfg(test)]
#[path = "outbox_alarms_tests.rs"]
mod alarms_tests;
#[cfg(test)]
#[path = "outbox_check_tests.rs"]
mod check_tests;
#[cfg(test)]
#[path = "outbox_sections_tests.rs"]
mod sections_tests;
#[cfg(test)]
#[path = "outbox_stamp_tests.rs"]
mod stamp_tests;
#[cfg(test)]
#[path = "outbox_stand3_tests.rs"]
mod stand3_tests;
#[cfg(test)]
#[path = "outbox_tests.rs"]
mod tests;
