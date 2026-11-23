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
