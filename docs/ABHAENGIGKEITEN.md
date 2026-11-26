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

## daedyx-update
| Crate | Art | Zweck | Lizenz | Bauskript/Makro | Geprüft |
|---|---|---|---|---|---|
| attestation-verify =0.1.0 (exakt) | normal | GitHub-Nachweise (Sigstore) offline prüfen | MIT OR Apache-2.0 | – (`forbid(unsafe_code)`) | Spike S-ATT 2026-11-23 (Fälschungsmatrix, deny/audit grün), Version vom 2026-08-03; **U-A1** |
| serde 1, serde_json 1, thiserror 2 | normal | `entry.json`/`updates.json` lesen und schreiben, Fehlertypen | MIT OR Apache-2.0 | derive-Makros | wie daedyx-tasks-core |
| semver 1.0.28 | normal | Versionen vergleichen | MIT OR Apache-2.0 | – | Bestand aus `daedyx-launcher/daedyx-update` |
| sha2 0.11 | normal | SHA-256 beim Laden | MIT OR Apache-2.0 | – | Bestand Launcher; dieselbe Fassung zieht attestation-verify |
| hex 0.4 | normal | Prüfsummen als Text | MIT OR Apache-2.0 | – | Bestand Launcher |
| proptest 1.12 (nur `std`) | Test | Eigenschafts- und Missbrauchstests | MIT OR Apache-2.0 | – | wie daedyx-tasks-core |
| tempfile 3.27 | Test | Temporäre Ordner in Tests | MIT OR Apache-2.0 | – | Bestand Launcher |

Mittelbar über attestation-verify (Lockfile, Fassungen wie im Spike): RustCrypto (`p256`/`p384` 0.14, `ecdsa` 0.17,
`signature` 3.0, `der` 0.8, `spki` 0.8, `x509-cert` 0.3.1 u. a.), `base64` 0.23. Kein C-Code, kein Netz-Code.

### Lieferketten-Ausnahmen
Beide stehen als Zeilen `lieferkette:` in `docs/SICHERHEIT-AUSNAHMEN.md`; `cargo xtask exceptions` prüft sie gegen
`Cargo.lock` und das Ablaufdatum.

- **U-A1** – Regel „Abhängigkeiten reif und gepflegt“ (SICHERHEIT 4.1). `attestation-verify` 0.1.0 hat einen Autor und
  kein unabhängiges Audit. Ausgleich: exakte Version, Fälschungsmatrix als Dauertest (`attest_tests.rs`),
  Prüfsumme und Größe aus `entry.json` als zweite Prüfung, Ersatz `sigstore-verify` vorbereitet (nur `attest.rs`
  betroffen). `cargo vet` gibt es im Kern noch nicht (ungeprüft). Wiedervorlage alle 3 Monate oder bei
  Ankündigung von Rekor v2. Eingetragen 2026-11-26, Ablauf 2027-02-26. **Ja des Betreuers 2026-11-26.**
  **Bekannte Lücke (Review U1-R N-U1-1):** Die Bibliothek liest Auslöser (`build_trigger`) und Läufer
  (`runner_environment`) des Workflows, prüft sie aber nicht. Gebunden ist also „`release.yml` lief auf
  `refs/heads/main`“, nicht „nach Push oder Start von Hand auf einem GitHub-Läufer“. `release.yml` muss das selbst
  sicherstellen (U2: nur `push` und `workflow_dispatch`, nur GitHub-Läufer, Freigabe im Environment, CI-Wache).
  Bei der Wiedervorlage bei `attestation-verify` anfragen bzw. im Ersatz `sigstore-verify` selbst prüfen.
- **U-A2** – Regel „Version mindestens 14 Tage alt“ (SICHERHEIT 4.1). Der Plan sah `x509-cert` 0.3.0 vor; offline
  liegt nur 0.3.1 (vom 2026-11-20) im Cache, also nimmt das Lockfile 0.3.1. Sie ist am 2026-12-04 14 Tage alt; bis
  dahin gilt die Ausnahme. Eingetragen 2026-11-26, Ablauf 2026-12-04. **Ja des Betreuers 2026-11-26.**

## xtask
Keine Abhängigkeiten (nur Standardbibliothek).
