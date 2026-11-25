use super::*;

fn v(s: &str) -> Version {
    parse(s).unwrap()
}

#[test]
fn plain_semver_is_accepted() {
    assert_eq!(v("0.2.0"), Version::new(0, 2, 0));
    assert_eq!(v("10.20.30"), Version::new(10, 20, 30));
}

#[test]
fn pre_release_build_and_sloppy_forms_are_refused() {
    for bad in ["0.3.0-rc1", "0.3.0+build", "v0.3.0", " 0.3.0", "0.3", "0.3.0.1", "", "01.2.3", "0.3.0\n", "x"] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn newer_compares_numerically() {
    assert!(is_newer(&v("0.10.0"), &v("0.9.9")));
    assert!(!is_newer(&v("0.2.0"), &v("0.2.0")));
    assert!(!is_newer(&v("0.1.9"), &v("0.2.0")));
}

#[test]
fn decision_follows_installed_and_highest_seen() {
    let installed = v("0.2.0");
    assert_eq!(decide(&v("0.2.1"), &installed, None), Decision::Newer);
    assert_eq!(decide(&v("0.2.0"), &installed, None), Decision::UpToDate);
    // Älter als installiert, aber nie Höheres gesehen: nichts anbieten (z. B. Entwicklerstand).
    assert_eq!(decide(&v("0.1.0"), &installed, None), Decision::UpToDate);
    // Die Quelle liefert weniger als schon einmal geprüft gesehen: Warnung, nie installieren.
    assert_eq!(decide(&v("0.2.1"), &installed, Some(&v("0.2.2"))), Decision::WentBack);
    assert_eq!(decide(&v("0.2.2"), &installed, Some(&v("0.2.2"))), Decision::Newer);
    // Nach Neuinstallation einer älteren Fassung bleibt die höchste gesehene Version der Maßstab.
    assert_eq!(decide(&v("0.2.0"), &v("0.1.0"), Some(&v("0.2.1"))), Decision::WentBack);
}

#[test]
fn highest_keeps_the_maximum() {
    assert_eq!(highest(None, &v("0.2.0")), v("0.2.0"));
    assert_eq!(highest(Some(&v("0.3.0")), &v("0.2.0")), v("0.3.0"));
    assert_eq!(highest(Some(&v("0.1.0")), &v("0.2.0")), v("0.2.0"));
}
