//! Schnelleingabe für Aufgaben: „Steuer morgen 15 Uhr !!“ → Titel, Fälligkeit, Priorität (und
//! Wiederholung). Reine Funktion ohne Datenbank; das Ergebnis passt zu `edit_tasks::create_task`.
//! Die Stichworte sind deutsch (wie bisher in der Tauri-Oberfläche).
use crate::quick_when::parse;
use chrono::NaiveDateTime;
use chrono_tz::Tz;
use serde_json::{json, Map, Value};

/// Längster Titel, den der Aufgaben-Kern annimmt.
const MAX_TITLE_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct QuickTask {
    pub title: String,
    /// Felder für `create_task` (immer mit `title`).
    pub fields: Map<String, Value>,
}

/// `!`, `!!` oder `!!!` als eigenes Wort setzt die Priorität 1–3 (das erste solche Wort zählt).
fn take_priority(words: &mut Vec<String>) -> Option<u8> {
    let i = words.iter().position(|w| (1..=3).contains(&w.len()) && w.bytes().all(|b| b == b'!'))?;
    let p = u8::try_from(words.get(i)?.len()).ok()?;
    words.remove(i);
    Some(p)
}

fn clip(text: &str) -> String {
    text.chars().take(MAX_TITLE_CHARS).collect::<String>().trim().to_string()
}

/// Zerlegt die Eingabe. `now` ist die Ortszeit der Anzeige, `tz` ihre Zone (für Fälligkeiten mit Uhrzeit).
/// `None`, wenn kein Titel übrig bleibt.
pub fn parse_quick_task(input: &str, now: NaiveDateTime, tz: Tz) -> Option<QuickTask> {
    let mut words: Vec<String> = input.split_whitespace().map(str::to_owned).collect();
    let mut fields = Map::new();
    if let Some(p) = take_priority(&mut words) {
        fields.insert("priority".into(), json!(p));
    }
    let today = now.date();
    let title = match parse(&words, today) {
        Ok(when) => {
            let title = clip(&when.rest.join(" "));
            let date = when.date.or(when.repeat.as_ref().map(|_| today));
            match (date, when.time) {
                (_, Some(t)) => {
                    let local = date.unwrap_or(today).and_time(t);
                    let secs = crate::repeat::local_to_utc(tz, local)?;
                    fields.insert("due".into(), json!(secs.to_string()));
                    fields.insert("due_has_time".into(), json!(true));
                }
                (Some(d), None) => {
                    fields.insert("due".into(), json!(d.format("%Y-%m-%d").to_string()));
                    fields.insert("due_has_time".into(), json!(false));
                }
                (None, None) => {}
            }
            if let Some(r) = when.repeat {
                fields.insert("repeat_rule".into(), r);
            }
            title
        }
        Err(()) => clip(&words.join(" ")),
    };
    if title.is_empty() {
        return None;
    }
    if !fields.contains_key("due") {
        fields.remove("due_has_time");
    }
    fields.insert("title".into(), json!(title));
    Some(QuickTask { title, fields })
}
