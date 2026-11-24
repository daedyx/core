# Sicherheits-Ausnahmen (daedyx-core)

Jede Ausnahme von einer Regel aus `docs/SICHERHEIT.md` steht hier: `#[allow(...)]` im Code, `ignore` in
`deny.toml`, `#[ignore]` an Tests, `unsafe`. Ohne Eintrag ist die Ausnahme nicht erlaubt. Dazu zählen auch
Lockerungen in `deny.toml`, `clippy.toml`, `.cargo/audit.toml` und Workflows. Die CI sucht die Stellen
(`cargo xtask exceptions`) und vergleicht sie mit dieser Tabelle; eine Stelle ohne passende Zeile, eine Zeile ohne
Stelle oder ein abgelaufener Eintrag sind rot.

| Nr. | Datei | Regel | Grund | Entscheid von | Datum | Ablauf |
|---|---|---|---|---|---|---|
| A-001 | crates/daedyx-tasks-core/src/ids.rs | clippy::expect_used | Liefert das Betriebssystem keinen Zufall, darf keine (vorhersagbare) Kennung entstehen. Abbruch ist sicherer als weiterzumachen. Der Panik-Text enthält keine Daten. Ersatz durch einen Fehlertyp ändert die öffentliche Schnittstelle (Desk, Tasks) und kommt mit dem Gerüst M2. | Bau-Chat, Ja des Betreuers 2026-11-24 | 2026-11-23 | 2027-03-31 |
| A-002 | xtask/src/files.rs | clippy::disallowed_methods | Das Prüfwerkzeug startet `git` mit festen Argumenten, um die versionierten bzw. vorgemerkten Dateien zu lesen. Keine Fremddaten in der Befehlszeile außer Dateinamen aus Git selbst (als eigenes Argument, nie über eine Shell). Läuft nur auf dem Entwicklerrechner und in CI, nie in einer App. | Bau-Chat, Ja des Betreuers 2026-11-24 | 2026-11-23 | 2027-11-23 |
| A-003 | crates/daedyx-tasks-core/src/edit_tasks.rs | clippy::too_many_arguments | Stil-Regel ohne Sicherheitswirkung, schon vor dem Rahmen vorhanden (`move_task_depth`). Umbau in eine Parameter-Struktur ändert die öffentliche Schnittstelle für Desk und Tasks. | Bau-Chat, Ja des Betreuers 2026-11-24 | 2026-11-23 | 2027-06-30 |
| A-004 | deny.toml | RUSTSEC-2026-0459 (libsqlite3-sys) | Lokale Lücke, braucht präparierte Datenbank- und WAL-Datei; der Kern bündelt SQLite nur in Tests. Die Apps bauen mit SQLCipher, und auch die neue libsqlite3-sys 0.39 bündelt dort noch ein betroffenes SQLite; ein Sprung schließt die Lücke also nicht. | Bau-Chat nach Empfehlung „haertung-libsqlite3-chrono“, Ja des Betreuers 2026-11-24 | 2026-11-23 | 2027-02-28 |
| A-005 | deny.toml | RUSTSEC-2026-0543 (chrono nicht gepflegt) | Keine bekannte Lücke, nur „nicht mehr gepflegt“. Ersatz (z. B. jiff) ist ein eigener Auftrag, weil Wiederholung und Erinnerungen daran hängen. | wie A-004 | 2026-11-23 | 2027-02-28 |
| A-006 | deny.toml | RUSTSEC-2026-0555 (chrono-tz nicht gepflegt) | wie A-005 | wie A-004 | 2026-11-23 | 2027-02-28 |
| A-007 | xtask/src/main.rs | clippy::print_stdout, clippy::print_stderr | Das Prüfwerkzeug meldet Ergebnisse auf der Konsole; das ist seine Aufgabe. Ausgegeben werden nur Datei, Zeile und Regel, nie der gefundene Text. Läuft nie in einer App. | Bau-Chat, Ja des Betreuers 2026-11-24 | 2026-11-23 | 2027-11-23 |

Eine Zeile mit Regel `scan:<regel>` (z. B. `scan:secret-im-workflow`) erlaubt genau diesen Fund des Scans in genau
dieser Datei bis zum Ablaufdatum; sie zählt nicht als Code-Stelle. Gedacht nur für Altlasten, deren Ablösung beschlossen ist.

