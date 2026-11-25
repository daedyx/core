//! Fälschungsmatrix aus dem Spike S-ATT als Dauertest. Prüfdaten: öffentliches Bündel von astral-sh/uv 0.13.9
//! (`release.yml@refs/heads/main`, genau unsere Form) mit drei Dateien. Regeln: jede Gruppe hat einen Kontrollfall
//! „unverändert = gültig“, jede Erwartung nennt den Grund, Bündel werden nur als Bytes gelesen.
use super::*;
use crate::policy::{Platform, UpdatePolicy};
use sha2::{Digest, Sha256};

const BUNDLE: &[u8] = include_bytes!("../testdata/uv/uv-manifest.provenance.sigstore.json");
const MANIFEST: &[u8] = include_bytes!("../testdata/uv/dist-manifest.json");
const INSTALLER: &[u8] = include_bytes!("../testdata/uv/uv-installer-ps1.txt");
const OWNER_ID: u64 = 115_962_839;
const REPO_ID: u64 = 699_532_645;

fn policy(owner: &str, repo: &str, owner_id: u64, repo_id: u64) -> UpdatePolicy {
    UpdatePolicy::new(owner, repo, owner_id, repo_id, "uv", Platform::WindowsX64).unwrap()
}

fn uv() -> GithubAttestation {
    GithubAttestation::new(&policy("astral-sh", "uv", OWNER_ID, REPO_ID)).unwrap()
}

