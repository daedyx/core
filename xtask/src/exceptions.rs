//! Ausnahmenzähler (SICHERHEIT Anhang B.4): Stellen im Code gegen `docs/SICHERHEIT-AUSNAHMEN.md`.
//! Rot, wenn eine Datei mehr oder weniger Stellen hat als Einträge, oder ein Eintrag abgelaufen ist.
use std::collections::BTreeMap;

pub const TABLE: &str = "docs/SICHERHEIT-AUSNAHMEN.md";

/// Muster werden zusammengesetzt, damit diese Datei sich nicht selbst zählt.
const RUST_MARKERS: [&str; 8] = [
    concat!("#[", "allow("),
    concat!("#![", "allow("),
    concat!("#[", "expect("),
    concat!("#![", "expect("),
    concat!("#[", "ignore"),
    concat!("unsafe", " {"),
    concat!("unsafe", " fn"),
    concat!("unsafe", " impl"),
];

/// Ein Eintrag der Tabelle: Datei und Ablaufdatum `JJJJ-MM-TT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub expires: String,
}

/// Zeilen `| Nr. | Datei | Regel | Grund | Entscheid | Datum | Ablauf |`; Kopf- und Trennzeilen zählen nicht.
pub fn parse_table(md: &str) -> Vec<Entry> {
    md.lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            let nr = cells.get(1)?;
            if nr.is_empty() || nr.starts_with("Nr") || nr.starts_with("---") {
                return None;
            }
            Some(Entry { path: cells.get(2)?.to_string(), expires: cells.get(7)?.to_string() })
        })
        .collect()
}

/// Stellen in Rust-Quelltext; Kommentarzeilen zählen nicht.
pub fn count_rust(text: &str) -> usize {
    text.lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with("//"))
        .map(|l| RUST_MARKERS.iter().map(|m| l.matches(m).count()).sum::<usize>())
        .sum()
}

/// Ausgenommene Meldungen in `deny.toml` (`id = "RUSTSEC-…"`, nicht auskommentiert).
pub fn count_deny(text: &str) -> usize {
    text.lines().map(str::trim_start).filter(|l| !l.starts_with('#') && l.contains(concat!("\"RUST", "SEC-"))).count()
}

/// Probleme (leer = grün). `found`: Datei → Anzahl Stellen (nur Dateien mit mindestens einer Stelle).
pub fn check(found: &BTreeMap<String, usize>, entries: &[Entry], today: &str) -> Vec<String> {
    let mut listed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    for e in entries {
        let n = listed.entry(e.path.as_str()).or_insert(0);
        *n = n.saturating_add(1);
        let valid_date = e.expires.len() == 10 && e.expires.bytes().filter(|c| *c == b'-').count() == 2;
        if !valid_date {
            problems.push(format!("{}: Ablaufdatum fehlt oder ist kein JJJJ-MM-TT", e.path));
        } else if e.expires.as_str() < today {
            problems.push(format!("{}: Ausnahme ist am {} abgelaufen", e.path, e.expires));
        }
    }
    let paths: std::collections::BTreeSet<&str> =
        found.keys().map(String::as_str).chain(listed.keys().copied()).collect();
    for p in paths {
        let (have, allowed) = (found.get(p).copied().unwrap_or(0), listed.get(p).copied().unwrap_or(0));
        if have != allowed {
            problems.push(format!("{p}: {have} Ausnahme-Stelle(n) im Code, {allowed} Eintrag/Einträge in {TABLE}"));
        }
    }
    problems
}

#[cfg(test)]
#[path = "exceptions_tests.rs"]
mod tests;
