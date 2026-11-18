//! Nächste Fälligkeit einer wiederkehrenden Aufgabe. Reine Funktionen, keine Datenbank.
//! Ortszeit bleibt über die Zeitumstellung stabil (Fälligkeit „9:00“ bleibt 9:00 Ortszeit).
use super::types::{RepeatKind, RepeatRule};
use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Tz;

fn add_months(d: NaiveDate, months: u32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months as i32;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let mut day = d.day();
    loop {
        if let Some(n) = NaiveDate::from_ymd_opt(y, m, day) {
            return n;
        }
        day -= 1;
    }
}

fn weekday_no(w: Weekday) -> u8 {
    w.number_from_monday() as u8
}

/// Nächster Termin nach `base` (immer später als `base`).
pub fn next_date(rule: &RepeatRule, base: NaiveDate) -> NaiveDate {
    let n = rule.interval.max(1);
    match rule.kind {
        RepeatKind::Daily => base + Duration::days(n as i64),
        RepeatKind::Weekly if rule.weekdays.is_empty() => base + Duration::days(7 * n as i64),
        RepeatKind::Weekly => {
            let monday = base - Duration::days(base.weekday().num_days_from_monday() as i64);
            let mut d = base + Duration::days(1);
            loop {
                let week = (d - monday).num_days() / 7;
                if rule.weekdays.contains(&weekday_no(d.weekday())) && (week == 0 || week % n as i64 == 0) {
                    return d;
                }
                d += Duration::days(1);
            }
        }
        RepeatKind::Monthly => add_months(base, n),
        RepeatKind::Yearly => add_months(base, 12 * n),
    }
}

fn local_to_utc(tz: Tz, local: NaiveDateTime) -> i64 {
    match tz.from_local_datetime(&local) {
        LocalResult::Single(t) => t.timestamp(),
        LocalResult::Ambiguous(first, _) => first.timestamp(),
        // Lücke der Umstellung: eine Stunde später nehmen
        LocalResult::None => tz.from_local_datetime(&(local + Duration::hours(1))).earliest().map_or(0, |t| t.timestamp()),
    }
}

/// Neue Fälligkeit nach dem Abhaken. `due`: Datum `YYYY-MM-DD` oder UTC-Sekunden (bei `due_has_time`).
/// Ergebnis liegt immer nach `due` **und** nach dem Tag des Abhakens; bei `from_done` zählt vom Abhaken aus.
pub fn advance(rule: &RepeatRule, due: &str, due_has_time: bool, done_at: i64, tz: Tz) -> Option<String> {
    let done_day = tz.timestamp_opt(done_at, 0).single()?.date_naive();
    if due_has_time {
        let local = tz.timestamp_opt(due.parse::<i64>().ok()?, 0).single()?.naive_local();
        let mut base = if rule.from_done { done_day.and_time(local.time()) } else { local };
        for _ in 0..2000 {
            let day = next_date(rule, base.date());
            base = day.and_time(local.time());
            if day > done_day || rule.from_done {
                return Some(local_to_utc(tz, base).to_string());
            }
        }
        None
    } else {
        let mut base = if rule.from_done { done_day } else { NaiveDate::parse_from_str(due, "%Y-%m-%d").ok()? };
        for _ in 0..2000 {
            base = next_date(rule, base);
            if base > done_day || rule.from_done {
                return Some(base.format("%Y-%m-%d").to_string());
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "repeat_tests.rs"]
mod tests;
