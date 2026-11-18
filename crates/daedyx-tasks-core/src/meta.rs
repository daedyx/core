//! Gerätekennung und Uhrstand, in `task_meta` gespeichert. Jede lokale Änderung holt hier ihren Stempel.
use super::errors::TaskError;
use super::hlc::{Clock, Hlc};
use rusqlite::{Connection, OptionalExtension};

fn get(conn: &Connection, key: &str) -> Result<Option<String>, TaskError> {
    Ok(conn.query_row("SELECT value FROM task_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn put(conn: &Connection, key: &str, value: &str) -> Result<(), TaskError> {
    conn.execute(
        "INSERT INTO task_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Kurze, zufällige Kennung dieses Geräts (einmal erzeugt, danach stabil).
pub fn device_id(conn: &Connection) -> Result<String, TaskError> {
    if let Some(d) = get(conn, "device")? {
        return Ok(d);
    }
    let d: String = crate::ids::new_id().chars().take(8).collect();
    put(conn, "device", &d)?;
    Ok(d)
}

/// Nächster Stempel für eine lokale Änderung (streng größer als alle bisherigen und beobachteten).
pub fn next_stamp(conn: &Connection, wall_ms: u64) -> Result<String, TaskError> {
    let mut clock = Clock::new(&device_id(conn)?);
    if let Some(last) = get(conn, "clock")?.and_then(|s| Hlc::parse(&s)) {
        clock.observe(&last);
    }
    let stamp = clock.tick(wall_ms).to_string();
    put(conn, "clock", &stamp)?;
    Ok(stamp)
}

/// Fremden Stempel einrechnen, damit spätere eigene Stempel größer sind (beim Einspielen fremder Änderungen).
pub fn observe(conn: &Connection, stamp: &str) -> Result<(), TaskError> {
    let Some(remote) = Hlc::parse(stamp) else { return Ok(()) };
    let mut clock = Clock::new(&device_id(conn)?);
    if let Some(last) = get(conn, "clock")?.and_then(|s| Hlc::parse(&s)) {
        clock.observe(&last);
    }
    clock.observe(&remote);
    put(conn, "clock", &clock.last().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db::fresh;

    #[test]
    fn device_id_is_created_once_and_stays() {
        let conn = fresh();
        let a = device_id(&conn).unwrap();
        assert_eq!(a.len(), 8);
        assert_eq!(device_id(&conn).unwrap(), a);
    }

    #[test]
    fn stamps_grow_even_when_the_wall_clock_stands_or_goes_back() {
        let conn = fresh();
        let a = next_stamp(&conn, 5000).unwrap();
        let b = next_stamp(&conn, 5000).unwrap();
        let c = next_stamp(&conn, 100).unwrap();
        assert!(a < b && b < c);
        assert!(a.ends_with(&device_id(&conn).unwrap()));
    }
}
