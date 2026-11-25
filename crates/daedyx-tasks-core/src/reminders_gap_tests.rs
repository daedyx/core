//! Review T1a R-3: Auch die Erinnerung „9 Uhr am Fälligkeitstag“ (Aufgabe ohne Uhrzeit) folgt der einen Regel für
//! Zeitlücken (`repeat::local_to_utc`). In Apia fehlt der 30.12.2011 ganz; die Erinnerung fällt nicht still weg,
//! sondern kommt zur ersten gültigen Zeit danach (31.12. 00:00).
use super::*;
use crate::edit_lists::create_list;
use crate::edit_tasks::create_task;
use crate::test_db::fresh;
use chrono::TimeZone;
use serde_json::json;

#[test]
fn the_nine_oclock_reminder_in_the_apia_gap_comes_at_the_next_valid_time() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    let f = json!({"title": "Probe", "due": "2011-12-30", "due_has_time": false, "remind_minutes": 0});
    create_task(&conn, &l, None, f.as_object().unwrap(), 1000).unwrap();
    let apia = chrono_tz::Pacific::Apia;
    let want = apia.with_ymd_and_hms(2011, 12, 31, 0, 0, 0).single().unwrap().timestamp();
    let got = pending(&conn, want - 3600, want + 1, "Pacific/Apia").unwrap();
    assert_eq!(got.iter().map(|r| r.fire_at).collect::<Vec<_>>(), [want]);
}

#[test]
fn the_nine_oclock_reminder_elsewhere_is_unchanged() {
    let conn = fresh();
    let l = create_list(&conn, "A", "", 1000).unwrap().id;
    let f = json!({"title": "Probe", "due": "2026-11-19", "due_has_time": false, "remind_minutes": 0});
    create_task(&conn, &l, None, f.as_object().unwrap(), 1000).unwrap();
    let berlin = chrono_tz::Europe::Berlin;
    let want = berlin.with_ymd_and_hms(2026, 11, 19, 9, 0, 0).single().unwrap().timestamp();
    let got = pending(&conn, want - 1, want + 1, "Europe/Berlin").unwrap();
    assert_eq!(got.iter().map(|r| r.fire_at).collect::<Vec<_>>(), [want]);
}
