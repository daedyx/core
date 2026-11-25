//! Eigenschafts- und Missbrauchstests (SICHERHEIT 2.4) für alle Eingänge aus dem Netz: nie Panik, und was
//! angenommen wird, hält die Regeln ein.
use crate::attest::{GithubAttestation, Provenance};
use crate::fetch::HttpsUrl;
use crate::policy::{Platform, UpdatePolicy};
use crate::version::{self, Decision};
use crate::{entry, state};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

const BUNDLE: &[u8] = include_bytes!("../testdata/uv/uv-manifest.provenance.sigstore.json");
const MANIFEST: &[u8] = include_bytes!("../testdata/uv/dist-manifest.json");

fn tasks() -> UpdatePolicy {
    UpdatePolicy::new("daedyx", "tasks", 1001, 2002, "tasks", Platform::WindowsX64).unwrap()
}

const ENTRY: &str = r#"{"schema":1,"app":"tasks","version":"0.2.1","date":"2026-12-01","notes":"x",
"packages":{"windows-x64":{"url":"https://github.com/daedyx/tasks/releases/download/v0.2.1/s.exe",
"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","size":5,"kind":"nsis-silent"}}}"#;

proptest! {
    #[test]
    fn entry_never_panics_on_any_bytes(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = entry::parse(&data, &tasks());
    }

    #[test]
    fn an_accepted_entry_always_keeps_the_rules(pos in 0usize..ENTRY.len(), byte in any::<u8>()) {
        let mut data = ENTRY.as_bytes().to_vec();
        data[pos] = byte;
        if let Ok(e) = entry::parse(&data, &tasks()) {
            prop_assert!(e.package.url.starts_with("https://github.com/daedyx/tasks/releases/download/v"));
            prop_assert!(e.package.size > 0 && e.package.size <= entry::MAX_SETUP_BYTES);
            prop_assert!(e.version.pre.is_empty() && e.version.build.is_empty());
        }
    }

    #[test]
    fn urls_never_panic_and_hosts_are_plain(text in ".{0,300}") {
        if let Ok(u) = HttpsUrl::parse(&text) {
            prop_assert!(u.as_str().starts_with("https://"));
            prop_assert!(u.host().bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'));
        }
    }

    #[test]
    fn versions_never_panic_and_decisions_are_consistent(a in "[0-9.a-z+-]{0,20}", b in 0u64..5, c in 0u64..5, d in 0u64..5) {
        let _ = version::parse(&a);
        let latest = version::Version::new(0, b, 0);
        let installed = version::Version::new(0, c, 0);
        let seen = version::Version::new(0, d, 0);
        if version::decide(&latest, &installed, Some(&seen)) == Decision::Newer {
            prop_assert!(latest > installed && latest >= seen);
        }
    }

    #[test]
    fn state_times_never_overflow(last in any::<i64>(), now in any::<i64>()) {
        let mut s = state::UpdateState::default();
        s.last_check = Some(last);
        let _ = s.due_for_auto_check(now);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn a_bundle_with_any_flipped_byte_never_panics(
        pos in 0usize..BUNDLE.len(),
        mask in prop::sample::select(vec![1u8, 2, 4, 8, 16, 32, 64, 128]),
    ) {
        let v = GithubAttestation::new(
            &UpdatePolicy::new("astral-sh", "uv", 115_962_839, 699_532_645, "uv", Platform::WindowsX64).unwrap(),
        ).unwrap();
        let mut data = BUNDLE.to_vec();
        data[pos] ^= mask;
        let _ = v.verify(&Sha256::digest(MANIFEST).into(), &data);
    }

    #[test]
    fn random_bundles_are_never_accepted(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let v = GithubAttestation::new(&tasks()).unwrap();
        prop_assert!(v.verify(&[0u8; 32], &data).is_err());
    }
}
