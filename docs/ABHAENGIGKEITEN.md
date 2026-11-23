# Abhängigkeiten (daedyx-core)

Jede direkte Abhängigkeit mit Zweck, Lizenz und Prüfung (SICHERHEIT 4.1). Neue Versionen erst, wenn sie
mindestens 14 Tage alt sind (Ausnahme: Sicherheits-Updates). Name und Version werden auf crates.io
nachgesehen, nicht aus dem Gedächtnis übernommen.

## daedyx-tasks-core
| Crate | Art | Zweck | Lizenz | Bauskript/Makro | Geprüft |
|---|---|---|---|---|---|
| rusqlite 0.37 | normal | Zugriff auf die Datenbank der App (SQLCipher oder SQLite stellt die App ein) | MIT | libsqlite3-sys hat `build.rs` | Bestand, vor dem Rahmen |
| serde 1 | normal | Typen zu/aus JSON | MIT OR Apache-2.0 | derive-Makro | Bestand |
| serde_json 1 | normal | JSON lesen und schreiben | MIT OR Apache-2.0 | – | Bestand |
| thiserror 2 | normal | Fehlertypen | MIT OR Apache-2.0 | derive-Makro | Bestand |
| chrono 0.4 | normal | Datum und Uhrzeit (Wiederholung, Erinnerung) | MIT OR Apache-2.0 | – | Bestand; nicht mehr gepflegt (A-005) |
| chrono-tz 0.10 | normal | Zeitzonen | MIT OR Apache-2.0 | `build.rs` | Bestand; nicht mehr gepflegt (A-006) |
| getrandom 0.3 | normal | Zufall des Betriebssystems für Kennungen | MIT OR Apache-2.0 | – | Bestand |
| rusqlite 0.37 `bundled` | Test | SQLite nur für Tests | MIT | `build.rs` | Bestand |
| proptest 1.12 (ohne Standard-Merkmale, nur `std`) | Test | Eigenschafts- und Missbrauchstests (SICHERHEIT 2.4) | MIT OR Apache-2.0 | – | 2026-11-23: Version 1.12.0 vom 2026-10-10 (über 14 Tage), crates.io `proptest`, Repo proptest-rs/proptest |

## xtask
Keine Abhängigkeiten (nur Standardbibliothek).
