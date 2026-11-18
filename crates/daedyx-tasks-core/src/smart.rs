//! Smart-Listen (Heute, Geplant, Überfällig, Alle, Erledigt) und Suche. Reine Lesezugriffe.
use super::errors::TaskError;
use super::store::{task_from_row, tasks_in_list, TASK_COLS};
use super::types::Task;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum View {
    /// Eine einzelne Liste (`list_id` nötig).
    List,
    Today,
    Planned,
    Overdue,
    All,
    Done,
}

/// Kalendertag der Fälligkeit in der Anzeigezone.
pub fn due_day(t: &Task, tz: Tz) -> Option<NaiveDate> {
    let due = t.due.as_deref()?;
    if t.due_has_time {
        Some(tz.timestamp_opt(due.parse().ok()?, 0).single()?.date_naive())
    } else {
        NaiveDate::parse_from_str(due, "%Y-%m-%d").ok()
    }
}

fn live_tasks(conn: &Connection, filter: &str) -> Result<Vec<Task>, TaskError> {
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE deleted_at IS NULL AND list_id IN \
         (SELECT id FROM task_lists WHERE deleted_at IS NULL) AND {filter}"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map([], task_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Frühester Tag zuerst, an einem Tag Ganztägiges vor Uhrzeiten (nach Uhrzeit), dann hohe Priorität, dann Handreihenfolge.
fn by_due(tz: Tz) -> impl Fn(&Task, &Task) -> std::cmp::Ordering {
    move |a, b| {
        due_day(a, tz)
            .cmp(&due_day(b, tz))
            .then(a.due_has_time.cmp(&b.due_has_time))
            .then_with(|| {
                let secs = |t: &Task| t.due.as_deref().and_then(|d| d.parse::<i64>().ok()).filter(|_| t.due_has_time);
                secs(a).cmp(&secs(b))
            })
            .then(b.priority.cmp(&a.priority))
            .then(a.position.total_cmp(&b.position))
    }
}

pub fn query(conn: &Connection, view: View, list_id: Option<&str>, today: NaiveDate, tz: Tz) -> Result<Vec<Task>, TaskError> {
    match view {
        View::List => tasks_in_list(conn, list_id.ok_or(TaskError::Invalid("list"))?),
        View::All => {
            let mut v = live_tasks(conn, "done_at IS NULL")?;
            v.sort_by(|a, b| a.list_id.cmp(&b.list_id).then(a.position.total_cmp(&b.position)));
            Ok(v)
        }
        View::Done => {
            let mut v = live_tasks(conn, "done_at IS NOT NULL")?;
            v.sort_by_key(|a| std::cmp::Reverse(a.done_at));
            v.truncate(200);
            Ok(v)
        }
        View::Today | View::Planned | View::Overdue => {
            let mut v: Vec<Task> = live_tasks(conn, "done_at IS NULL AND due IS NOT NULL")?
                .into_iter()
                .filter(|t| {
                    let Some(d) = due_day(t, tz) else { return false };
                    match view {
                        View::Today => d == today,
                        View::Overdue => d < today,
                        _ => true,
                    }
                })
                .collect();
            v.sort_by(by_due(tz));
            Ok(v)
        }
    }
}

/// Anzahl je Smart-Liste für die Seitenleiste.
pub fn counts(conn: &Connection, today: NaiveDate, tz: Tz) -> Result<[(View, usize); 4], TaskError> {
    let n = |v| query(conn, v, None, today, tz).map(|r| r.len());
    Ok([(View::Today, n(View::Today)?), (View::Planned, n(View::Planned)?), (View::Overdue, n(View::Overdue)?), (View::All, n(View::All)?)])
}

/// Suche in Titel und Notiz (ohne Groß-/Kleinschreibung), offene vor erledigten, höchstens `limit` Treffer.
pub fn search(conn: &Connection, text: &str, limit: usize) -> Result<Vec<Task>, TaskError> {
    let needle = text.trim();
    if needle.is_empty() {
        return Ok(vec![]);
    }
    let esc = needle.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE deleted_at IS NULL AND list_id IN \
         (SELECT id FROM task_lists WHERE deleted_at IS NULL) \
         AND (title LIKE ?1 ESCAPE '\\' OR notes LIKE ?1 ESCAPE '\\') \
         ORDER BY done_at IS NOT NULL, position LIMIT ?2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map((format!("%{esc}%"), limit as i64), task_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
#[path = "smart_tests.rs"]
mod tests;
