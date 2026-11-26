use super::*;

#[test]
fn a_valid_policy_builds_fixed_addresses() {
    let p = UpdatePolicy::new("daedyx", "tasks", 1001, 2002, "tasks", Platform::WindowsX64).unwrap();
    assert_eq!(p.latest_asset_url("entry.json"), "https://github.com/daedyx/tasks/releases/latest/download/entry.json");
    assert_eq!(p.release_path_prefix(&semver::Version::new(0, 2, 0)), "/daedyx/tasks/releases/download/v0.2.0/");
    // Kein Link nach Namen für „von Hand laden“ (Review M-U1-1): nur die erwartete Quelle als Text.
    assert_eq!(p.expected_source(), "github.com/daedyx/tasks");
    assert_eq!(p.to_string(), "daedyx/tasks (1001/2002) .github/workflows/release.yml@refs/heads/main");
}

#[test]
fn bad_names_ids_and_app_ids_are_refused() {
    let w = Platform::WindowsX64;
    for (owner, repo) in [("", "x"), ("a/b", "x"), ("x", ".."), ("x", "a b"), ("x", "a?b"), ("ü", "x")] {
        assert!(UpdatePolicy::new(owner, repo, 1, 1, "x", w).is_err(), "{owner}/{repo}");
    }
    assert!(UpdatePolicy::new("x", "y", 0, 1, "x", w).is_err());
    assert!(UpdatePolicy::new("x", "y", 1, 0, "x", w).is_err());
    for app in ["", "-x", "Tasks", "a.b", "a/b", "..", &"a".repeat(41)] {
        assert!(UpdatePolicy::new("x", "y", 1, 1, app, w).is_err(), "{app}");
    }
}

#[test]
fn app_ids_are_lowercase_digits_and_dashes() {
    for ok in ["tasks", "desk", "a-1", "0x"] {
        assert!(valid_app_id(ok), "{ok}");
    }
}

#[test]
fn platform_keys_are_stable() {
    assert_eq!(Platform::WindowsX64.key(), "windows-x64");
    assert_eq!(Platform::MacosArm64.key(), "macos-arm64");
    assert_eq!(Platform::MacosX64.key(), "macos-x64");
    if cfg!(all(windows, target_arch = "x86_64")) {
        assert_eq!(Platform::current(), Some(Platform::WindowsX64));
    }
}
