//! Gemeinsame Testhilfe: frische In-Memory-Datenbank mit dem Kern-Schema.
use rusqlite::Connection;

pub fn fresh() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    crate::migrate(&conn).unwrap();
    conn
}
