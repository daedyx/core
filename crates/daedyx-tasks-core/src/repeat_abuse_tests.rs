//! Missbrauchstests (SICHERHEIT 2.2/2.3): feindliche Regeln dürfen weder abstürzen noch hängen.
use super::*;
use chrono_tz::Europe::Berlin;

fn rule(kind: RepeatKind, interval: u32, weekdays: &[u8], month_day: Option<u8>) -> RepeatRule {
    RepeatRule { kind, interval, weekdays: weekdays.to_vec(), from_done: false, month_day }
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn weekdays_outside_1_to_7_end_without_hanging() {
    for wd in [0u8, 8, 9, 255] {
        assert_eq!(next_date(&rule(RepeatKind::Weekly, 1, &[wd], None), d("2026-11-18")), None, "{wd}");
    }
}

#[test]
fn huge_intervals_do_not_panic_or_hang() {
    for kind in [RepeatKind::Daily, RepeatKind::Weekly, RepeatKind::Monthly, RepeatKind::Yearly] {
        assert_eq!(next_date(&rule(kind.clone(), u32::MAX, &[], None), d("2026-11-18")), None, "{kind:?}");
    }
    assert_eq!(next_date(&rule(RepeatKind::Weekly, u32::MAX, &[1, 5], None), d("2026-11-18")), d("2026-11-20").into());
}

#[test]
fn dates_at_the_end_of_the_calendar_do_not_panic() {
    let last = NaiveDate::MAX;
    for kind in [RepeatKind::Daily, RepeatKind::Weekly, RepeatKind::Monthly, RepeatKind::Yearly] {
        assert_eq!(next_date(&rule(kind, 1, &[], Some(31)), last), None);
    }
}

#[test]
fn advance_with_hostile_due_values_returns_none() {
    let r = rule(RepeatKind::Daily, 1, &[], None);
    assert_eq!(advance(&r, &i64::MAX.to_string(), true, 0, Berlin), None);
    assert_eq!(advance(&r, "kaputt", false, 0, Berlin), None);
    assert_eq!(advance(&rule(RepeatKind::Weekly, 1, &[9], None), "2026-11-18", false, 0, Berlin), None);
    assert_eq!(advance(&rule(RepeatKind::Monthly, u32::MAX, &[], None), "2026-11-18", false, 0, Berlin), None);
}

#[test]
fn the_ends_of_time_do_not_panic() {
    assert!(local_to_utc(Berlin, NaiveDateTime::MAX).is_some_and(|t| t > 0));
    assert!(local_to_utc(Berlin, NaiveDateTime::MIN).is_some_and(|t| t < 0));
}
