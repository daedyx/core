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

#[test]
fn huge_weekday_lists_are_refused_without_work() {
    // Über den Abgleich kann eine Regel mit beliebig langer Liste kommen; jeder Schritt wäre dann teuer.
    let many = vec![1u8; 1_000_000];
    assert_eq!(next_date(&rule(RepeatKind::Weekly, 1, &many, None), d("2026-11-18")), None);
    assert!(!valid_rule(&rule(RepeatKind::Weekly, 1, &[1, 2, 3, 4, 5, 6, 7, 1], None)));
}

#[test]
fn valid_rule_accepts_only_sensible_values() {
    assert!(valid_rule(&rule(RepeatKind::Weekly, 1, &[1, 7], None)));
    assert!(valid_rule(&rule(RepeatKind::Daily, MAX_INTERVAL, &[], None)));
    assert!(valid_rule(&rule(RepeatKind::Monthly, 1, &[], Some(31))));
    assert!(valid_rule(&rule(RepeatKind::Yearly, 2, &[], Some(1))));
    for wd in [0u8, 8, 9, 255] {
        assert!(!valid_rule(&rule(RepeatKind::Weekly, 1, &[wd], None)), "{wd}");
        assert!(!valid_rule(&rule(RepeatKind::Weekly, 1, &[1, wd], None)), "gemischt {wd}");
    }
    for n in [0, MAX_INTERVAL.saturating_add(1), u32::MAX] {
        assert!(!valid_rule(&rule(RepeatKind::Daily, n, &[], None)), "{n}");
    }
    for md in [0u8, 32, 255] {
        assert!(!valid_rule(&rule(RepeatKind::Monthly, 1, &[], Some(md))), "{md}");
    }
}

#[test]
fn a_whole_skipped_day_moves_to_the_next_valid_hour_instead_of_1970() {
    // Samoa ließ den 30.12.2011 ganz aus: auch eine Stunde später liegt noch in der Lücke.
    let apia = chrono_tz::Pacific::Apia;
    let local = NaiveDate::from_ymd_opt(2011, 12, 30).and_then(|d| d.and_hms_opt(9, 0, 0)).unwrap();
    let next_day = NaiveDate::from_ymd_opt(2011, 12, 31).and_then(|d| d.and_hms_opt(0, 0, 0)).unwrap();
    let want = apia.from_local_datetime(&next_day).single().map(|t| t.timestamp());
    assert!(want.is_some_and(|t| t > 0));
    assert_eq!(local_to_utc(apia, local), want);
}

#[test]
fn due_and_done_near_the_end_of_the_calendar_end_without_panic() {
    // Gerade noch darstellbare UTC-Sekunden: die Ortszeit (Berlin +1) läge schon hinter dem Kalenderende.
    let edge = NaiveDateTime::MAX.and_utc().timestamp();
    let first = NaiveDateTime::MIN.and_utc().timestamp();
    let last_day = NaiveDate::MAX.format("%Y-%m-%d").to_string();
    for kind in [RepeatKind::Daily, RepeatKind::Weekly, RepeatKind::Monthly, RepeatKind::Yearly] {
        for from_done in [false, true] {
            let r = RepeatRule { from_done, ..rule(kind.clone(), 1, &[], None) };
            for secs in [edge, edge.saturating_sub(3600), first, first.saturating_add(3600), i64::MIN, i64::MAX] {
                assert_eq!(advance(&r, &secs.to_string(), true, 0, Berlin), None, "{kind:?} {secs}");
                assert_eq!(advance(&r, "2026-11-18", false, secs, Berlin), None, "{kind:?} erledigt {secs}");
                assert_eq!(advance(&r, &secs.to_string(), true, secs, Berlin), None, "{kind:?} beides {secs}");
            }
            // Mit `from_done` zählt die Regel ab dem Abhaken (hier 1970), nicht ab dem letzten Tag.
            if !from_done {
                assert_eq!(advance(&r, &last_day, false, 0, Berlin), None, "{kind:?} letzter Tag");
            }
        }
    }
}

#[test]
fn ordinary_dates_still_advance() {
    let r = rule(RepeatKind::Daily, 1, &[], None);
    assert_eq!(advance(&r, "2026-11-18", false, 0, Berlin).as_deref(), Some("2026-11-19"));
    assert_eq!(advance(&r, "1795096800", true, 0, Berlin).as_deref(), Some("1795183200"));
}

#[test]
fn plain_dates_outside_1970_to_9999_do_not_advance() {
    let r = rule(RepeatKind::Daily, 1, &[], None);
    for due in ["1969-12-31", "0000-01-01", "9999-12-31"] {
        assert_eq!(advance(&r, due, false, 0, Berlin), None, "{due}");
    }
    assert_eq!(advance(&r, "1970-01-01", false, 0, Berlin).as_deref(), Some("1970-01-02"));
}
