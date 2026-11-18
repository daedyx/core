//! Zeitzone aus dem Namen der Anzeige-Einstellung.
use chrono_tz::Tz;

/// Unbekannte oder leere Namen gelten als UTC.
pub fn zone(name: &str) -> Tz {
    name.parse().unwrap_or(chrono_tz::UTC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_names_parse_and_unknown_fall_back_to_utc() {
        assert_eq!(zone("Europe/Berlin"), chrono_tz::Europe::Berlin);
        assert_eq!(zone("Nirgendwo/Stadt"), chrono_tz::UTC);
        assert_eq!(zone(""), chrono_tz::UTC);
    }
}
