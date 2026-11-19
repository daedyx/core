//! Baum der Unteraufgaben: Tiefe, Höhe, Nachfahren. Die Spalte `parent_id` kann beliebig tief; wie tief erlaubt ist,
//! bestimmt der Aufrufer (`max_depth`): Desk bleibt bei einer Ebene (`DESK_MAX_DEPTH`), Tasks darf mehr.
use super::errors::TaskError;
use rusqlite::Connection;

/// Desk zeigt und erlaubt nur eine Ebene Unteraufgaben.
pub const DESK_MAX_DEPTH: u32 = 1;

/// Ebene der Aufgabe: 0 = oberste, 1 = Unteraufgabe usw.
pub fn depth_of(conn: &Connection, id: &str) -> Result<u32, TaskError> {
    let n: i64 = conn.query_row(
        "WITH RECURSIVE up(id, n) AS (SELECT ?1, 0 UNION ALL SELECT t.parent_id, up.n + 1 FROM tasks t JOIN up ON t.id = up.id \
         WHERE t.parent_id IS NOT NULL AND up.n < 1000) SELECT max(n) FROM up",
        [id],
        |r| r.get(0),
    )?;
    Ok(n as u32)
}

/// Wie viele Ebenen unter der Aufgabe liegen (ohne Gelöschte); 0 = keine Unteraufgaben.
pub fn height_of(conn: &Connection, id: &str) -> Result<u32, TaskError> {
    let n: i64 = conn.query_row(
        "WITH RECURSIVE d(id, n) AS (SELECT ?1, 0 UNION ALL SELECT t.id, d.n + 1 FROM tasks t JOIN d ON t.parent_id = d.id \
         WHERE t.deleted_at IS NULL AND d.n < 1000) SELECT max(n) FROM d",
        [id],
        |r| r.get(0),
    )?;
    Ok(n as u32)
}

/// Alle lebenden Nachfahren (Kinder, Enkel, …) ohne die Aufgabe selbst.
pub fn descendants(conn: &Connection, id: &str) -> Result<Vec<String>, TaskError> {
    let mut st = conn.prepare(
        "WITH RECURSIVE d(id, n) AS (SELECT ?1, 0 UNION ALL SELECT t.id, d.n + 1 FROM tasks t JOIN d ON t.parent_id = d.id \
         WHERE t.deleted_at IS NULL AND d.n < 1000) SELECT id FROM d WHERE n > 0 ORDER BY n, id",
    )?;
    Ok(st.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
