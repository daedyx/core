# daedyx-core

Gemeinsamer Rust-Kern der Daedyx-Suite (Cargo-Workspace).

- `crates/daedyx-tasks-core` — Aufgaben: Modell, Schema und Migration, Feldstempel (HLC), Abgleich (Outbox, Merge), Bearbeiten, Wiederholung, Smart-Listen, Erinnerungen (nur „was ist fällig“). Kein Tauri, öffnet keine Datenbank: alles nimmt `&rusqlite::Connection`. SQLCipher oder SQLite stellt die App ein.

## Schema-Stände (`task_meta.schema_version`)
| Stand | Inhalt |
|---|---|
| 1 | Listen, Aufgaben, Verlauf, Erinnerungs-Merker, Meta (bis dahin Desk-Migration 7) |
| 2 | Abschnitte (`task_sections`, `tasks.section_id`; bis dahin Desk-Migration 8) |
| 3 | Farben (`task_sections.color`, `tasks.color`, `#RRGGBB` oder NULL), `tasks.links` (JSON-Liste `{app, kind, ref, title}`), `task_lists.home` (`desk`/`tasks`/NULL), `task_lists.moved_at` (UTC-Sekunden), Tabelle `task_alarms` (mehrere Erinnerungen je Aufgabe, `edit_alarms::set_alarms`; `tasks.remind_minutes` bleibt die erste). Alle gestempelt. |

**Regel:** Jede Erhöhung von `SCHEMA_VERSION` braucht in Desk eine neue Migration mit demselben Aufruf plus Wächtertest.

`daedyx_tasks_core::migrate(&conn)` legt frisch an oder hebt an, erkennt Desk-Bestand ohne gespeicherten Stand an den Tabellen, lehnt neuere Stände ab (`TooNew`) und läuft in einem `SAVEPOINT` (auch innerhalb einer fremden Transaktion).

## Ohne Schema-Erhöhung (Logik)
- **Monatstag-Anker:** `RepeatRule.month_day` (JSON `monthDay`, 1–31) für monatlich/jährlich; kurze Monate nehmen den letzten Tag, der Anker bleibt. Ohne Anker altes Verhalten.
- **Unterbaum:** `tree.rs` (`depth_of`, `height_of`, `descendants`), `create_task_depth`/`move_task_depth` mit `max_depth` (Zyklusprüfung, TooDeep). `create_task`/`move_task` nutzen `DESK_MAX_DEPTH = 1`. Löschen und Listenwechsel erfassen alle Nachfahren; Abgleich (Export und Import) ordnet nach Tiefe.
- **Zeitbereich und Prüfungen:** `due` mit Uhrzeit (UTC-Sekunden) nur 1970 bis Ende 9999 (`0..=253402300799`), sonst `Invalid("due")`. Der Abgleich (`import_changes`) prüft `due`, `due_has_time`, `priority`, `remind_minutes`, `done_at`, `repeat_rule` und `task_alarms.minutes_before` nach denselben Regeln und verwirft ungültige fremde Werte (der lokale bleibt). `reminders::pending`/`tick` rechnen geprüft, überspringen Minuten außerhalb 0–525 600, Bezugszeiten außerhalb des Bereichs und Zeilen mit falschem Datentyp, ohne die übrigen Erinnerungen abzuschalten. `repeat::advance` gibt außerhalb des Bereichs `None` statt am Kalenderende abzubrechen. Ein Datum ohne Uhrzeit gilt ebenso nur 1970–9999. Öffentlicher Helfer für Oberflächen: `due_time` (`local_at`, `due_secs`, `due_date`, `valid_due_text`, `in_range`).
- **Abgleich-Stempel und Eltern:** `import_changes` nimmt nur Stempel in genauer Textform (12 + 4 Hex-Stellen) und höchstens einen Tag nach `now_ms` an, sonst wird das Feld verworfen und die eigene Uhr bleibt stehen. Ein voller Zähler rückt die Uhr eine Millisekunde weiter. Eine Zeile, deren Liste, Abschnitt oder Oberaufgabe fehlt, wartet (wird übersprungen), statt den Stapel abzubrechen; `import_changes_counted` gibt die Zahl der wartenden Zeilen zurück.

## Prüfen
```
cargo test
```
