//! Feldweise Zusammenführung (last-writer-wins je Feld). Arbeitet **generisch** über eine Tabelle von Feldern,
//! damit neue Felder (z. B. die Zuständigen der späteren Zusammenarbeit) keine Änderung hier brauchen.
//! Der Grabstein ist ein ganz normales Feld (`deleted_at`): Löschen und Wiederbeleben gewinnen nur mit neuerem Stempel.
use serde_json::Value;
use std::collections::BTreeMap;

/// Ein abgleichbares Objekt: Feldwerte und je Feld der Stempel (Text einer `Hlc`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Record {
    pub fields: BTreeMap<String, Value>,
    pub clocks: BTreeMap<String, String>,
}

impl Record {
    pub fn set(&mut self, field: &str, value: Value, clock: &str) {
        self.fields.insert(field.to_string(), value);
        self.clocks.insert(field.to_string(), clock.to_string());
    }
}

/// Je Feld gewinnt der größere Stempel; fehlt er auf einer Seite, gewinnt die andere. Gleiche Stempel (gleiche
/// Änderung) ergeben denselben Wert. Das Ergebnis hängt nicht von der Reihenfolge der Argumente ab.
pub fn merge(a: &Record, b: &Record) -> Record {
    let mut out = Record::default();
    let names: std::collections::BTreeSet<&String> = a.clocks.keys().chain(b.clocks.keys()).collect();
    for name in names {
        let pick = match (a.clocks.get(name), b.clocks.get(name)) {
            (Some(x), Some(y)) if y > x => b,
            (Some(_), _) => a,
            (None, _) => b,
        };
        if let (Some(v), Some(c)) = (pick.fields.get(name), pick.clocks.get(name)) {
            out.fields.insert(name.clone(), v.clone());
            out.clocks.insert(name.clone(), c.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(items: &[(&str, Value, &str)]) -> Record {
        let mut r = Record::default();
        for (f, v, c) in items {
            r.set(f, v.clone(), c);
        }
        r
    }

    const T1: &str = "000000000001-0000-a";
    const T2: &str = "000000000002-0000-b";
    const T3: &str = "000000000003-0000-a";

    #[test]
    fn newer_stamp_wins_per_field_and_other_fields_are_kept() {
        let a = rec(&[("title", json!("Milch"), T1), ("notes", json!("bio"), T3)]);
        let b = rec(&[("title", json!("Hafermilch"), T2), ("priority", json!(2), T2)]);
        let m = merge(&a, &b);
        assert_eq!(m.fields["title"], json!("Hafermilch"));
        assert_eq!(m.fields["notes"], json!("bio"));
        assert_eq!(m.fields["priority"], json!(2));
        assert_eq!(m.clocks["title"], T2);
    }

    #[test]
    fn a_newer_tombstone_beats_older_edits_but_a_newer_edit_does_not_revive() {
        let live = rec(&[("title", json!("A"), T1), ("deleted_at", Value::Null, T1)]);
        let gone = rec(&[("deleted_at", json!(99), T2)]);
        let m = merge(&live, &gone);
        assert_eq!(m.fields["deleted_at"], json!(99));
        let edited = rec(&[("title", json!("B"), T3)]);
        let m = merge(&m, &edited);
        assert_eq!(m.fields["deleted_at"], json!(99));
        assert_eq!(m.fields["title"], json!("B"));
        let revived = rec(&[("deleted_at", Value::Null, "000000000004-0000-b")]);
        assert_eq!(merge(&m, &revived).fields["deleted_at"], Value::Null);
    }

    #[test]
    fn unknown_future_fields_merge_without_code_changes() {
        let a = rec(&[("assignee_id", json!("p1"), T1)]);
        let b = rec(&[("assignee_id", json!("p2"), T2)]);
        assert_eq!(merge(&a, &b).fields["assignee_id"], json!("p2"));
    }

    #[test]
    fn merge_is_commutative_idempotent_and_associative() {
        let stamps = [T1, T2, T3, "000000000002-0000-a"];
        let mut recs = Vec::new();
        for (i, s) in stamps.iter().enumerate() {
            recs.push(rec(&[("title", json!(format!("t{i}")), s), ("notes", json!(i), stamps[(i + 1) % 4])]));
            recs.push(rec(&[("priority", json!(i), s)]));
        }
        for a in &recs {
            assert_eq!(merge(a, a), *a);
            for b in &recs {
                assert_eq!(merge(a, b), merge(b, a));
                for c in &recs {
                    assert_eq!(merge(&merge(a, b), c), merge(a, &merge(b, c)));
                }
            }
        }
    }
}
