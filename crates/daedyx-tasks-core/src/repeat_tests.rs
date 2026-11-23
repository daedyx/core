use super::*;
use chrono_tz::Europe::Berlin;

fn rule(kind: RepeatKind, interval: u32, weekdays: &[u8], from_done: bool) -> RepeatRule {
    RepeatRule { kind, interval, weekdays: weekdays.to_vec(), from_done, month_day: None }
}

fn anchored(kind: RepeatKind, interval: u32, day: u8) -> RepeatRule {
    RepeatRule { month_day: Some(day), ..rule(kind, interval, &[], false) }
}

fn nd(rule: &RepeatRule, base: NaiveDate) -> NaiveDate {
    next_date(rule, base).unwrap()
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn utc(s: &str) -> i64 {
    // Ortszeit Berlin „YYYY-MM-DD HH:MM“ → UTC-Sekunden
    local_to_utc(Berlin, NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
}

#[test]
fn daily_and_interval() {
    assert_eq!(nd(&rule(RepeatKind::Daily, 1, &[], false), d("2026-12-31")), d("2027-01-01"));
    assert_eq!(nd(&rule(RepeatKind::Daily, 3, &[], false), d("2026-12-30")), d("2027-01-02"));
}

#[test]
fn weekly_plain_and_with_weekdays() {
    assert_eq!(nd(&rule(RepeatKind::Weekly, 2, &[], false), d("2026-11-18")), d("2026-12-02"));
    // Mi 18.11.2026; Mo+Mi+Fr jede Woche → Freitag
    let r = rule(RepeatKind::Weekly, 1, &[1, 3, 5], false);
    assert_eq!(nd(&r, d("2026-11-18")), d("2026-11-20"));
    assert_eq!(nd(&r, d("2026-11-20")), d("2026-11-23"));
    // alle 2 Wochen Mo+Mi: nach Mi springt es in die übernächste Woche
    let r = rule(RepeatKind::Weekly, 2, &[1, 3], false);
    assert_eq!(nd(&r, d("2026-11-16")), d("2026-11-18"));
    assert_eq!(nd(&r, d("2026-11-18")), d("2026-11-30"));
}

#[test]
fn monthly_clamps_to_month_end_and_yearly_handles_leap_day() {
    let m = rule(RepeatKind::Monthly, 1, &[], false);
    assert_eq!(nd(&m, d("2026-01-31")), d("2026-02-28"));
    assert_eq!(nd(&m, d("2026-12-15")), d("2027-01-15"));
    let y = rule(RepeatKind::Yearly, 1, &[], false);
    assert_eq!(nd(&y, d("2028-02-29")), d("2029-02-28"));
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

#[test]
fn without_an_anchor_the_day_drifts_down_and_stays_there() {
    let m = rule(RepeatKind::Monthly, 1, &[], false);
    let feb = nd(&m, d("2026-01-31"));
    assert_eq!(nd(&m, feb), d("2026-03-28"), "altes Verhalten bleibt für Regeln ohne Anker");
}

#[test]
fn a_month_day_anchor_returns_to_the_31st_after_short_months() {
    let r = anchored(RepeatKind::Monthly, 1, 31);
    let mut day = d("2026-01-31");
    let mut seen = Vec::new();
    for _ in 0..6 {
        day = nd(&r, day);
        seen.push(day.format("%Y-%m-%d").to_string());
    }
    assert_eq!(seen, ["2026-02-28", "2026-03-31", "2026-04-30", "2026-05-31", "2026-06-30", "2026-07-31"]);
}

#[test]
fn the_anchor_wins_over_the_day_of_the_base_date_and_works_with_intervals_and_leap_years() {
    // Basis ist der 28. (rutscht aus Februar), Anker 30: der Folgemonat bekommt wieder den 30.
    assert_eq!(nd(&anchored(RepeatKind::Monthly, 1, 30), d("2026-02-28")), d("2026-03-30"));
    assert_eq!(nd(&anchored(RepeatKind::Monthly, 3, 31), d("2026-11-30")), d("2027-02-28"));
    assert_eq!(nd(&anchored(RepeatKind::Monthly, 1, 31), d("2027-12-31")), d("2028-01-31"));
    assert_eq!(nd(&anchored(RepeatKind::Monthly, 1, 31), d("2028-01-31")), d("2028-02-29"));
    // Jahr: Schalttag-Anker kehrt im nächsten Schaltjahr zurück
    let y = anchored(RepeatKind::Yearly, 1, 29);
    assert_eq!(nd(&y, d("2028-02-29")), d("2029-02-28"));
    assert_eq!(nd(&y, d("2029-02-28")), d("2030-02-28"));
    assert_eq!(nd(&anchored(RepeatKind::Yearly, 4, 29), d("2024-02-29")), d("2028-02-29"));
}

#[test]
fn the_anchor_is_ignored_for_daily_and_weekly_rules() {
    assert_eq!(nd(&anchored(RepeatKind::Daily, 1, 15), d("2026-11-18")), d("2026-11-19"));
    assert_eq!(nd(&anchored(RepeatKind::Weekly, 1, 15), d("2026-11-18")), d("2026-11-25"));
}

#[test]
fn advance_keeps_the_anchor_for_timed_and_date_only_dues() {
    let r = anchored(RepeatKind::Monthly, 1, 31);
    let done = utc("2026-02-20 12:00");
    assert_eq!(advance(&r, "2026-02-28", false, done, Berlin).unwrap(), "2026-03-31");
    let due = utc("2026-02-28 09:30");
    assert_eq!(advance(&r, &due.to_string(), true, done, Berlin).unwrap(), utc("2026-03-31 09:30").to_string());
}
