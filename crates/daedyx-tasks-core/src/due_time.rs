//! Erlaubter Bereich für Zeitpunkte (Fälligkeit mit Uhrzeit, Erledigt-Zeit): 1970 bis Ende 9999 in UTC-Sekunden.
//! Innerhalb davon läuft keine Rechnung über, und die Umrechnung in Ortszeit bleibt im Kalender von chrono
//! (Review Tasks T0 M-1: Werte am Rand ließen `base - min * 60` überlaufen und `naive_local` abbrechen).
use chrono::{NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;

/// 9999-12-31 23:59:59 UTC.
pub(crate) const MAX_DUE_SECS: i64 = 253_402_300_799;

/// `true`, wenn `secs` im erlaubten Bereich liegt.
pub(crate) fn in_range(secs: i64) -> bool {
    (0..=MAX_DUE_SECS).contains(&secs)
}

/// UTC-Sekunden aus dem Text einer Fälligkeit mit Uhrzeit, nur im erlaubten Bereich.
pub(crate) fn due_secs(text: &str) -> Option<i64> {
    text.parse::<i64>().ok().filter(|s| in_range(*s))
}

/// Gültiger Text für `due`: UTC-Sekunden im erlaubten Bereich oder ein Datum `YYYY-MM-DD`.
pub(crate) fn valid_due_text(text: &str) -> bool {
    match text.parse::<i64>() {
        Ok(secs) => in_range(secs),
        Err(_) => NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok(),
    }
}

/// Ortszeit zu UTC-Sekunden, `None` außerhalb des erlaubten Bereichs (statt Abbruch am Kalenderende).
pub(crate) fn local_at(tz: Tz, secs: i64) -> Option<NaiveDateTime> {
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
    }

    #[test]
    fn the_edges_convert_even_in_the_furthest_zone() {
        assert!(local_at(Kiritimati, MAX_DUE_SECS).is_some());
        assert!(local_at(chrono_tz::Etc::GMTPlus12, 0).is_some());
    }
}
