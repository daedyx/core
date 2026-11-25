//! Ablauf mit gefälschtem Server und Prüf-Attrappe (nur hier): Die Attrappe bestätigt genau die Prüfsummen, die
//! ihr Test nennt, und nur für das Bündel `BUNDLE`. Die echte Kryptoprüfung steckt in `attest_tests`.
use super::*;
use crate::attest::{AttestError, AttestKind};
use crate::fetch::TransportError;
use crate::policy::Platform;
use crate::test_support::{Fake, FakeServer};
use serde_json::json;

const BUNDLE: &[u8] = b"erfundenes-buendel";
const ENTRY_URL: &str = "https://github.com/daedyx/tasks/releases/latest/download/entry.json";
const BUNDLE_URL: &str = "https://github.com/daedyx/tasks/releases/latest/download/provenance.sigstore.json";
const SETUP: &[u8] = b"erfundenes Setup 0.2.1";

struct FakeProof {
    known: Vec<[u8; 32]>,
    fail_as: Option<AttestKind>,
}

impl Provenance for FakeProof {
    fn verify(&self, sha256: &[u8; 32], bundle: &[u8]) -> Result<(), AttestError> {
        if let Some(kind) = self.fail_as {
            return Err(AttestError { kind, detail: "attrappe".into() });
        }
        if bundle == BUNDLE && self.known.contains(sha256) {
            Ok(())
        } else {
            Err(AttestError { kind: AttestKind::NotInBundle, detail: "attrappe: nicht im buendel".into() })
        }
    }
}

fn policy() -> UpdatePolicy {
    UpdatePolicy::new("daedyx", "tasks", 1001, 2002, "tasks", Platform::WindowsX64).unwrap()
}

fn setup_url(v: &str) -> String {
    format!("https://github.com/daedyx/tasks/releases/download/v{v}/daedyx-tasks-{v}-setup.exe")
}

fn entry_json(app: &str, v: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema": 1, "app": app, "version": v, "date": "2026-12-01", "notes": "Erfundene Notiz",
        "packages": {"windows-x64": {"url": setup_url(v), "sha256": hex::encode(sha(SETUP)),
                                     "size": SETUP.len(), "kind": "nsis-silent"}}
    }))
    .unwrap()
}

/// Server und Prüfer für eine echte Veröffentlichung von `v`.
fn world(v: &str) -> (FakeServer, FakeProof) {
    let entry = entry_json("tasks", v);
    let server = FakeServer::default()
        .with(ENTRY_URL, Fake::ok(&entry))
        .with(BUNDLE_URL, Fake::ok(BUNDLE))
        .with(&setup_url(v), Fake::ok(SETUP));
    (server, FakeProof { known: vec![sha(&entry), sha(SETUP)], fail_as: None })
}

fn run(server: &mut FakeServer, proof: &FakeProof, installed: &str, seen: Option<&str>) -> Outcome {
    let seen = seen.map(|s| Version::parse(s).unwrap());
    check(server, proof, &policy(), &Version::parse(installed).unwrap(), seen.as_ref())
}

fn blocked(o: Outcome) -> Blocked {
    match o {
        Outcome::Blocked(b) => b,
        other => panic!("erwartet Sperre, bekam {other:?}"),
    }
}

#[test]
fn a_newer_genuine_release_is_offered_without_downloading_the_setup() {
    let (mut s, p) = world("0.2.1");
    let Outcome::Available(offer) = run(&mut s, &p, "0.2.0", None) else { panic!("kein Angebot") };
    assert_eq!(offer.entry().version, Version::new(0, 2, 1));
    assert_eq!(s.requests, [ENTRY_URL, BUNDLE_URL]);
}

#[test]
fn the_same_or_an_older_version_is_up_to_date() {
    let (mut s, p) = world("0.2.1");
    assert_eq!(run(&mut s, &p, "0.2.1", None), Outcome::UpToDate { latest: Version::new(0, 2, 1) });
    assert_eq!(run(&mut s, &p, "0.3.0", None).verified_latest(), Some(&Version::new(0, 2, 1)));
}

#[test]
fn a_release_below_the_highest_seen_is_reported_not_offered() {
    let (mut s, p) = world("0.2.1");
    let b = blocked(run(&mut s, &p, "0.2.0", Some("0.2.2")));
    assert_eq!(b.kind, BlockKind::SourceWentBack);
}

#[test]
fn an_unattested_entry_is_never_read() {
    let (mut s, mut p) = world("0.2.1");
    p.known.retain(|h| *h == sha(SETUP));
    let b = blocked(run(&mut s, &p, "0.2.0", None));
    assert_eq!(b.kind, BlockKind::NotTrusted);
    assert!(b.detail.contains("attrappe"), "Grund kommt vom Nachweis, nicht vom Lesen: {b}");
}

