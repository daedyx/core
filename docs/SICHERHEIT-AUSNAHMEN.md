# Sicherheits-Ausnahmen (daedyx-core)

Jede Ausnahme von einer Regel aus `docs/SICHERHEIT.md` steht hier: `#[allow(...)]` im Code, `ignore` in
`deny.toml`, `#[ignore]` an Tests, `unsafe`. Ohne Eintrag ist die Ausnahme nicht erlaubt. Die CI zählt die
Stellen im Code (`cargo xtask exceptions`) und vergleicht sie mit dieser Tabelle; mehr Stellen als Einträge
oder ein abgelaufener Eintrag sind rot.

| Nr. | Datei | Regel | Grund | Entscheid von | Datum | Ablauf |
|---|---|---|---|---|---|---|
| A-001 | crates/daedyx-tasks-core/src/ids.rs | clippy::expect_used | Liefert das Betriebssystem keinen Zufall, darf keine (vorhersagbare) Kennung entstehen. Abbruch ist sicherer als weiterzumachen. Der Panik-Text enthält keine Daten. Ersatz durch einen Fehlertyp ändert die öffentliche Schnittstelle (Desk, Tasks) und kommt mit dem Gerüst M2. | Bau-Chat, Björns Ja ausstehend | 2026-11-23 | 2027-03-31 |
| A-002 | xtask/src/files.rs | clippy::disallowed_methods | Das Prüfwerkzeug startet `git` mit festen Argumenten, um die versionierten bzw. vorgemerkten Dateien zu lesen. Keine Fremddaten in der Befehlszeile außer Dateinamen aus Git selbst (als eigenes Argument, nie über eine Shell). Läuft nur auf dem Entwicklerrechner und in CI, nie in einer App. | Bau-Chat, Björns Ja ausstehend | 2026-11-23 | 2027-11-23 |
| A-003 | crates/daedyx-tasks-core/src/edit_tasks.rs | clippy::too_many_arguments | Stil-Regel ohne Sicherheitswirkung, schon vor dem Rahmen vorhanden (`move_task_depth`). Umbau in eine Parameter-Struktur ändert die öffentliche Schnittstelle für Desk und Tasks. | Bau-Chat, Björns Ja ausstehend | 2026-11-23 | 2027-06-30 |

Format für `cargo xtask exceptions`: Spalte 2 ist der Pfad (genau wie im Repo), Spalte 7 ein Datum `JJJJ-MM-TT`.
Eine Datei mit zwei Ausnahmen bekommt zwei Zeilen.
