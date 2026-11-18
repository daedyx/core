use super::*;
use chrono_tz::Europe::Berlin;

fn rule(kind: RepeatKind, interval: u32, weekdays: &[u8], from_done: bool) -> RepeatRule {
    RepeatRule { kind, interval, weekdays: weekdays.to_vec(), from_done }
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn utc(s: &str) -> i64 {
    // Ortszeit Berlin „YYYY-MM-DD HH:MM“ → UTC-Sekunden
    local_to_utc(Berlin, NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap())
}

#[test]
fn daily_and_interval() {
    assert_eq!(next_date(&rule(RepeatKind::Daily, 1, &[], false), d("2026-12-31")), d("2027-01-01"));
    assert_eq!(next_date(&rule(RepeatKind::Daily, 3, &[], false), d("2026-12-30")), d("2027-01-02"));
}

#[test]
fn weekly_plain_and_with_weekdays() {
    assert_eq!(next_date(&rule(RepeatKind::Weekly, 2, &[], false), d("2026-11-18")), d("2026-12-02"));
    // Mi 18.11.2026; Mo+Mi+Fr jede Woche → Freitag
    let r = rule(RepeatKind::Weekly, 1, &[1, 3, 5], false);
    assert_eq!(next_date(&r, d("2026-11-18")), d("2026-11-20"));
    assert_eq!(next_date(&r, d("2026-11-20")), d("2026-11-23"));
    // alle 2 Wochen Mo+Mi: nach Mi springt es in die übernächste Woche
    let r = rule(RepeatKind::Weekly, 2, &[1, 3], false);
    assert_eq!(next_date(&r, d("2026-11-16")), d("2026-11-18"));
    assert_eq!(next_date(&r, d("2026-11-18")), d("2026-11-30"));
}

#[test]
fn monthly_clamps_to_month_end_and_yearly_handles_leap_day() {
    let m = rule(RepeatKind::Monthly, 1, &[], false);
    assert_eq!(next_date(&m, d("2026-01-31")), d("2026-02-28"));
    assert_eq!(next_date(&m, d("2026-12-15")), d("2027-01-15"));
    let y = rule(RepeatKind::Yearly, 1, &[], false);
    assert_eq!(next_date(&y, d("2028-02-29")), d("2029-02-28"));
}

#[test]
fn advance_counts_from_due_but_never_lands_before_the_done_day() {
    let daily = rule(RepeatKind::Daily, 1, &[], false);
    let done = utc("2026-11-19 12:00");
    assert_eq!(advance(&daily, "2026-11-18", false, done, Berlin).unwrap(), "2026-11-20");
    // pünktlich abgehakt: einfach der nächste Tag
    assert_eq!(advance(&daily, "2026-11-19", false, done, Berlin).unwrap(), "2026-11-20");
    // früh abgehakt: zählt ab Fälligkeit
    assert_eq!(advance(&daily, "2026-11-25", false, done, Berlin).unwrap(), "2026-11-26");
}

#[test]
fn advance_from_done_counts_from_the_completion_day() {
    let r = rule(RepeatKind::Weekly, 1, &[], true);
    assert_eq!(advance(&r, "2026-11-01", false, utc("2026-11-19 12:00"), Berlin).unwrap(), "2026-11-26");
}

#[test]
fn timed_due_keeps_local_time_across_the_dst_change() {
    // Sommerzeit endet 25.10.2026: 9:00 Ortszeit bleibt 9:00
    let r = rule(RepeatKind::Daily, 1, &[], false);
    let due = utc("2026-10-24 09:00");
    let done = utc("2026-10-24 08:00");
    let next: i64 = advance(&r, &due.to_string(), true, done, Berlin).unwrap().parse().unwrap();
    assert_eq!(next, utc("2026-10-25 09:00"));
    assert_eq!(next - due, 25 * 3600);
}

#[test]
fn garbage_due_gives_none() {
    let r = rule(RepeatKind::Daily, 1, &[], false);
    assert!(advance(&r, "morgen", false, 0, Berlin).is_none());
    assert!(advance(&r, "abc", true, 0, Berlin).is_none());
}
