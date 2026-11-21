//! Prüfung der Felder des Stands 3: Farbe, Heimat-App und Verweise.
use super::errors::TaskError;
use super::types::Link;
use serde_json::{json, Value};

pub const MAX_LINKS: usize = 50;

/// `#RRGGBB` (hex, beide Schreibweisen) oder NULL. Gespeichert wird in Großbuchstaben.
pub fn check_color(v: &Value) -> Result<Value, TaskError> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::String(s) if s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit()) => {
            Ok(json!(s.to_ascii_uppercase()))
        }
        _ => Err(TaskError::Invalid("color")),
    }
}

/// `desk`, `tasks` oder NULL (= Standard-App).
pub fn check_home(home: Option<&str>) -> Result<Value, TaskError> {
    match home {
        None => Ok(Value::Null),
        Some(h @ ("desk" | "tasks")) => Ok(json!(h)),
        Some(_) => Err(TaskError::Invalid("home")),
    }
}

/// NULL oder Liste von `{app, kind, ref, title}` (nur Texte, begrenzt). Gespeichert als JSON-Text; eine leere Liste wird NULL.
pub fn check_links(v: &Value) -> Result<Value, TaskError> {
    let bad = || Err(TaskError::Invalid("links"));
    let items = match v {
        Value::Null => return Ok(Value::Null),
        Value::Array(a) if a.len() <= MAX_LINKS => a,
        _ => return bad(),
    };
    let mut out = Vec::new();
    for it in items {
        let Ok(l) = serde_json::from_value::<Link>(it.clone()) else { return bad() };
        let ok = |s: &str, max: usize| !s.trim().is_empty() && s.chars().count() <= max;
        if !(ok(&l.app, 40) && ok(&l.kind, 40) && ok(&l.r#ref, 500) && l.title.chars().count() <= 200) {
            return bad();
        }
        out.push(l);
    }
    if out.is_empty() {
        return Ok(Value::Null);
    }
    Ok(json!(serde_json::to_string(&out).expect("links")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_hex_or_null() {
        assert_eq!(check_color(&json!("#aabbcc")).unwrap(), json!("#AABBCC"));
        assert_eq!(check_color(&Value::Null).unwrap(), Value::Null);
        for bad in ["red", "#abc", "#GGGGGG", "aabbcc", "#aabbccdd", ""] {
            assert!(check_color(&json!(bad)).is_err(), "{bad}");
        }
        assert!(check_color(&json!(5)).is_err());
    }

    #[test]
    fn home_is_desk_tasks_or_none() {
        assert_eq!(check_home(Some("desk")).unwrap(), json!("desk"));
        assert_eq!(check_home(None).unwrap(), Value::Null);
        assert!(check_home(Some("server")).is_err());
    }

    #[test]
    fn links_are_checked_and_stored_as_text() {
        let ok = json!([{"app": "desk", "kind": "mail", "ref": "k/m", "title": "Betreff"}]);
        let stored = check_links(&ok).unwrap();
        assert!(stored.as_str().unwrap().contains("\"ref\":\"k/m\""));
        assert_eq!(check_links(&json!([])).unwrap(), Value::Null);
        assert_eq!(check_links(&Value::Null).unwrap(), Value::Null);
        for bad in [
            json!("x"),
            json!([{"app": "desk"}]),
            json!([{"app": "", "kind": "mail", "ref": "k", "title": ""}]),
            json!([{"app": "desk", "kind": "mail", "ref": "k", "title": "t".repeat(201)}]),
            json!([{"app": "desk", "kind": "mail", "ref": "k", "title": "t", "extra": 1}]),
        ] {
            assert!(check_links(&bad).is_err(), "{bad}");
        }
        let many: Vec<Value> = (0..51).map(|i| json!({"app": "desk", "kind": "mail", "ref": i.to_string(), "title": ""})).collect();
        assert!(check_links(&json!(many)).is_err());
    }
}