Format für `cargo xtask exceptions`: Spalte 2 ist der Pfad (genau wie im Repo), Spalte 3 die Regel, Spalte 6 das
Eintragsdatum und Spalte 7 der Ablauf, beide `JJJJ-MM-TT`. Eine Datei mit zwei Ausnahmen bekommt zwei Zeilen.

Verglichen werden **Datei und Regel**, nicht nur die Anzahl. Die Regel ist der Text vor einer Erläuterung in Klammern
` (…)`; Groß-/Kleinschreibung, Leerzeichen und Backticks zählen nicht, mehrere Regeln eines Attributs stehen mit Komma
in einer Zeile (Reihenfolge egal). Mögliche Regeln:
- Rust: die Lints eines `allow`/`expect` (auch in `cfg_attr`, über mehrere Zeilen), z. B. `clippy::print_stdout,
  clippy::print_stderr`; `ignore`; `unsafe`; `cfg:<bedingung>` für Code, den Clippy in der CI nie sieht (Erlaubnisliste:
  `test`, `windows`, `clippy`, `debug_assertions`, `target_os`/`target_family = "windows"`, `feature`; alles andere,
  z. B. `cfg:not(windows)`, `cfg:not(clippy)`, `cfg:not(debug_assertions)`, `cfg:any()`, braucht eine Zeile); `build.rs`
  für jedes Build-Skript.
- `deny.toml`: die Kennung `RUSTSEC-…` einer Meldung; `advisories.ignore=<crate>`;
  `licenses.exceptions=<crate>:<Lizenz>+<Lizenz>`; jede Abweichung von der Grundliste im Werkzeug als
  `<abschnitt>.<schlüssel>=<wert>`, z. B. `advisories.yanked=warn`, `sources.allow-git=<adresse>`,
  `licenses.allow=<lizenz>`, `bans.skip=<crate>`, `bans.skip-tree=<crate>`, `bans.deny-fehlt=<crate>`,
  `bans.deny-feld=<crate>:<feld>`, `bans.deny-version=<crate>@<version>`.
- `clippy.toml`: `<schlüssel>=<wert>` außerhalb der Grundliste, `disallowed-methods-fehlt=<pfad>`.
- `.cargo/audit.toml`: alles außer `ignore`; `Cargo.toml`: zusätzliche `workspace.lints.<werkzeug>.<lint>=allow` und
  `manifest:<schlüssel>` für jeden Schlüssel außerhalb der Erlaubnisliste, z. B. `manifest:lib.test`,
  `manifest:lib.harness`, `manifest:test#1.harness`, `manifest:package.autotests`, `manifest:package.build`,
  `manifest:patch.crates-io.<crate>`, `manifest:replace.<crate>`, `manifest:workspace.exclude`,
  `manifest:profile.release.<schlüssel>[=<wert>]`.
- Workflows (`.github/workflows/*`): `workflow:<name>` für das Setzen von `RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC`,
  `RUSTC_BOOTSTRAP`, `RUSTUP_TOOLCHAIN`, `CLIPPY_CONF_DIR`, `CARGO_HOME`, `CARGO_BUILD_*`, `CARGO_TARGET_*`,
  `CARGO_PROFILE_*`, `CARGO_ALIAS_*` u. ä.; `workflow:--config`, `workflow:--cap-lints`, `workflow:cargo +`.

Ein Crate-Manifest ohne `[lints] workspace = true` oder mit eigenen Lint-Stufen ist die Stelle `manifest:lints` (nur
mit Eintrag, z. B. für ein `*-sys`-Crate mit `unsafe`). Ein `build.rs` zählt nur neben einem `Cargo.toml` (Build-Skript).

Ohne Ausnahme-Weg rot: Workspace-Lints schwächer als die Grundliste, eine zweite `clippy.toml`; in `.cargo/config.toml` jeder Schlüssel außer dem
`xtask`-Alias (z. B. `runner`, `rustc-wrapper`, `rustflags`, `linker`, `[env]`), jede weitere Cargo-Konfigurationsdatei
und jede `rust-toolchain`-Datei; mehrzeilige Texte (`"""`, `'''`) und doppelte Schlüssel in Prüf-TOML; Attribute aus
Makro-Platzhaltern (`#[$m]`). Der Ablauf liegt höchstens 12 Monate nach dem Eintragsdatum,
das Eintragsdatum nicht in der Zukunft.