fn sha(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn kind(r: Result<(), AttestError>) -> AttestKind {
    r.unwrap_err().kind
}

#[test]
fn genuine_files_from_one_bundle_are_accepted() {
    let v = uv();
    v.verify(&sha(MANIFEST), BUNDLE).unwrap();
    v.verify(&sha(INSTALLER), BUNDLE).unwrap();
}

#[test]
fn changed_missing_or_empty_files_are_not_in_the_bundle() {
    let v = uv();
    let mut flipped = MANIFEST.to_vec();
    flipped[100] ^= 1;
    for data in [&flipped[..], b"", b"erfundene Datei"] {
        assert_eq!(kind(v.verify(&sha(data), BUNDLE)), AttestKind::NotInBundle);
    }
}

#[test]
fn another_origin_is_refused_with_the_policy_reason() {
    let cases = [
        policy("astral-sh", "uvx", OWNER_ID, REPO_ID),
        policy("daedyx", "uv", OWNER_ID, REPO_ID),
        policy("astral-sh", "uv", OWNER_ID, 1),
        policy("astral-sh", "uv", 1, REPO_ID),
    ];
    for p in cases {
        let v = GithubAttestation::new(&p).unwrap();
        let e = v.verify(&sha(MANIFEST), BUNDLE).unwrap_err();
        assert_eq!(e.kind, AttestKind::WrongOrigin, "{p}: {e}");
        assert!(e.detail.contains("mismatch"), "{e}");
    }
}

#[test]
fn a_foreign_bundle_or_githubs_release_attestation_is_refused() {
    let cli = include_bytes!("../testdata/foreign/cli-provenance.sigstore.json");
    assert_eq!(kind(uv().verify(&sha(MANIFEST), cli)), AttestKind::NotInBundle);
    let tsa = include_bytes!("../testdata/foreign/uv-release-att.sigstore.json");
    assert_eq!(kind(uv().verify(&sha(MANIFEST), tsa)), AttestKind::Unsupported);
}

macro_rules! tampered {
    ($name:literal) => {
        include_bytes!(concat!("../testdata/tampered/", $name, ".json")).as_slice()
    };
}

#[test]
fn control_cases_rewritten_but_unchanged_stay_valid() {
    let v = uv();
    v.verify(&sha(MANIFEST), tampered!("B0-reserialized")).unwrap();
    v.verify(&sha(MANIFEST), tampered!("B0b-ascii-escaped")).unwrap();
}

#[test]
fn every_tampered_bundle_is_refused_for_the_right_reason() {
    let v = uv();
    let mut flipped = MANIFEST.to_vec();
    flipped[100] ^= 1;
    assert_eq!(kind(v.verify(&sha(&flipped), tampered!("B1-payload-rehash"))), AttestKind::Invalid);
    let cases = [
        (tampered!("B2-sig-flip"), AttestKind::Invalid, "signature"),
        (tampered!("B3-cert-swap"), AttestKind::Invalid, "certificate"),
        (tampered!("B4-time-plus-1d"), AttestKind::Invalid, "(SET)"),
        (tampered!("B4b-time-minus-1d"), AttestKind::Invalid, "(SET)"),
        (tampered!("B5-proof-flip"), AttestKind::Invalid, "inclusion proof"),
        (tampered!("B6-checkpoint"), AttestKind::Invalid, "checkpoint"),
        (tampered!("B7-set-flip"), AttestKind::Invalid, "(SET)"),
        (tampered!("B8-no-tlog"), AttestKind::Invalid, "no transparency-log"),
        (tampered!("B9-body-flip"), AttestKind::Malformed, "parse"),
        (tampered!("B11-truncated"), AttestKind::Malformed, "EOF"),
        (tampered!("B16-dup-key"), AttestKind::Malformed, "duplicate"),
        (tampered!("B17-logindex"), AttestKind::Invalid, "(SET)"),
        (tampered!("B18-two-sigs"), AttestKind::Malformed, "2 signatures"),
    ];
    for (bundle, want, why) in cases {
        let e = v.verify(&sha(MANIFEST), bundle).unwrap_err();
        assert_eq!(e.kind, want, "{e}");
        assert!(e.detail.contains(why), "{e} sollte „{why}“ nennen");
    }
}

#[test]
fn an_old_media_type_is_unsupported() {
    let text = String::from_utf8(BUNDLE.to_vec()).unwrap();
    let old = text.replacen(
        "\"mediaType\":\"application/vnd.dev.sigstore.bundle.v0.3+json\"",
        "\"mediaType\":\"application/vnd.dev.sigstore.bundle+json;version=0.1\"",
        1,
    );
    assert_ne!(old, text, "Ersetzung muss greifen");
    assert_eq!(kind(uv().verify(&sha(MANIFEST), old.as_bytes())), AttestKind::Unsupported);
}

#[test]
fn garbage_deep_nesting_and_oversize_are_malformed() {
    let v = uv();
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    let mut huge = BUNDLE.to_vec();
    huge.resize(MAX_BUNDLE_BYTES + 1, b' ');
    for bad in [&b"\xff not json"[..], b"", b"{}", b"[]", deep.as_bytes(), &huge] {
        assert_eq!(kind(v.verify(&sha(MANIFEST), bad)), AttestKind::Malformed);
    }
    // Genau an der Grenze wird noch gelesen (Leerzeichen hinter dem JSON sind erlaubt).
    let mut edge = BUNDLE.to_vec();
    edge.resize(MAX_BUNDLE_BYTES, b' ');
    v.verify(&sha(MANIFEST), &edge).unwrap();
}

#[test]
fn a_root_this_version_cannot_use_asks_for_a_manual_update() {
    let p = policy("astral-sh", "uv", OWNER_ID, REPO_ID);
    let roots: [&[u8]; 5] = [
        include_bytes!("../testdata/roots/T3-ca-expired.json"),
        include_bytes!("../testdata/roots/T4-rekor-key-expired.json"),
        include_bytes!("../testdata/roots/T5-only-rekor-v2.json"),
        include_bytes!("../testdata/roots/T6-no-ctlog.json"),
        include_bytes!("../testdata/roots/staging.json"),
    ];
    for root in roots {
        let e = GithubAttestation::with_root(&p, root).and_then(|v| v.verify(&sha(MANIFEST), BUNDLE)).unwrap_err();
        assert_eq!(e.kind, AttestKind::Unsupported, "{e}");
    }
    let e = GithubAttestation::with_root(&p, b"{}").err().unwrap();
    assert_eq!(e.kind, AttestKind::Unsupported);
}

#[test]
fn the_embedded_root_is_the_upstream_snapshot() {
    // Wechsel der Wurzel nur bewusst (neuer Kern-Tag): Prüfsumme der Datei aus sigstore/root-signing.
    assert_eq!(hex::encode(sha(TRUSTED_ROOT)), "6494e21ea73fa7ee769f85f57d5a3e6a08725eae1e38c755fc3517c9e6bc0b66");
}
