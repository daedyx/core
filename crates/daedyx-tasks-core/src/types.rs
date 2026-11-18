//! Typen zwischen Kern und Oberfläche (JSON, camelCase).
use serde::{Deserialize, Serialize};

/// Wiederholungsart; die Regel selbst steht als JSON in `tasks.repeat_rule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepeatKind {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// Wiederholung: alle `interval` Einheiten; bei `Weekly` optional feste Wochentage (1 = Montag … 7 = Sonntag).
/// `from_done`: nächste Fälligkeit zählt ab Erledigung statt ab bisheriger Fälligkeit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatRule {
    pub kind: RepeatKind,
    pub interval: u32,
    #[serde(default)]
    pub weekdays: Vec<u8>,
    #[serde(default)]
    pub from_done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskList {
    pub id: String,
    pub name: String,
    pub color: String,
    pub position: f64,
    /// Für die spätere Zusammenarbeit vorbereitet, heute immer `None`.
    pub owner_id: Option<String>,
}

/// Abschnitt (Spalte im Board) einer Liste.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub list_id: String,
    pub name: String,
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub parent_id: Option<String>,
    /// `None` = „Ohne Abschnitt“. Unteraufgaben haben keinen eigenen Abschnitt.
    pub section_id: Option<String>,
    pub title: String,
    pub notes: String,
    /// `YYYY-MM-DD`, oder UTC-Sekunden als Text bei `due_has_time`.
    pub due: Option<String>,
    pub due_has_time: bool,
    /// 0 keine, 1 niedrig, 2 mittel, 3 hoch.
    pub priority: u8,
    pub done_at: Option<i64>,
    pub position: f64,
    pub repeat: Option<RepeatRule>,
    pub remind_minutes: Option<i64>,
    /// Für die spätere Zusammenarbeit vorbereitet, heute immer `None`.
    pub assignee_id: Option<String>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_rule_uses_camel_case_and_defaults() {
        let r: RepeatRule = serde_json::from_str(r#"{"kind":"weekly","interval":2,"weekdays":[1,3]}"#).unwrap();
        assert_eq!(r, RepeatRule { kind: RepeatKind::Weekly, interval: 2, weekdays: vec![1, 3], from_done: false });
        let r: RepeatRule = serde_json::from_str(r#"{"kind":"daily","interval":1,"fromDone":true}"#).unwrap();
        assert!(r.from_done && r.weekdays.is_empty());
        assert!(serde_json::to_string(&r).unwrap().contains("fromDone"));
    }

    #[test]
    fn task_serializes_collaboration_fields_as_null() {
        let t = Task {
            id: "t".into(),
            list_id: "l".into(),
            parent_id: None,
            section_id: None,
            title: "Einkaufen".into(),
            notes: String::new(),
            due: None,
            due_has_time: false,
            priority: 0,
            done_at: None,
            position: 1.0,
            repeat: None,
            remind_minutes: None,
            assignee_id: None,
            created_by: None,
        };
        let v = serde_json::to_value(&t).unwrap();
        assert!(v["assigneeId"].is_null() && v["createdBy"].is_null() && v["listId"] == "l");
        assert!(v["sectionId"].is_null());
    }
}
