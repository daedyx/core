//! Erlaubter Bereich für Zeitpunkte (Fälligkeit mit Uhrzeit, Erledigt-Zeit): 1970 bis Ende 9999 in UTC-Sekunden.
//! Innerhalb davon läuft keine Rechnung über, und die Umrechnung in Ortszeit bleibt im Kalender von chrono
//! (Review Tasks T0 M-1: Werte am Rand ließen `base - min * 60` überlaufen und `naive_local` abbrechen).
//! Ein Datum ohne Uhrzeit gilt ebenso nur in den Jahren 1970 bis 9999 (eine Regel, Review Kern-Fix N-2).
//! Öffentlich, damit Oberflächen (Desk) vor eigenen Umrechnungen denselben Schutz nutzen.
use chrono::{Datelike, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;

/// 9999-12-31 23:59:59 UTC.
pub const MAX_DUE_SECS: i64 = 253_402_300_799;

/// `true`, wenn `secs` im erlaubten Bereich liegt.
pub fn in_range(secs: i64) -> bool {
    (0..=MAX_DUE_SECS).contains(&secs)
}

/// UTC-Sekunden aus dem Text einer Fälligkeit mit Uhrzeit, nur im erlaubten Bereich.
pub fn due_secs(text: &str) -> Option<i64> {
    text.parse::<i64>().ok().filter(|s| in_range(*s))
}

/// Datum `YYYY-MM-DD` einer Fälligkeit ohne Uhrzeit, nur in den Jahren 1970 bis 9999.
pub fn due_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok().filter(|d| (1970..=9999).contains(&d.year()))
}

/// Gültiger Text für `due`: UTC-Sekunden im erlaubten Bereich oder ein Datum `YYYY-MM-DD` (1970 bis 9999).
pub fn valid_due_text(text: &str) -> bool {
    match text.parse::<i64>() {
        Ok(secs) => in_range(secs),
        Err(_) => due_date(text).is_some(),
    }
}

/// UTC-Sekunden zu Ortszeit, `None` außerhalb des erlaubten Bereichs (statt Abbruch am Kalenderende).
pub fn local_at(tz: Tz, secs: i64) -> Option<NaiveDateTime> {
    if !in_range(secs) {
        return None;
    }
    Some(tz.timestamp_opt(secs, 0).single()?.naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{Europe::Berlin, Pacific::Kiritimati};

    #[test]
    fn the_range_is_1970_to_9999() {
        for ok in [0, 1_795_096_800, MAX_DUE_SECS] {
            assert!(in_range(ok) && due_secs(&ok.to_string()) == Some(ok), "{ok}");
        }
        for bad in [i64::MIN, -1, MAX_DUE_SECS.saturating_add(1), i64::MAX] {
            assert!(!in_range(bad) && due_secs(&bad.to_string()).is_none(), "{bad}");
            assert!(local_at(Berlin, bad).is_none(), "{bad}");
        }
        assert!(valid_due_text("2026-12-01") && !valid_due_text("morgen") && !valid_due_text("-1"));
        assert!(due_date("1970-01-01").is_some() && due_date("9999-12-31").is_some());
        for bad in ["1969-12-31", "0000-01-01", "+10000-01-01", "2026-02-30"] {
            assert!(due_date(bad).is_none() && !valid_due_text(bad), "{bad}");
        }
    }

    #[test]
    fn local_time_becomes_utc_and_the_gap_takes_the_hour_after() {
        let at = |y, m, d, h, mi| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, 0).unwrap();
        assert_eq!(crate::repeat::local_to_utc(Berlin, at(2026, 11, 18, 15, 0)), Some(1_795_010_400));
        // 29.03.2026 02:30 gibt es in Berlin nicht: es gilt 03:30 Sommerzeit (01:30 UTC).
        assert_eq!(crate::repeat::local_to_utc(Berlin, at(2026, 3, 29, 2, 30)), Some(1_774_747_800));
        // 25.10.2026 02:30 gibt es zweimal: es gilt das erste Mal (Sommerzeit, 00:30 UTC).
        assert_eq!(crate::repeat::local_to_utc(Berlin, at(2026, 10, 25, 2, 30)), Some(1_792_888_200));
    }

    #[test]
    fn the_edges_convert_even_in_the_furthest_zone() {
        assert!(local_at(Kiritimati, MAX_DUE_SECS).is_some());
        assert!(local_at(chrono_tz::Etc::GMTPlus12, 0).is_some());
    }
}