#[test]
fn the_setup_subject_must_be_in_the_same_bundle() {
    let (mut s, mut p) = world("0.2.1");
    p.known.retain(|h| *h != sha(SETUP));
    assert_eq!(blocked(run(&mut s, &p, "0.2.0", None)).kind, BlockKind::NotTrusted);
}

#[test]
fn an_attested_entry_of_another_app_is_refused() {
    let entry = entry_json("desk", "0.9.0");
    let mut s = FakeServer::default().with(ENTRY_URL, Fake::ok(&entry)).with(BUNDLE_URL, Fake::ok(BUNDLE));
    let p = FakeProof { known: vec![sha(&entry), sha(SETUP)], fail_as: None };
    let b = blocked(run(&mut s, &p, "0.2.0", None));
    assert_eq!((b.kind, b.detail.as_str()), (BlockKind::NotTrusted, "entry belongs to another app"));
}

#[test]
fn network_and_source_problems_map_to_quiet_reasons() {
    let cases = [
        (Fake::Fail(TransportError::Offline), BlockKind::Offline),
        (Fake::Fail(TransportError::Timeout), BlockKind::Offline),
        (Fake::Status(404), BlockKind::SourceUnavailable),
        (Fake::Status(503), BlockKind::SourceUnavailable),
        (Fake::Body(vec![b' '; MAX_ENTRY_BYTES + 1], None), BlockKind::NotTrusted),
        (Fake::Redirect("https://evil.example/entry.json".into()), BlockKind::NotTrusted),
    ];
    for (reply, want) in cases {
        let (s, p) = world("0.2.1");
        let mut s = s.with(ENTRY_URL, reply);
        assert_eq!(blocked(run(&mut s, &p, "0.2.0", None)).kind, want);
    }
    let (s, p) = world("0.2.1");
    let mut s = s.with(BUNDLE_URL, Fake::Body(vec![b' '; MAX_BUNDLE_BYTES + 1], None));
    assert_eq!(blocked(run(&mut s, &p, "0.2.0", None)).kind, BlockKind::NotTrusted);
}

#[test]
fn a_proof_this_version_cannot_check_asks_for_a_manual_update() {
    let (mut s, mut p) = world("0.2.1");
    p.fail_as = Some(AttestKind::Unsupported);
    assert_eq!(blocked(run(&mut s, &p, "0.2.0", None)).kind, BlockKind::NeedsManualUpdate);
}

fn offer() -> (FakeServer, FakeProof, Offer) {
    let (mut s, p) = world("0.2.1");
    let Outcome::Available(o) = run(&mut s, &p, "0.2.0", None) else { panic!("kein Angebot") };
    (s, p, *o)
}

fn dir_is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir).map(|mut d| d.next().is_none()).unwrap_or(true)
}

#[test]
fn the_setup_is_loaded_only_on_request_and_checked() {
    let (mut s, p, o) = offer();
    let dir = tempfile::tempdir().unwrap();
    let path = fetch_setup(&mut s, &p, &policy(), &o, dir.path(), None, &mut |_, _| {}).unwrap();
    assert_eq!(path.file_name().unwrap(), "tasks-0.2.1-setup.exe");
    assert_eq!(std::fs::read(path).unwrap(), SETUP);
}

#[test]
fn a_bad_setup_or_a_failing_second_proof_leaves_nothing() {
    let cases = [
        (Fake::ok(b"erfundenes Setup 0.2.X"), None, BlockKind::NotTrusted),
        (Fake::BodyThenError(SETUP[..4].to_vec(), std::io::ErrorKind::ConnectionReset), None, BlockKind::Interrupted),
        (Fake::ok(SETUP), Some(AttestKind::Invalid), BlockKind::NotTrusted),
    ];
    for (reply, proof_fails, want) in cases {
        let (s, mut p, o) = offer();
        let mut s = s.with(&setup_url("0.2.1"), reply);
        p.fail_as = proof_fails;
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_setup(&mut s, &p, &policy(), &o, dir.path(), None, &mut |_, _| {}).unwrap_err();
        assert_eq!(err.kind, want, "{err}");
        assert!(dir_is_empty(dir.path()), "{err}: nichts bleibt liegen");
    }
}

#[test]
fn too_little_space_is_reported_before_loading() {
    let (mut s, p, o) = offer();
    let dir = tempfile::tempdir().unwrap();
    let before = s.requests.len();
    let err = fetch_setup(&mut s, &p, &policy(), &o, dir.path(), Some(1), &mut |_, _| {}).unwrap_err();
    assert_eq!(err.kind, BlockKind::NoSpace);
    assert_eq!(s.requests.len(), before);
}
