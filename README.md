# daedyx-core

Gemeinsamer Rust-Kern der Daedyx-Suite (Cargo-Workspace).

- `crates/daedyx-tasks-core` — Aufgaben: Modell, Schema und Migration, Feldstempel (HLC), Abgleich (Outbox, Merge), Bearbeiten, Wiederholung, Smart-Listen, Erinnerungen (nur „was ist fällig“). Kein Tauri, öffnet keine Datenbank: alles nimmt `&rusqlite::Connection`. SQLCipher oder SQLite stellt die App ein.

## Schema-Stände (`task_meta.schema_version`)
| Stand | Inhalt |
|---|---|
| 1 | Listen, Aufgaben, Verlauf, Erinnerungs-Merker, Meta (bis dahin Desk-Migration 7) |
| 2 | Abschnitte (`task_sections`, `tasks.section_id`; bis dahin Desk-Migration 8) |

`daedyx_tasks_core::migrate(&conn)` legt frisch an oder hebt an, erkennt Desk-Bestand ohne gespeicherten Stand an den Tabellen, lehnt neuere Stände ab (`TooNew`) und läuft in einem `SAVEPOINT` (auch innerhalb einer fremden Transaktion).

## Prüfen
```
cargo test
```
