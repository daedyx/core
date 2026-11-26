//! „Reiner Text“ für Notiz und Registry-Schlüssel: keine Steuerzeichen, keine unsichtbaren Format-Zeichen
//! (Unicode-Kategorie Cf, darunter alle Richtungsmarken) und keine Zeilen- oder Absatztrenner außer `\n` und `\t`
//! (Review N-U1-5). Die Tabelle folgt Unicode 16; `std` kennt die Kategorien nicht, eine Abhängigkeit lohnt nicht.

/// Format-Zeichen (Cf) und die Trenner U+2028/U+2029.
const INVISIBLE: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{2028}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0001}', '\u{E0001}'),
    ('\u{E0020}', '\u{E007F}'),
];

/// `true`, wenn `s` reiner Text ist; `\n` und `\t` nur bei `allow_newline`.
pub(crate) fn plain(s: &str, allow_newline: bool) -> bool {
    s.chars().all(|c| {
        let control = c.is_control() && !(allow_newline && matches!(c, '\n' | '\t'));
        let invisible = INVISIBLE.iter().any(|&(lo, hi)| (lo..=hi).contains(&c));
        !control && !invisible
    })
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
