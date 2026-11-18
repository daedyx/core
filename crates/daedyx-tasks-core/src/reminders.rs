//! Erinnerungen zur Fälligkeit. Der Kern rechnet aus, was fällig ist, und meldet es als Ereignis
//! `tasks://reminder`; die Benachrichtigung zeigt die Oberfläche (Texte in `de.json`).
//! Ohne Uhrzeit gilt 9:00 Ortszeit des Fälligkeitstages als Bezugspunkt.
use super::errors::TaskError;
use super::smart::due_day;
use super::store::{task_from_row, TASK_COLS};
use super::types::Task;
use crate::zone::zone;
use chrono::TimeZone;
use rusqlite::Connection;
use serde::Serialize;

/// Nach einem Neustart werden verpasste Erinnerungen dieser letzten Zeitspanne nachgeholt.
pub const CATCH_UP_SECS: i64 = 3600;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReminder {
    pub task_id: String,
    pub title: String,
    pub fire_at: i64,
    pub minutes_before: i64,
    pub due_has_time: bool,
}

fn base_secs(t: &Task, tz: chrono_tz::Tz) -> Option<i64> {
    if t.due_has_time {
        return t.due.as_deref()?.parse().ok();
    }
    let day = due_day(t, tz)?;
    let local = day.and_hms_opt(9, 0, 0)?;
    tz.from_local_datetime(&local).earliest().map(|d| d.timestamp())
}

/// Offene Aufgaben mit Erinnerung, deren Zeitpunkt in `[from, to)` liegt und die noch nicht gemeldet wurden.
pub fn pending(conn: &Connection, from: i64, to: i64, tz_name: &str) -> Result<Vec<DueReminder>, TaskError> {
    let tz = zone(tz_name);
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE deleted_at IS NULL AND done_at IS NULL AND due IS NOT NULL \
         AND remind_minutes IS NOT NULL AND list_id IN (SELECT id FROM task_lists WHERE deleted_at IS NULL)"
    );
    let tasks = conn.prepare(&sql)?.query_map([], task_from_row)?.collect::<Result<Vec<_>, _>>()?;
    let mut fired = conn.prepare("SELECT 1 FROM task_alarms_fired WHERE task_id = ?1 AND fire_at = ?2")?;
    let mut out = Vec::new();
    for t in tasks {
        let (Some(base), Some(min)) = (base_secs(&t, tz), t.remind_minutes) else { continue };
        let fire_at = base - min * 60;
        if fire_at < from || fire_at >= to || fired.exists((&t.id, fire_at))? {
            continue;
        }
        out.push(DueReminder { task_id: t.id, title: t.title, fire_at, minutes_before: min, due_has_time: t.due_has_time });
    }
    out.sort_by_key(|r| r.fire_at);
    Ok(out)
}

pub fn mark_fired(conn: &Connection, r: &DueReminder) -> Result<(), TaskError> {
    conn.execute("INSERT OR IGNORE INTO task_alarms_fired (task_id, fire_at) VALUES (?1, ?2)", (&r.task_id, r.fire_at))?;
    Ok(())
}

/// Ein Durchgang: Fälliges der letzten `CATCH_UP_SECS` melden (über `fire`) und merken.
pub fn tick(conn: &Connection, now: i64, tz_name: &str, mut fire: impl FnMut(&DueReminder)) -> Result<usize, TaskError> {
    conn.execute("DELETE FROM task_alarms_fired WHERE fire_at < ?1", [now - 2 * 86_400])?;
    let due = pending(conn, now - CATCH_UP_SECS, now + 1, tz_name)?;
    for r in &due {
        fire(r);
        mark_fired(conn, r)?;
    }
    Ok(due.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit_lists::create_list;
    use crate::edit_tasks::create_task;
    use crate::test_db::fresh;
    use serde_json::{json, Map, Value};

    fn fields(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    // 2026-11-19 09:00 Berlin = 08:00 UTC
    const NINE: i64 = 1_795_075_200;

    fn setup() -> Connection {
        let conn = fresh();
        let l = create_list(&conn, "A", "", 1000).unwrap().id;
        let mk = |f: Value| create_task(&conn, &l, None, &fields(f), 1000).unwrap();
        mk(json!({"title": "ganztägig", "due": "2026-11-19", "remind_minutes": 30}));
        mk(json!({"title": "mit Uhrzeit", "due": "1795096800", "due_has_time": true, "remind_minutes": 60}));
        mk(json!({"title": "ohne Erinnerung", "due": "2026-11-19"}));
        conn
    }

    #[test]
    fn date_only_reminds_relative_to_nine_local_and_timed_relative_to_the_time() {
        let conn = setup();
        let all = pending(&conn, 0, i64::MAX, "Europe/Berlin").unwrap();
        let by = |t: &str| all.iter().find(|r| r.title == t).unwrap().fire_at;
        assert_eq!(by("ganztägig"), NINE - 1800);
        assert_eq!(by("mit Uhrzeit"), 1_795_096_800 - 3600);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn window_filters_and_tick_marks_as_fired_once() {
        let conn = setup();
        let mut got = Vec::new();
        let n = tick(&conn, NINE - 1700, "Europe/Berlin", |r| got.push(r.title.clone())).unwrap();
        assert_eq!((n, got.as_slice()), (1, ["ganztägig".to_string()].as_slice()));
        assert_eq!(tick(&conn, NINE - 1700, "Europe/Berlin", |_| panic!("doppelt")).unwrap(), 0);
    }

    #[test]
    fn done_and_deleted_tasks_do_not_remind() {
        let conn = setup();
        conn.execute("UPDATE tasks SET done_at = 1 WHERE title = 'ganztägig'", []).unwrap();
        conn.execute("UPDATE tasks SET deleted_at = 1 WHERE title = 'mit Uhrzeit'", []).unwrap();
        assert!(pending(&conn, 0, i64::MAX, "Europe/Berlin").unwrap().is_empty());
    }
}
