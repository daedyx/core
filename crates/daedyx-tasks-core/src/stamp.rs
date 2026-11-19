//! Änderungen schreiben und feldweise stempeln. Einzige Stelle, die Spalten der Aufgaben-Tabellen verändert,
//! damit jede lokale Änderung Stempel und `dirty` setzt (Grundlage des späteren Abgleichs, `merge.rs`).
use super::errors::TaskError;
use super::meta::next_stamp;
use rusqlite::types::Value as Sql;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Table {
    pub name: &'static str,
    /// Nur diese Felder dürfen geändert werden (Schutz vor beliebigen Spaltennamen).
    pub fields: &'static [&'static str],
}

pub const LISTS: Table =
    Table { name: "task_lists", fields: &["name", "color", "position", "owner_id", "home", "moved_at", "deleted_at"] };
pub const SECTIONS: Table =
    Table { name: "task_sections", fields: &["list_id", "name", "position", "color", "deleted_at"] };
pub const TASKS: Table = Table {
    name: "tasks",
    fields: &[
        "list_id", "parent_id", "section_id", "title", "notes", "due", "due_has_time", "priority", "done_at", "position",
        "repeat_rule", "remind_minutes", "assignee_id", "created_by", "color", "links", "deleted_at",
    ],
};
pub const COMPLETIONS: Table = Table { name: "task_completions", fields: &["task_id", "done_at", "due_was", "deleted_at"] };

fn to_sql(v: &Value) -> Sql {
    match v {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Sql::Integer(i),
            None => Sql::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

/// Setzt die Felder, stempelt jedes einzeln, setzt `updated_at` (Sekunden) und `dirty = 1`.
pub fn apply(
    conn: &Connection,
    table: &Table,
    id: &str,
    changes: &[(&str, Value)],
    now_ms: u64,
) -> Result<(), TaskError> {
    if changes.is_empty() {
        return Ok(());
    }
    if let Some((bad, _)) = changes.iter().find(|(f, _)| !table.fields.contains(f)) {
        return Err(TaskError::Invalid(if *bad == "id" { "id" } else { "field" }));
    }
    let clocks_json: String =
        conn.query_row(&format!("SELECT clocks FROM {} WHERE id = ?1", table.name), [id], |r| r.get(0))?;
    let mut clocks: BTreeMap<String, String> = serde_json::from_str(&clocks_json).unwrap_or_default();
    let stamp = next_stamp(conn, now_ms)?;
    let mut sets = Vec::new();
    let mut params: Vec<Sql> = Vec::new();
    for (f, v) in changes {
        clocks.insert(f.to_string(), stamp.clone());
        params.push(to_sql(v));
        sets.push(format!("{f} = ?{}", params.len()));
    }
    params.push(Sql::Text(serde_json::to_string(&clocks).expect("clocks")));
    sets.push(format!("clocks = ?{}", params.len()));
    params.push(Sql::Integer((now_ms / 1000) as i64));
    sets.push(format!("updated_at = ?{}, dirty = 1", params.len()));
    params.push(Sql::Text(id.to_string()));
    let sql = format!("UPDATE {} SET {} WHERE id = ?{}", table.name, sets.join(", "), params.len());
    conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db::fresh;
    use serde_json::json;

    fn seed(conn: &Connection) {
        conn.execute("INSERT INTO task_lists (id, name, updated_at) VALUES ('l1', 'A', 0)", []).unwrap();
        conn.execute("UPDATE task_lists SET dirty = 0", []).unwrap();
    }

    #[test]
    fn apply_sets_fields_stamps_each_and_marks_dirty() {
        let conn = fresh();
        seed(&conn);
        apply(&conn, &LISTS, "l1", &[("name", json!("B")), ("color", json!("blue"))], 7000).unwrap();
        let (name, clocks, dirty, upd): (String, String, i64, i64) = conn
            .query_row("SELECT name, clocks, dirty, updated_at FROM task_lists", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .unwrap();
        assert_eq!((name.as_str(), dirty, upd), ("B", 1, 7));
        let c: BTreeMap<String, String> = serde_json::from_str(&clocks).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c["name"], c["color"]);
    }

    #[test]
    fn a_later_change_gets_a_larger_stamp_for_that_field_only() {
        let conn = fresh();
        seed(&conn);
        apply(&conn, &LISTS, "l1", &[("name", json!("B")), ("color", json!("x"))], 7000).unwrap();
        apply(&conn, &LISTS, "l1", &[("name", json!("C"))], 7000).unwrap();
        let clocks: String = conn.query_row("SELECT clocks FROM task_lists", [], |r| r.get(0)).unwrap();
        let c: BTreeMap<String, String> = serde_json::from_str(&clocks).unwrap();
        assert!(c["name"] > c["color"]);
    }

    #[test]
    fn unknown_fields_and_ids_are_refused() {
        let conn = fresh();
        seed(&conn);
        assert!(apply(&conn, &LISTS, "l1", &[("clocks", json!("{}"))], 1).is_err());
        assert!(apply(&conn, &LISTS, "l1", &[("name; DROP TABLE tasks", json!("x"))], 1).is_err());
        assert!(apply(&conn, &LISTS, "nope", &[("name", json!("x"))], 1).is_err());
    }
}
