# Sicherheit (daedyx-core)

Gekürzte Fassung des verbindlichen Sicherheitsrahmens der Daedyx-Suite für dieses Repository. Die Mutter-Fassung
liegt bei Daedyx Desk (`docs/SICHERHEIT.md`); bei Widerspruch gilt sie. Stand: 2026-11-23.

Grundsatz Björn: Keine Lücken, keine persönlichen Daten. Fehlerklassen werden durch Bauweise unmöglich gemacht,
alles Übrige vor jedem Merge automatisch gestoppt. Eine Garantie gibt es nicht; ehrlich bleiben.

## Kurzfassung (wortgleich in jeder CLAUDE.md)
- **Kein `unsafe`** (Workspace-Lint `forbid`). Ausnahme nur im Crate `daedyx-sys`, mit `// SAFETY:` und Eintrag in `docs/SICHERHEIT-AUSNAHMEN.md`.
- **Keine Panik im Normalpfad:** kein `unwrap`/`expect`/`panic!`/Index `a[i]`/Text-Schnitt; Clippy verbietet es (Tests ausgenommen). Panik-Texte ohne Daten.
- **Jede Eingabe von außen hat Grenzen** (Größe, Tiefe/Anzahl, Zeit) aus dem Modul `limits`. Jeder Parser hat ein Fuzz-Ziel und proptest. Missbrauchstests zuerst.
- **Netz nur über `daedyx-net`/`NetGate`** (nur https, Zeit- und Größengrenze, Zweck). Keine Telemetrie. Fremdbilder aus, Links nur nach Bestätigung, Anhänge nie öffnen.
- **Keine Shell, keine Prozesse mit Fremddaten;** Prozesse nur über `daedyx-sys`. Schreiben nur in den eigenen Datenordner oder an einen im Dialog gewählten Pfad (`UserChosenPath`).
- **Daten verschlüsselt** (SQLCipher), Schlüssel und Passwörter nur im Betriebssystem-Schlüsselbund, im Programm nur als `Secret` (kein Debug, zeroize). Keine Klartext-Kopien.
- **Keine Protokolle mit Inhalten, Namen, Adressen, Pfaden.** Kein println/eprintln/dbg. Diagnose nur als Codes und Zahlen.
- **Keine echten Daten** in Code, Tests, Commits, CI-Ausgaben, Bildschirmfotos, Chats. Testdaten nur aus `daedyx-fixtures` (example.com, .test, 192.0.2.x). Bildschirmfotos nur über `DAEDYX_SCREENSHOT` im Demo-Modus.
- **Keine Secrets** in Repos und Workflows; Veröffentlichung nur über Attestierung (`plan-ohne-secrets`). Neue Abhängigkeit nur mit Eintrag in `docs/ABHAENGIGKEITEN.md`, Version mindestens 14 Tage alt.
- **Nie eine Prüfung abschalten, um grün zu werden** (kein `#[allow]`, `ignore`, `#[ignore]`, `--no-verify`, `skip_tests` ohne Ausnahme-Eintrag und Björns Ja). Sicherheitsdateien (deny.toml, clippy.toml, [lints], Workflows, xtask scan) nur mit Björns Ja ändern.
- **Vor dem Bau:** Abschnitt im `docs/BEDROHUNGSMODELL.md`. **Vor dem Merge:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --locked`, `cargo deny check`, `cargo audit`, `cargo xtask scan`, dazu Sicherheits-Review durch einen anderen Chat. Im Abschlussbericht jede Prüfung mit Ergebnis nennen oder „ungeprüft“ schreiben.

## Was der Kern ist und was er von außen bekommt
Der Kern (`daedyx-tasks-core`) hat keine Oberfläche, kein Netz, keine eigene Datenbank und startet keine Prozesse.
Er bekommt eine Datenbank-Verbindung von der App. Fremde Eingaben sind:
- Änderungen aus der Oberfläche (JSON-Felder, geprüft in `edit_tasks.rs`/`validate.rs`, Grenzen dort),
- Datensätze aus dem Abgleich mit anderen Geräten (Stempel `Hlc`, Felder, Wiederholungsregeln),
- Inhalte der Datenbankdatei selbst (können von einer älteren Version oder beschädigt sein).

Ein `limits`-Modul gibt es hier noch nicht; die Grenzen stehen bei den Prüfungen (`MAX_LINKS`, `MAX_ALARMS`,
Längen in `validate.rs`, Tiefe 1000 in `tree.rs`). Zusammenziehen gehört zum Gerüst-Auftrag M2.

## Was in diesem Repository erzwungen ist
| Regel | Wie | Wo |
|---|---|---|
| Kein `unsafe` | `unsafe_code = "forbid"` (kompiliert nicht) | `Cargo.toml` `[workspace.lints]` |
| Keine Panik, keine Ausgaben | Clippy `deny`: unwrap, expect, panic, Index, Text-Schnitt, println/eprintln/dbg, exit u. a.; in Tests erlaubt | `Cargo.toml`, `clippy.toml` |
| Rechnen ohne Überlauf | Clippy `arithmetic_side_effects`/Umwandlungen; in CI mit `-D warnings` rot; Release mit `overflow-checks` | `Cargo.toml` |
| Keine Prozesse, kein Temp, kein Netz | `disallowed-methods` | `clippy.toml` |
| Lieferkette | `cargo deny` (Lücken, Lizenzen, Quellen, Verbote) und `cargo audit` | `deny.toml`, `.cargo/audit.toml` |
| Keine Geheimnisse, keine echten Daten | `cargo xtask scan` (Pre-Commit nur Vorgemerktes, CI alles) | `xtask/`, `.githooks/pre-commit` |
| Ausnahmen nur mit Eintrag | `cargo xtask exceptions` | `docs/SICHERHEIT-AUSNAHMEN.md` |
| Parser mit Eigenschaftstests | proptest, Missbrauchstests | `prop_tests.rs`, `docs/FUZZING.md` |
| Alles bei jedem Push und Pull Request | GitHub Actions, keine Secrets, nur Lesen | `.github/workflows/ci.yml` |

`cargo xtask scan` meldet: private Schlüssel und Token, Mailadressen außer `example.com/.org/.net`, `.example`,
`.test`, `.invalid`, `.localhost` und GitHub-noreply, Benutzerordner (`X:\Users\name`, `/Users/name`,
`/home/name`), private IP-Netze, Namen echter Organisationen, verbotene Dateitypen (Datenbanken, Mails, Kalender,
Kontakte, Schlüssel) außer unter `testdata/erfunden/`, Dateien über 1 MB, `CLAUDE.md` im Git und Secrets in
Workflows. Echte Namen stehen nur in einer **lokalen** Wortliste außerhalb des Repos
(`%APPDATA%\Daedyx\verboten.txt` oder Umgebungsvariable `DAEDYX_SCAN_WORDS`), nie in Git und nie in CI.
Der Fundtext wird nie ausgegeben, nur Datei, Zeile und Regel.

## Einmal je Klon
```
git config core.hooksPath .githooks
```

## Prüfbefehle (vor jedem Merge, Ergebnis im Bericht nennen)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny --locked check advisories bans licenses sources
cargo audit --deny warnings
cargo xtask scan
cargo xtask exceptions
```

## Meldungen von außen
Siehe `SECURITY.md`. Reaktionszeiten: Eingang binnen 7 Tagen; kritisch 7, hoch 30, mittel 90 Tage, niedrig mit
dem nächsten Release. Gleiche Fristen für neue RustSec-Meldungen zu unseren Abhängigkeiten.

## Noch offen (nicht erzwungen, ehrlich)
- `rust-toolchain.toml` mit fester Version: CI nagelt Rust 1.99.1 fest; lokal fehlt die Datei, weil rustup sonst
  eine zweite Kopie derselben Version herunterlädt (Björns Ja nötig).
- `gitleaks` und `zizmor`: nicht installiert (Herunterladen nur nach Björns Ja). Bis dahin deckt `cargo xtask scan`
  die Muster ab; der Verlauf (alte Commits) wird damit nicht geprüft.
- `cargo-fuzz` (nightly), wöchentlicher Lauf, Dependabot, CODEOWNERS, `xtask deps-check/fuzz-check/test-hygiene`,
  Erkennung von NAS- und Laufwerkspfaden im Scan: kommen mit dem Gerüst-Auftrag M2.
- Repository-Einstellungen (Private vulnerability reporting, SHA-Pflicht für Actions, Branch-Schutz): nur Björn.
