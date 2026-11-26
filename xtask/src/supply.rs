//! Lieferketten-Ausnahmen (Regel `lieferkette:<crate>` oder `lieferkette:<crate>@<fassung>`, Datei `Cargo.lock`):
//! Ausnahmen ohne Code-Stelle, etwa zu Reife oder Alter einer Abhängigkeit. Sie zählen nicht als Code-Stelle, sind
//! aber rot, sobald das Paket (oder genau diese Fassung) nicht mehr im Lockfile steht, also nichts mehr deckt.
use crate::exceptions::{normalize_rule, Entry};

pub const PREFIX: &str = "lieferkette:";
pub const LOCKFILE: &str = "Cargo.lock";

/// Paare (Name, Fassung) aller `[[package]]`-Blöcke im Lockfile.
fn locked_packages(lockfile: &str) -> Vec<(String, String)> {
    let value = |line: &str, key: &str| {
        let rest = line.trim().strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
        Some(rest.strip_prefix('"')?.strip_suffix('"')?.to_string())
    };
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in lockfile.lines() {
        if line.trim() == "[[package]]" {
            name = None;
        } else if let Some(n) = value(line, "name") {
            name = Some(n);
        } else if let (Some(v), Some(n)) = (value(line, "version"), name.take()) {
            out.push((n, v));
        }
    }
    out
}

/// Probleme der `lieferkette:`-Zeilen gegen den Inhalt von `Cargo.lock` (leer = grün).
pub fn problems(entries: &[Entry], lockfile: &str) -> Vec<String> {
    let locked = locked_packages(lockfile);
    let mut out = Vec::new();
    for e in entries {
        let rule = normalize_rule(&e.rule);
        let Some(target) = rule.strip_prefix(PREFIX) else { continue };
        if e.path != LOCKFILE {
            out.push(format!("{}: Lieferketten-Ausnahme „{target}“ gehört zur Datei {LOCKFILE}", e.path));
            continue;
        }
        let (name, version) = target.split_once('@').map_or((target, None), |(n, v)| (n, Some(v)));
        let covered = !name.is_empty() && locked.iter().any(|(n, v)| n == name && version.is_none_or(|want| v == want));
        if !covered {
            out.push(format!("{LOCKFILE}: Lieferketten-Ausnahme „{target}“ deckt kein Paket im Lockfile"));
        }
    }
    out
}

#[cfg(test)]
#[path = "supply_tests.rs"]
mod tests;
