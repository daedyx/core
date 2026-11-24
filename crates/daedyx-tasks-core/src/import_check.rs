//! Prüfung fremder Werte und Stempel im Abgleich: dieselben Regeln wie die Eingabe (`edit_tasks`, `edit_alarms`), aber im
//! Speicherformat (Wahrheitswerte als 0/1, Regeln als JSON-Text). Ein ungültiger Wert wird verworfen (samt Stempel),
//! der lokale bleibt. `merge` selbst bleibt allgemein, damit neue Felder dort keine Änderung brauchen.
use super::due_time::valid_due_text;
use super::edit_alarms::MAX_MINUTES;
use super::hlc::canonical;
use super::merge::Record;
use super::repeat::valid_rule;
use super::types::RepeatRule;
use serde_json::Value;

fn int_in(v: &Value, lo: i64, hi: i64) -> bool {
    v.as_i64().is_some_and(|n| (lo..=hi).contains(&n))
}

fn rule_ok(v: &Value) -> bool {
    let parsed = match v {
        Value::String(s) => serde_json::from_str::<RepeatRule>(s).ok(),
        Value::Object(_) => serde_json::from_value::<RepeatRule>(v.clone()).ok(),
        _ => None,
    };
    parsed.is_some_and(|r| valid_rule(&r))
}

/// `true`, wenn der fremde Wert für dieses Feld zulässig ist. Felder ohne Regel hier gelten als zulässig.
pub(crate) fn acceptable(table: &str, field: &str, v: &Value) -> bool {
    match (table, field) {
        ("tasks", "due") => v.is_null() || v.as_str().is_some_and(valid_due_text),
        ("tasks", "due_has_time") => v.is_boolean() || int_in(v, 0, 1),
        ("tasks", "priority") => int_in(v, 0, 3),
        ("tasks", "remind_minutes") => v.is_null() || int_in(v, 0, MAX_MINUTES),
        ("tasks", "done_at") => v.is_null() || v.is_i64(),
        ("tasks", "repeat_rule") => v.is_null() || rule_ok(v),
        ("task_alarms", "minutes_before") => int_in(v, 0, MAX_MINUTES),
        _ => true,
    }
}

/// Fremde Stempel höchstens so weit vor der eigenen Zeit (sonst friert die eigene Uhr ein).
const MAX_AHEAD_MS: u64 = 86_400_000;

/// Stempel in genauer Textform und höchstens einen Tag nach `now_ms` (Review Kern-Fix M-1).
fn stamp_ok(stamp: &str, now_ms: u64) -> bool {
    canonical(stamp).is_some_and(|h| h.ms <= now_ms.saturating_add(MAX_AHEAD_MS))
}

/// Die fremde Änderung ohne unzulässige Felder und ohne Felder mit unzulässigem Stempel.
pub(crate) fn sanitized(table: &str, remote: Record, now_ms: u64) -> Record {
    let mut out = Record::default();
    for (f, v) in remote.fields {
        if let Some(c) = remote.clocks.get(&f).filter(|c| stamp_ok(c, now_ms) && acceptable(table, &f, &v)) {
            out.set(&f, v, c);
        }
    }
    out
}
