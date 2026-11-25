use super::*;

const DAY: i64 = AUTO_CHECK_INTERVAL_SECS;
const NOW: i64 = 1_795_000_000;
// Zeitpunkte als Konstanten, damit die Tests ohne Rechnung zur Laufzeit auskommen.
const NEXT: i64 = NOW + 1;
const ALMOST_A_DAY: i64 = NOW + DAY - 1;
const A_DAY: i64 = NOW + DAY;
const DAY_BEFORE: i64 = NOW - DAY;
const MORE_THAN_DAY_BEFORE: i64 = NOW - DAY - 1;
const TEN_DAYS: i64 = NOW + 10 * DAY;

#[test]
fn a_missing_file_gives_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let s = UpdateState::load(dir.path()).unwrap();
    assert_eq!(s, UpdateState::default());
    assert!(s.auto_check && s.last_check.is_none() && s.highest_seen().is_none());
}

#[test]
fn saving_and_loading_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = UpdateState { auto_check: false, ..Default::default() };
    s.note_check(NOW);
    s.note_seen(&Version::new(0, 2, 1));
    s.save(dir.path()).unwrap();
    assert_eq!(UpdateState::load(dir.path()).unwrap(), s);
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, [STATE_FILE], "keine Nebendatei bleibt liegen");
    s.note_check(NEXT);
    s.save(dir.path()).unwrap();
    assert_eq!(UpdateState::load(dir.path()).unwrap().last_check, Some(NEXT));
}

#[test]
fn the_highest_seen_version_only_goes_up() {
    let mut s = UpdateState::default();
    s.note_seen(&Version::new(0, 3, 0));
    s.note_seen(&Version::new(0, 2, 0));
    assert_eq!(s.highest_seen(), Some(Version::new(0, 3, 0)));
}

#[test]
fn damaged_files_are_reported_and_fall_back_to_defaults() {
    for bad in [
        &b"{"[..],
        b"[]",
        b"{\"auto_check\": true, \"extra\": 1}",
        b"{\"highest_seen\": \"0.3.0-rc1\"}",
        b"{\"auto_check\": \"ja\"}",
        b"{\"auto_check\": true, \"auto_check\": false}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(STATE_FILE), bad).unwrap();
        assert_eq!(UpdateState::load(dir.path()), Err(StateError::Damaged));
        assert_eq!(UpdateState::load_or_default(dir.path()), UpdateState::default());
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(STATE_FILE), vec![b' '; 4097]).unwrap();
    assert_eq!(UpdateState::load(dir.path()), Err(StateError::Damaged));
}

#[test]
fn auto_check_runs_at_most_once_a_day_and_only_when_switched_on() {
    let mut s = UpdateState::default();
    assert!(s.due_for_auto_check(NOW));
    s.note_check(NOW);
    assert!(!s.due_for_auto_check(ALMOST_A_DAY));
    assert!(s.due_for_auto_check(A_DAY));
    // Uhr stand weit in der Zukunft: nicht für immer still bleiben.
    assert!(!s.due_for_auto_check(DAY_BEFORE));
    assert!(s.due_for_auto_check(MORE_THAN_DAY_BEFORE));
    s.auto_check = false;
    assert!(!s.due_for_auto_check(TEN_DAYS));
}

#[test]
fn extreme_times_do_not_overflow() {
    let mut s = UpdateState::default();
    for (last, now) in [(i64::MIN, i64::MAX), (i64::MAX, i64::MIN), (0, 0), (i64::MAX, i64::MAX)] {
        s.last_check = Some(last);
        let _ = s.due_for_auto_check(now);
    }
}
