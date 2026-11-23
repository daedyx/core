//! Kennungen für Listen, Abschnitte und Aufgaben.

/// Neue zufällige Kennung (16 Zufallsbytes als Hex, 32 Zeichen).
/// Ohne Zufall des Betriebssystems darf keine (vorhersagbare) Kennung entstehen; dann bricht das Programm ab.
/// Ausnahme A-001 in docs/SICHERHEIT-AUSNAHMEN.md (Panik-Text ohne Daten).
#[allow(clippy::expect_used, reason = "SICHERHEIT-AUSNAHMEN A-001")]
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("Zufallszahlen");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_32_hex_chars_and_differ() {
        let (a, b) = (new_id(), new_id());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
