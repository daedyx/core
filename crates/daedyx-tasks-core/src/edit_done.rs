//! Abhaken und Wieder-Öffnen. Wiederkehrende Aufgaben bleiben offen: Verlaufseintrag plus neue Fälligkeit.
use super::errors::TaskError;
use super::repeat::advance;
use super::stamp::{apply, COMPLETIONS, TASKS};
use super::store;
use super::types::Task;
use crate::ids::new_id;
use chrono_tz::Tz;
use rusqlite::Connection;
use serde_json::json;

/// Offen → erledigt (bzw. bei Wiederholung: nächster Termin), erledigt → wieder offen.
pub fn toggle_done(conn: &Connection, id: &str, now_ms: u64, tz: Tz) -> Result<Task, TaskError> {
    let t = store::task(conn, id)?;
    let now = (now_ms / 1000) as i64;
    if t.done_at.is_some() {
        apply(conn, &TASKS, id, &[("done_at", json!(null))], now_ms)?;
        return store::task(conn, id);
    }
    let next = match (&t.repeat, &t.due) {
        (Some(rule), Some(due)) => advance(rule, due, t.due_has_time, now, tz),
        _ => None,
    };
    match next {
        Some(next_due) => {
            let cid = new_id();
            conn.execute(
                "INSERT INTO task_completions (id, task_id, done_at, due_was, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                (&cid, id, now, &t.due, now),
            )?;
            apply(
                conn,
                &COMPLETIONS,
                &cid,
                &[("task_id", json!(id)), ("done_at", json!(now)), ("due_was", json!(t.due))],
                now_ms,
            )?;
            apply(conn, &TASKS, id, &[("due", json!(next_due))], now_ms)?;
        }
        None => apply(conn, &TASKS, id, &[("done_at", json!(now))], now_ms)?,
    }
    store::task(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit_lists::create_list;
    use crate::edit_tasks::create_task;
    use crate::test_db::fresh;
    use chrono_tz::Europe::Berlin;
    use serde_json::{Map, Value};

    fn fields(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    // 2026-11-19 12:00 UTC
    const NOW_MS: u64 = 1_795_089_600_000;

    #[test]
    fn plain_task_is_done_and_reopened() {
        let conn = fresh();
        let l = create_list(&conn, "A", "", 1000).unwrap();
        let t = create_task(&conn, &l.id, None, &fields(json!({"title": "x"})), 1000).unwrap();
        let done = toggle_done(&conn, &t.id, NOW_MS, Berlin).unwrap();
        assert_eq!(done.done_at, Some(1_795_089_600));
        let open = toggle_done(&conn, &t.id, NOW_MS + 1000, Berlin).unwrap();
        assert_eq!(open.done_at, None);
    }

    #[test]
    fn repeating_task_stays_open_moves_on_and_logs_the_completion() {
        let conn = fresh();
        let l = create_list(&conn, "A", "", 1000).unwrap();
        let t = create_task(
            &conn,
            &l.id,
            None,
            &fields(json!({"title": "Pflanzen gießen", "due": "2026-11-19",
                "repeat_rule": {"kind": "weekly", "interval": 1}})),
            1000,
        )
        .unwrap();
        let after = toggle_done(&conn, &t.id, NOW_MS, Berlin).unwrap();
        assert_eq!((after.done_at, after.due.as_deref()), (None, Some("2026-11-26")));
        let (n, due_was, dirty): (i64, String, i64) = conn
            .query_row(
                "SELECT count(*), max(due_was), min(dirty) FROM task_completions WHERE task_id = ?1",
                [&t.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((n, due_was.as_str(), dirty), (1, "2026-11-19", 1));
        toggle_done(&conn, &t.id, NOW_MS + 5000, Berlin).unwrap();
        let n: i64 = conn.query_row("SELECT count(*) FROM task_completions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn repeat_without_due_date_just_completes() {
        let conn = fresh();
        let l = create_list(&conn, "A", "", 1000).unwrap();
        let t = create_task(
            &conn,
            &l.id,
            None,
            &fields(json!({"title": "x", "repeat_rule": {"kind": "daily", "interval": 1}})),
            1000,
        )
        .unwrap();
        assert!(toggle_done(&conn, &t.id, NOW_MS, Berlin).unwrap().done_at.is_some());
    }
}
