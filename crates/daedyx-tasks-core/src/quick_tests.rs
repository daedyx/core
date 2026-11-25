//! Schnelleingabe für Aufgaben (vorher `app/src/tasks/quickEntry.ts` und `calendar/quickEntry.ts`).
use crate::quick::{parse_quick_task, QuickTask};
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Europe::Berlin;
use serde_json::json;

/// Mittwoch, 18.11.2026, 10:00 Ortszeit
fn now() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 18).unwrap().and_hms_opt(10, 0, 0).unwrap()
}

fn q(text: &str) -> Option<QuickTask> {
    parse_quick_task(text, now(), Berlin)
}

fn field(text: &str, name: &str) -> serde_json::Value {
    q(text).unwrap().fields.get(name).cloned().unwrap_or(serde_json::Value::Null)
}

fn utc(y: i32, m: u32, d: u32, h: u32, mi: u32) -> String {
    Berlin.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap().timestamp().to_string()
}

#[test]
fn nur_ein_titel_bleibt_ein_titel() {
    let t = q("Müll rausbringen").unwrap();
    assert_eq!(t.title, "Müll rausbringen");
    assert_eq!(serde_json::Value::Object(t.fields), json!({ "title": "Müll rausbringen" }));
}

#[test]
fn leere_eingabe_ergibt_nichts() {
    assert!(q("   ").is_none());
    assert!(q("!!").is_none());
    assert!(q("").is_none());
}

#[test]
fn ausrufezeichen_setzen_die_prioritaet() {
    assert_eq!(field("Steuer !!!", "priority"), json!(3));
    assert_eq!(q("Steuer !!!").unwrap().title, "Steuer");
    assert_eq!(field("! Steuer", "priority"), json!(1));
    assert_eq!(q("Hallo!").unwrap().title, "Hallo!");
    assert_eq!(field("Hallo!", "priority"), serde_json::Value::Null);
}

#[test]
fn ein_tag_macht_eine_ganztaegige_faelligkeit() {
    assert_eq!(field("Steuer morgen", "due"), json!("2026-11-19"));
    assert_eq!(field("Steuer morgen", "due_has_time"), json!(false));
    assert_eq!(q("Steuer morgen").unwrap().title, "Steuer");
    assert_eq!(field("Ablesen heute", "due"), json!("2026-11-18"));
    assert_eq!(field("Ablesen übermorgen", "due"), json!("2026-11-20"));
}

#[test]
fn tag_mit_uhrzeit_macht_eine_faelligkeit_in_utc_sekunden() {
    let t = q("Anruf morgen 15:30 !!").unwrap();
    assert_eq!(t.title, "Anruf");
    assert_eq!(t.fields.get("due"), Some(&json!(utc(2026, 11, 19, 15, 30))));
    assert_eq!(t.fields.get("due_has_time"), Some(&json!(true)));
    assert_eq!(t.fields.get("priority"), Some(&json!(2)));
    assert_eq!(field("Zahnarzt am 3.12. um 9 Uhr", "due"), json!(utc(2026, 12, 3, 9, 0)));
    assert_eq!(q("Zahnarzt am 3.12. um 9 Uhr").unwrap().title, "Zahnarzt");
}

#[test]
fn uhrzeit_ohne_tag_gilt_heute() {
    assert_eq!(field("Rückruf 16 Uhr", "due"), json!(utc(2026, 11, 18, 16, 0)));
}

#[test]
fn wochentage_meinen_das_naechste_mal_nie_heute() {
    assert_eq!(field("Bericht Freitag", "due"), json!("2026-11-20"));
    assert_eq!(field("Bericht am Mittwoch", "due"), json!("2026-11-25"));
    assert_eq!(field("Bericht nächsten Montag", "due"), json!("2026-11-23"));
}

#[test]
fn datum_mit_monatsnamen_und_jahr() {
    assert_eq!(field("Frist 5. Januar 2027", "due"), json!("2027-01-05"));
    assert_eq!(field("Frist 24.12.2026", "due"), json!("2026-12-24"));
    assert_eq!(field("Frist 1.3.27", "due"), json!("2027-03-01"));
    // ohne Jahr: das nächste Mal (vergangene Tage springen ins Folgejahr)
    assert_eq!(field("Frist 2.1.", "due"), json!("2027-01-02"));
}

#[test]
fn wiederholungen() {
    let t = q("Gießen jeden Montag").unwrap();
    assert_eq!(t.title, "Gießen");
    assert_eq!(t.fields.get("repeat_rule"), Some(&json!({ "kind": "weekly", "interval": 1, "weekdays": [1] })));
    assert_eq!(t.fields.get("due"), Some(&json!("2026-11-23")));
    assert_eq!(field("Backup täglich", "repeat_rule"), json!({ "kind": "daily", "interval": 1 }));
    assert_eq!(field("Backup täglich", "due"), json!("2026-11-18"), "Wiederholung ohne Tag beginnt heute");
    assert_eq!(field("Miete monatlich", "repeat_rule"), json!({ "kind": "monthly", "interval": 1 }));
    assert_eq!(field("Steuer jedes Jahr", "repeat_rule"), json!({ "kind": "yearly", "interval": 1 }));
}

#[test]
fn fuellwoerter_verschwinden_aus_dem_titel() {
    assert_eq!(q("Abgabe bis Freitag").unwrap().title, "Abgabe");
}

#[test]
fn unmoegliche_angaben_bleiben_text() {
    assert_eq!(q("Termin 31.2.").unwrap().title, "Termin 31.2.");
    assert_eq!(field("Termin 31.2.", "due"), serde_json::Value::Null);
    assert_eq!(q("Treffen um 25 Uhr").unwrap().title, "Treffen um 25 Uhr");
    assert_eq!(q("Treffen 10:61").unwrap().title, "Treffen 10:61");
}

#[test]
fn nur_ein_datum_ohne_titel_ergibt_nichts() {
    assert!(q("morgen 15 Uhr").is_none());
}

#[test]
fn fremde_und_riesige_eingaben_stuerzen_nicht_ab() {
    for text in ["\u{1F600} morgen", "99999999999999999999.12.", "1.1.99999999999", "!!!!", "am am am", "ẞ İ morgen"]
    {
        let _ = q(text);
    }
    let long = "x ".repeat(20_000);
    assert!(q(&long).is_some_and(|t| t.title.chars().count() <= 500));
}

/// Review M5a N-4 / M6a N-1: Füllwörter fallen nur direkt vor einer erkannten Angabe weg.
#[test]
fn fuellwoerter_im_titel_bleiben_stehen() {
    assert_eq!(q("Kuchen für Oma morgen").unwrap().title, "Kuchen für Oma");
    assert_eq!(q("Brief an den Chef Freitag").unwrap().title, "Brief an den Chef");
    assert_eq!(q("Treffen am Montag um 15:00").unwrap().title, "Treffen");
    assert_eq!(q("Abgabe bis morgen").unwrap().title, "Abgabe");
    assert_eq!(q("Wartung von 3.12.").unwrap().title, "Wartung");
}
