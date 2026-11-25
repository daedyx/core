# Fuzzing und Eigenschaftstests (daedyx-core)

SICHERHEIT 2.4 verlangt für jeden Code, der Fremdformate liest, Eigenschaftstests und ein Fuzz-Ziel.

## Heute: Eigenschafts- und Missbrauchstests (läuft überall, stabiles Rust)
`crates/daedyx-tasks-core/src/prop_tests.rs` (proptest) und `repeat_abuse_tests.rs`, Teil von `cargo test`:

| Eingang | Woher kommt Fremdes | Eigenschaft |
|---|---|---|
| `Hlc::parse` | Stempel aus dem Abgleich (andere Geräte) | nie Panik; Text → Stempel → Text verlustfrei |
| `check_color`, `check_links` | Felder aus der Oberfläche und dem Abgleich | nie Panik; Ergebnis ist NULL oder normierter Text |
| `update_task`/`create_task` mit beliebigem JSON | Patches der Oberfläche | nie Panik, höchstens Fehler |
| `next_date`, `advance` | Wiederholungsregel aus Datenbank/Abgleich (beliebige Zahlen, Wochentage, Daten bis Kalenderende) | nie Panik, nie Hängen, Ergebnis liegt später; Wochenregel gleich dem früheren Tag-für-Tag-Verfahren |

Funde werden **zuerst** als fester Test eingefroren (wie `repeat_abuse_tests.rs`), dann behoben.

## daedyx-update (Eingänge aus dem Netz)
`crates/daedyx-update/src/prop_tests.rs` (proptest, Teil von `cargo test`) und die Fälschungsmatrix `attest_tests.rs`:

| Eingang | Eigenschaft |
|---|---|
| `entry::parse` (`entry.json`, ≤ 64 KB) | nie Panik bei beliebigen Bytes; ein angenommener Eintrag liegt immer auf dem eigenen Release-Pfad, Größe 1 bis 512 MB, Version ohne Vorabkennung |
| `GithubAttestation::verify` (Bündel, ≤ 256 KB) | nie Panik bei beliebigen Bytes und bei jedem gekippten Bit des echten Bündels; Zufallsbündel werden nie angenommen |
| `fetch::HttpsUrl::parse` (Weiterleitungen) | nie Panik; angenommene Rechnernamen nur `a-z0-9.-` |
| `version::parse`/`decide` | nie Panik; „neuer“ heißt immer größer als installiert und nicht unter der höchsten gesehenen |
| `UpdateState::due_for_auto_check` | keine Überläufe bei beliebigen Zeiten |

Fuzz-Ziele für später liegen als Eingänge bereit (`crates/daedyx-update/src/fuzzing.rs`, Merkmal `fuzzing`):
`catalog_entry`, `attest_bundle`, `version_compare`, `redirect_url`. Ein `fuzz/`-Crate (libfuzzer-sys) braucht einen
Download und nightly (siehe unten) und ist deshalb noch nicht angelegt.

## Später: cargo-fuzz (braucht Rust nightly)
`cargo-fuzz` arbeitet mit libFuzzer und braucht die nightly-Toolchain; beides ist auf Björns Rechner nicht
installiert, und unter Windows/MSVC ist es ungeprüft. Deshalb hier nur dokumentiert, nicht eingebaut.
Einführung (nach Björns Ja zum Herunterladen, SICHERHEIT 8.3 E3), Linux-Läufer in CI:
```
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
cargo +nightly fuzz init           # legt fuzz/ an
cargo +nightly fuzz run hlc_parse -- -max_total_time=60
```
Erste Ziele: `hlc_parse`, `repeat_rule` (JSON → `RepeatRule` → `advance`), `task_patch` (JSON → `update_task`).
Liste der Ziele dann in `fuzz/ZIELE.md`, Kurzlauf 60 s je Ziel in CI, Langlauf wöchentlich.
