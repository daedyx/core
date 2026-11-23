//! Nächste Fälligkeit einer wiederkehrenden Aufgabe. Reine Funktionen, keine Datenbank.
//! Ortszeit bleibt über die Zeitumstellung stabil (Fälligkeit „9:00“ bleibt 9:00 Ortszeit).
use super::types::{RepeatKind, RepeatRule};
use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Tz;

/// `anchor`: fester Tag des Monats; ohne ihn zählt der Tag von `d` (und rutscht nach kurzen Monaten nach unten).
/// `None`, wenn das Ergebnis außerhalb des Kalenders läge.
fn add_months(d: NaiveDate, months: u32, anchor: Option<u8>) -> Option<NaiveDate> {
    let total =
        i64::from(d.year()).checked_mul(12)?.checked_add(i64::from(d.month0()))?.checked_add(i64::from(months))?;
    let y = i32::try_from(total.div_euclid(12)).ok()?;
    let m = u32::try_from(total.rem_euclid(12)).ok()?.checked_add(1)?;
    let first = anchor.map_or(d.day(), |a| u32::from(a.clamp(1, 31)));
    // Kurze Monate: vom gewünschten Tag abwärts bis zum letzten gültigen (höchstens 31 Versuche).
    (1..=first).rev().find_map(|day| NaiveDate::from_ymd_opt(y, m, day))
}

fn weekday_no(w: Weekday) -> u32 {
    w.number_from_monday()
}

fn plus_days(d: NaiveDate, days: i64) -> Option<NaiveDate> {
    d.checked_add_signed(Duration::try_days(days)?)
}

/// Erster Tag in `[from, from + count)`, dessen Wochentag in `weekdays` steht.
fn first_listed_day(weekdays: &[u8], from: NaiveDate, count: i64) -> Option<NaiveDate> {
    (0..count)
        .filter_map(|i| plus_days(from, i))
        .find(|d| weekdays.iter().any(|w| u32::from(*w) == weekday_no(d.weekday())))
}

/// Nächster Termin nach `base` (immer später als `base`). `None` bei unmöglichen Regeln
/// (z. B. Wochentag außerhalb 1–7) oder wenn das Ergebnis außerhalb des Kalenders läge.
pub fn next_date(rule: &RepeatRule, base: NaiveDate) -> Option<NaiveDate> {
    let n = i64::from(rule.interval.max(1));
    match rule.kind {
        RepeatKind::Daily => plus_days(base, n),
        RepeatKind::Weekly if rule.weekdays.is_empty() => plus_days(base, n.checked_mul(7)?),
        RepeatKind::Weekly => {
            // Rest der laufenden Woche, sonst die Woche `n` Wochen später (gleiche Regel wie früher Tag für Tag).
            let monday =
                base.checked_sub_signed(Duration::try_days(i64::from(base.weekday().num_days_from_monday()))?)?;
            let rest = 6_i64.checked_sub(i64::from(base.weekday().num_days_from_monday()))?;
            if let Some(day) = plus_days(base, 1).and_then(|from| first_listed_day(&rule.weekdays, from, rest)) {
                return Some(day);
            }
            first_listed_day(&rule.weekdays, plus_days(monday, n.checked_mul(7)?)?, 7)
        }
        RepeatKind::Monthly => add_months(base, rule.interval.max(1), rule.month_day),
        RepeatKind::Yearly => add_months(base, rule.interval.max(1).checked_mul(12)?, rule.month_day),
    }
}

fn local_to_utc(tz: Tz, local: NaiveDateTime) -> Option<i64> {
    match tz.from_local_datetime(&local) {
        LocalResult::Single(t) => Some(t.timestamp()),
        LocalResult::Ambiguous(first, _) => Some(first.timestamp()),
        // Lücke der Umstellung: eine Stunde später nehmen
        LocalResult::None => {
            let later = local.checked_add_signed(Duration::try_hours(1)?)?;
            Some(tz.from_local_datetime(&later).earliest().map_or(0, |t| t.timestamp()))
        }
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
            let day = next_date(rule, base.date())?;
            base = day.and_time(local.time());
            if day > done_day || rule.from_done {
                return local_to_utc(tz, base).map(|t| t.to_string());
            }
        }
        None
    } else {
        let mut base = if rule.from_done { done_day } else { NaiveDate::parse_from_str(due, "%Y-%m-%d").ok()? };
        for _ in 0..2000 {
            base = next_date(rule, base)?;
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

#[cfg(test)]
#[path = "repeat_abuse_tests.rs"]
mod abuse_tests;
