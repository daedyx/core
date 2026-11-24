//! Missbrauchstests (SICHERHEIT 2.2/2.3, Review Tasks T0 M-1 und N-7): unsinnige Werte oder falsche Datentypen aus
//! einer veränderten Datei (oder später aus dem Abgleich) dürfen weder abstürzen noch die übrigen Erinnerungen
//! abschalten. Ungültiges wird übersprungen, Gültiges weiter gemeldet, die Daten bleiben unverändert.
use super::*;
use crate::edit_alarms::set_alarms;
use crate::edit_lists::create_list;
use crate::edit_tasks::create_task;
use crate::test_db::fresh;
use serde_json::{json, Value};

const BERLIN: &str = "Europe/Berlin";
/// 2026-11-19 10:00 UTC, eine gültige Fälligkeit mit Uhrzeit.
const DUE: i64 = 1_795_096_800;
const NOWS: [i64; 5] = [i64::MIN, -1, DUE, 253_402_300_799, i64::MAX];

fn task(conn: &Connection, list: &str, title: &str, due: &str, minutes: &[i64]) -> String {
    let f = json!({"title": title, "due": due, "due_has_time": true});
    let id = create_task(conn, list, None, f.as_object().unwrap(), 1000).unwrap().id;
    set_alarms(conn, &id, minutes, 1000).unwrap();
    id
}

fn setup() -> (Connection, String) {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    task(&conn, &l, "Gültige Aufgabe", &DUE.to_string(), &[60]);
    (conn, l)
}

/// Inhalt der Tabellen, die ein Durchgang nicht ändern darf.
fn snapshot(conn: &Connection) -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    for sql in [
        "SELECT id, title, due, due_has_time, remind_minutes, deleted_at, done_at FROM tasks ORDER BY id",
        "SELECT id, task_id, minutes_before, deleted_at FROM task_alarms ORDER BY id",
    ] {
        let mut st = conn.prepare(sql).unwrap();
        let n = st.column_count();
        let rows = st
            .query_map([], |r| {
                (0..n).map(|i| r.get_ref(i).map(|v| json!(format!("{v:?}")))).collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        out.extend(rows.map(Result::unwrap));
    }
    out
}

fn titles(conn: &Connection) -> Vec<String> {
    pending(conn, i64::MIN, i64::MAX, BERLIN).unwrap().into_iter().map(|r| r.title).collect()
}

#[test]
fn absurd_reminder_minutes_are_skipped() {
    let (conn, l) = setup();
    for (i, m) in [i64::MAX, i64::MIN, -1, 525_601].into_iter().enumerate() {
        let id = task(&conn, &l, &format!("Kaputt {i}"), &DUE.to_string(), &[5]);
        conn.execute("UPDATE task_alarms SET minutes_before = ?1 WHERE task_id = ?2", (m, &id)).unwrap();
        conn.execute("UPDATE tasks SET remind_minutes = ?1 WHERE id = ?2", (m, &id)).unwrap();
    }
    let before = snapshot(&conn);
    assert_eq!(titles(&conn), ["Gültige Aufgabe"]);
    for now in NOWS {
        tick(&conn, now, BERLIN, |_| {}).unwrap();
    }
    assert_eq!(snapshot(&conn), before);
}

#[test]
fn the_legacy_field_with_absurd_minutes_is_skipped() {
    let (conn, l) = setup();
    for (i, m) in [i64::MAX, i64::MIN, -1].into_iter().enumerate() {
        let id = task(&conn, &l, &format!("Alt {i}"), &DUE.to_string(), &[]);
        conn.execute("UPDATE tasks SET remind_minutes = ?1 WHERE id = ?2", (m, &id)).unwrap();
    }
    assert_eq!(titles(&conn), ["Gültige Aufgabe"]);
}

#[test]
fn absurd_due_times_are_skipped() {
    let (conn, l) = setup();
    for (i, due) in [i64::MIN, i64::MAX, -1, 253_402_300_800].into_iter().enumerate() {
        let id = task(&conn, &l, &format!("Zeit {i}"), &DUE.to_string(), &[1, 525_600]);
        conn.execute("UPDATE tasks SET due = ?1 WHERE id = ?2", (due.to_string(), &id)).unwrap();
    }
    let before = snapshot(&conn);
    assert_eq!(titles(&conn), ["Gültige Aufgabe"]);
    for now in NOWS {
        tick(&conn, now, BERLIN, |_| {}).unwrap();
    }
    assert_eq!(snapshot(&conn), before);
}

#[test]
fn the_edges_of_the_allowed_range_still_remind() {
    let (conn, l) = setup();
    task(&conn, &l, "Anfang", "0", &[0]);
    task(&conn, &l, "Ende", "253402300799", &[525_600]);
    let mut got = titles(&conn);
    got.sort();
    assert_eq!(got, ["Anfang", "Ende", "Gültige Aufgabe"]);
}

#[test]
fn a_row_with_a_wrong_data_type_does_not_silence_the_other_reminders() {
    let (conn, l) = setup();
    let blob = task(&conn, &l, "Titel kaputt", &DUE.to_string(), &[30]);
    conn.execute("UPDATE tasks SET title = X'00FF' WHERE id = ?1", [&blob]).unwrap();
    let half = task(&conn, &l, "Eine Erinnerung kaputt", &DUE.to_string(), &[10, 20]);
    conn.execute("UPDATE task_alarms SET minutes_before = 'abc' WHERE task_id = ?1 AND minutes_before = 20", [&half])
        .unwrap();
    let all = task(&conn, &l, "Alle Erinnerungen kaputt", &DUE.to_string(), &[15]);
    conn.execute("UPDATE task_alarms SET minutes_before = 'abc' WHERE task_id = ?1", [&all]).unwrap();
    let before = snapshot(&conn);
    let found = pending(&conn, i64::MIN, i64::MAX, BERLIN).unwrap();
    let mut got: Vec<(String, i64)> = found.into_iter().map(|r| (r.title, r.minutes_before)).collect();
    got.sort();
    // „Alle Erinnerungen kaputt“ hat eigene Zeilen und fällt daher nicht auf das alte Einzelfeld (15) zurück.
    assert_eq!(got, [("Eine Erinnerung kaputt".to_string(), 10), ("Gültige Aufgabe".to_string(), 60)]);
    let mut fired = Vec::new();
    tick(&conn, DUE.saturating_sub(5 * 60), BERLIN, |r| fired.push(r.minutes_before)).unwrap();
    fired.sort_unstable();
    assert_eq!(fired, [10, 60], "die Stunde davor: beide gültigen Erinnerungen, keine kaputte");
    assert_eq!(snapshot(&conn), before);
}

#[test]
fn absurd_now_values_do_not_panic() {
    let (conn, _) = setup();
    for now in NOWS {
        assert!(tick(&conn, now, BERLIN, |_| {}).is_ok(), "{now}");
        assert!(pending(&conn, now, now, BERLIN).is_ok_and(|v| v.is_empty()), "{now}");
    }
}
