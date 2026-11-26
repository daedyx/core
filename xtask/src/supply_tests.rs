use super::*;
use crate::exceptions::{check, parse_table};

const LOCK: &str = "\
version = 4

[[package]]
name = \"attestation-verify\"
version = \"0.1.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"x509-cert\"
version = \"0.3.1\"
";

fn table(rows: &str) -> Vec<crate::exceptions::Entry> {
    parse_table(rows)
}

#[test]
fn a_row_for_a_crate_in_the_lockfile_is_valid_and_needs_no_code_site() {
    let e = table(
        "\
| U-A1 | Cargo.lock | `lieferkette:attestation-verify` (Reife) | Grund | Betreuer | 2026-11-26 | 2027-02-26 |
| U-A2 | Cargo.lock | lieferkette:x509-cert@0.3.1 (Alter) | Grund | Betreuer | 2026-11-26 | 2026-12-04 |
",
    );
    assert!(problems(&e, LOCK).is_empty(), "{:?}", problems(&e, LOCK));
    assert!(check(&[], &e, "2026-11-27").is_empty(), "keine Code-Stelle nötig");
}

#[test]
fn a_row_without_its_crate_or_version_in_the_lockfile_is_red() {
    for row in [
        "| U-9 | Cargo.lock | lieferkette:fehlt-ganz | G | B | 2026-11-26 | 2027-01-01 |",
        "| U-9 | Cargo.lock | lieferkette:x509-cert@0.3.0 | G | B | 2026-11-26 | 2027-01-01 |",
        "| U-9 | Cargo.lock | lieferkette:x509 | G | B | 2026-11-26 | 2027-01-01 |",
        "| U-9 | Cargo.lock | lieferkette: | G | B | 2026-11-26 | 2027-01-01 |",
        "| U-9 | src/lib.rs | lieferkette:x509-cert | G | B | 2026-11-26 | 2027-01-01 |",
    ] {
        assert_eq!(problems(&table(row), LOCK).len(), 1, "{row}");
    }
}

#[test]
fn an_expired_supply_chain_row_is_red_like_every_other() {
    let e = table("| U-A2 | Cargo.lock | lieferkette:x509-cert@0.3.1 | G | B | 2026-11-26 | 2026-12-04 |");
    assert!(check(&[], &e, "2026-12-04").is_empty());
    assert_eq!(check(&[], &e, "2026-12-05").len(), 1);
}

#[test]
fn other_rows_are_ignored_here() {
    let e = table("| A-1 | src/a.rs | clippy::expect_used | G | B | 2026-11-23 | 2027-03-31 |");
    assert!(problems(&e, "").is_empty());
}
