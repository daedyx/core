//! Eingänge für Fuzz-Ziele (docs/FUZZING.md, SICHERHEIT 2.4), nur mit Merkmal `fuzzing`. Jede Funktion nimmt
//! beliebige Bytes und darf nie abstürzen oder hängen; Ergebnisse werden verworfen. Dieselben Eingänge prüft
//! `prop_tests` heute schon mit proptest.
use crate::attest::{GithubAttestation, Provenance};
use crate::policy::{Platform, UpdatePolicy};
use crate::{entry, fetch, version};

fn policy() -> Option<UpdatePolicy> {
    UpdatePolicy::new("daedyx", "fuzz", 1, 1, "fuzz", Platform::WindowsX64).ok()
}

/// Ziel `catalog_entry`: `entry.json` lesen.
pub fn catalog_entry(data: &[u8]) {
    if let Some(p) = policy() {
        let _ = entry::parse(data, &p);
    }
}

/// Ziel `attest_bundle`: beliebiges Bündel gegen eine feste Prüfsumme prüfen.
pub fn attest_bundle(data: &[u8]) {
    if let Some(Ok(v)) = policy().map(|p| GithubAttestation::new(&p)) {
        let _ = v.verify(&[0u8; 32], data);
    }
}

/// Ziel `version_compare`: zwei Texte als Versionen lesen und vergleichen.
pub fn version_compare(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    let (a, b) = text.split_once('|').unwrap_or((&text, ""));
    if let (Ok(a), Ok(b)) = (version::parse(a), version::parse(b)) {
        let _ = version::decide(&a, &b, Some(&b));
    }
}

/// Ziel `redirect_url`: Adresse aus einem `Location`-Kopf lesen.
pub fn redirect_url(data: &[u8]) {
    let _ = fetch::HttpsUrl::parse(&String::from_utf8_lossy(data));
}
