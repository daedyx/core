use super::*;

#[test]
fn ordinary_text_with_umlauts_emoji_and_line_breaks_is_plain() {
    assert!(plain("Neu: Größere Schrift, Fehler behoben 🙂\n\t- Punkt zwei", true));
    assert!(plain("Daedyx Tasks", false));
}

#[test]
fn line_breaks_are_only_allowed_where_asked() {
    assert!(!plain("a\nb", false));
    assert!(!plain("a\tb", false));
}

/// Review N-U1-5, Probe R1: alle Format-Zeichen (Kategorie Cf), Zeilen- und Absatztrenner und Steuerzeichen.
#[test]
fn invisible_format_marks_and_controls_are_refused() {
    for c in [
        '\u{0000}',
        '\u{001B}',
        '\u{007F}',
        '\u{0085}',
        '\u{00AD}',
        '\u{0600}',
        '\u{061C}',
        '\u{06DD}',
        '\u{070F}',
        '\u{180E}',
        '\u{200B}',
        '\u{200C}',
        '\u{200D}',
        '\u{200E}',
        '\u{200F}',
        '\u{2028}',
        '\u{2029}',
        '\u{202A}',
        '\u{202E}',
        '\u{2060}',
        '\u{2064}',
        '\u{2066}',
        '\u{2069}',
        '\u{206F}',
        '\u{FEFF}',
        '\u{FFF9}',
        '\u{FFFB}',
        '\u{110BD}',
        '\u{13430}',
        '\u{1BCA0}',
        '\u{1D173}',
        '\u{E0001}',
        '\u{E0041}',
        '\u{E007F}',
        '\r',
    ] {
        assert!(!plain(&format!("Text{c}Text"), true), "U+{:04X}", c as u32);
    }
}
