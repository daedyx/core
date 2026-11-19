//! Migrationsbrücke: eigene Zählung `task_meta.schema_version`, unabhängig von `user_version` der App.
//! Stand 1 = Desk-Migration 7, Stand 2 = Migration 8. Der Kern läuft in einem `SAVEPOINT`, damit er
//! auch innerhalb der Transaktion einer App-Migration (Desk) und für sich allein (Tasks) arbeiten kann.
use crate::schema::{STAND_1, STAND_2};
use rusqlite::{Connection, OptionalExtension};

/// Neuester Stand, den dieser Kern anlegt und versteht.
pub const SCHEMA_VERSION: i64 = 2;

const SAVEPOINT: &str = "daedyx_tasks_core";

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("Aufgaben-Datenbank ist neuer (Stand {found}) als dieser Kern versteht (Stand {supported})")]
    TooNew { found: i64, supported: i64 },
    #[error("Gespeicherter Schema-Stand der Aufgaben ist unlesbar: {0}")]
    BadVersion(String),
    #[error("Gespeicherter Stand {stored} passt nicht zu den vorhandenen Aufgaben-Tabellen")]
    Inconsistent { stored: i64 },
    #[error("Datenbank: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Legt das Aufgaben-Schema an oder hebt es auf den neuesten Stand. Bestehende Zeilen bleiben unverändert.
/// Fehler lassen die Datenbank im Ausgangszustand.
pub fn migrate(conn: &Connection) -> Result<(), MigrateError> {
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))?;
    match run(conn) {
        Ok(()) => {
            conn.execute_batch(&format!("RELEASE {SAVEPOINT}"))?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch(&format!("ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"));
            Err(e)
        }
    }
}

fn has_table(conn: &Connection, name: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |_| Ok(())).optional().map(|r| r.is_some())
}

/// 0 = noch nichts da. Ohne gespeicherten Stand (Desk-Altbestand) entscheidet der Tabellenbestand.
fn detect(conn: &Connection) -> Result<i64, MigrateError> {
    if has_table(conn, "task_meta")? {
        let stored: Option<String> =
            conn.query_row("SELECT value FROM task_meta WHERE key = 'schema_version'", [], |r| r.get(0)).optional()?;
        if let Some(s) = stored {
            let v: i64 = s.parse().map_err(|_| MigrateError::BadVersion(s.clone()))?;
            if v < 1 {
                return Err(MigrateError::BadVersion(s));
            }
            // Der gespeicherte Stand muss zum Tabellenbestand passen, sonst nicht still bestätigen.
            if v >= 2 && !has_table(conn, "task_sections")? {
                return Err(MigrateError::Inconsistent { stored: v });
            }
            return Ok(v);
        }
    }
    Ok(if !has_table(conn, "task_lists")? {
        0
    } else if has_table(conn, "task_sections")? {
        2
    } else {
        1
    })
}

fn run(conn: &Connection) -> Result<(), MigrateError> {
    let found = detect(conn)?;
    if found > SCHEMA_VERSION {
        return Err(MigrateError::TooNew { found, supported: SCHEMA_VERSION });
    }
    if found < 1 {
        conn.execute_batch(STAND_1)?;
    }
    if found < 2 {
        conn.execute_batch(STAND_2)?;
    }
    conn.execute(
        "INSERT INTO task_meta (key, value) VALUES ('schema_version', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
