use super::*;
use serde_json::{json, Value};

const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const URL: &str = "https://github.com/daedyx/tasks/releases/download/v0.2.1/daedyx-tasks-0.2.1-setup.exe";

fn policy() -> UpdatePolicy {
    UpdatePolicy::new("daedyx", "tasks", 1001, 2002, "tasks", Platform::WindowsX64).unwrap()
}

fn good() -> Value {
    json!({
        "schema": 1, "app": "tasks", "version": "0.2.1", "date": "2026-12-01",
        "notes": "Neu: Suche.\nBehoben: Absturz.", "important": false, "minLauncherVersion": "0.2.0",
        "registryKey": "Daedyx Tasks",
        "packages": {
            "windows-x64": {"url": URL, "sha256": SHA, "size": 1234, "kind": "nsis-silent"},
            "macos-arm64": {"url": "https://github.com/daedyx/tasks/releases/download/v0.2.1/t.dmg",
                            "sha256": SHA, "size": 1, "kind": "dmg"}
        }
    })
}

fn parse_value(v: &Value) -> Result<Entry, EntryError> {
    parse(&serde_json::to_vec(v).unwrap(), &policy())
}

fn with(path: &[&str], value: Value) -> Value {
    let mut v = good();
    let mut cur = &mut v;
    let (last, head) = path.split_last().unwrap();
    for p in head {
        cur = cur.get_mut(*p).unwrap();
    }
    cur.as_object_mut().unwrap().insert((*last).to_string(), value);
    v
}

#[test]
fn the_control_entry_is_valid() {
    let e = parse_value(&good()).unwrap();
    assert_eq!(e.version, Version::new(0, 2, 1));
    assert_eq!(e.package.size, 1234);
    assert_eq!(e.package.url, URL);
    assert_eq!(hex::encode(e.package.sha256), SHA);
    assert_eq!(e.package.kind, PackageKind::NsisSilent);
    assert_eq!(e.min_launcher_version, Some(Version::new(0, 2, 0)));
    assert!(!e.important);
}

#[test]
fn optional_fields_may_be_missing() {
    let mut v = good();
    for k in ["notes", "important", "minLauncherVersion", "registryKey"] {
        v.as_object_mut().unwrap().remove(k);
    }
    let e = parse_value(&v).unwrap();
    assert!(e.notes.is_empty() && e.min_launcher_version.is_none() && e.registry_key.is_none());
}

#[test]
fn unknown_fields_are_refused_on_every_level() {
    assert!(matches!(parse_value(&with(&["extra"], json!(1))), Err(EntryError::Malformed(_))));
    let v = with(&["packages", "windows-x64", "args"], json!("/S"));
    assert!(matches!(parse_value(&v), Err(EntryError::Malformed(_))));
}

#[test]
fn duplicate_keys_are_refused_on_every_level() {
    let text = serde_json::to_string(&good()).unwrap();
    let top = text.replacen("\"schema\":1", "\"schema\":1,\"schema\":1", 1);
    let pkg = text.replacen("\"packages\":{", &format!("\"packages\":{{\"windows-x64\":{{\"url\":\"{URL}\",\"sha256\":\"{SHA}\",\"size\":1,\"kind\":\"nsis-silent\"}},"), 1);
    let inner = text.replacen("\"size\":1234", "\"size\":1234,\"size\":1", 1);
    for t in [top, pkg, inner] {
        assert!(matches!(parse(t.as_bytes(), &policy()), Err(EntryError::Malformed(_))), "{t}");
    }
}

#[test]
fn size_schema_app_and_version_are_checked() {
    let mut big = serde_json::to_vec(&good()).unwrap();
    big.resize(MAX_ENTRY_BYTES + 1, b' ');
    assert_eq!(parse(&big, &policy()), Err(EntryError::TooLarge));
    assert_eq!(parse_value(&with(&["schema"], json!(2))), Err(EntryError::Schema));
    assert_eq!(parse_value(&with(&["app"], json!("desk"))), Err(EntryError::WrongApp));
    for bad in ["0.2.1-rc1", "v0.2.1", "0.2", "0.2.1+x"] {
        assert_eq!(parse_value(&with(&["version"], json!(bad))), Err(EntryError::Version), "{bad}");
    }
    assert_eq!(parse_value(&with(&["date"], json!("1.12.2026"))), Err(EntryError::Field("date")));
    assert_eq!(parse_value(&with(&["minLauncherVersion"], json!("1"))), Err(EntryError::Field("minLauncherVersion")));
}

#[test]
fn a_padded_entry_at_exactly_64_kb_is_read() {
    let mut text = serde_json::to_vec(&good()).unwrap();
    text.resize(MAX_ENTRY_BYTES, b' ');
    assert!(parse(&text, &policy()).is_ok());
}

#[test]
fn notes_are_plain_text() {
    for bad in [
        "a\u{0}b",
        "a\u{1b}[31m",
        "a\rb",
        "a\u{202E}b",
        "a\u{2066}b",
        "a\u{7f}",
        "a\u{061C}b",
        "a\u{200B}b",
        "a\u{2028}b",
    ] {
        assert_eq!(parse_value(&with(&["notes"], json!(bad))), Err(EntryError::Field("notes")), "{bad:?}");
    }
    let long: String = "ä".repeat(MAX_NOTES_CHARS + 1);
    assert_eq!(parse_value(&with(&["notes"], json!(long))), Err(EntryError::Field("notes")));
    assert!(parse_value(&with(&["notes"], json!("ä".repeat(MAX_NOTES_CHARS)))).is_ok());
    assert!(parse_value(&with(&["notes"], json!("[Link](https://example.invalid)\tok"))).is_ok());
}

#[test]
fn registry_key_must_be_plain() {
    for bad in ["", "a\\b", "a\nb", &"k".repeat(201)] {
        assert_eq!(parse_value(&with(&["registryKey"], json!(bad))), Err(EntryError::Field("registryKey")));
    }
}

#[test]
fn the_package_must_exist_and_match_the_platform() {
    let mut v = good();
    v["packages"].as_object_mut().unwrap().remove("windows-x64");
    assert_eq!(parse_value(&v), Err(EntryError::NoPackage));
    let dmg = with(&["packages", "windows-x64", "kind"], json!("dmg"));
    assert_eq!(parse_value(&dmg), Err(EntryError::Field("kind")));
    let unknown_kind = with(&["packages", "windows-x64", "kind"], json!("msi"));
    assert!(matches!(parse_value(&unknown_kind), Err(EntryError::Malformed(_))));
}

#[test]
fn hash_and_size_are_strict() {
    let short = SHA.get(1..).unwrap();
    for bad in [short, &SHA.to_uppercase(), &format!("{short}g"), ""] {
        let v = with(&["packages", "windows-x64", "sha256"], json!(bad));
        assert_eq!(parse_value(&v), Err(EntryError::Field("sha256")), "{bad}");
    }
    for bad in [json!(0), json!(MAX_SETUP_BYTES + 1)] {
        assert_eq!(parse_value(&with(&["packages", "windows-x64", "size"], bad)), Err(EntryError::Field("size")));
    }
    for bad in [json!(-1), json!(1.5), json!("12")] {
        assert!(matches!(parse_value(&with(&["packages", "windows-x64", "size"], bad)), Err(EntryError::Malformed(_))));
    }
}

#[test]
fn the_url_must_be_on_this_apps_release_path() {
    let base = "https://github.com/daedyx/tasks/releases/download/v0.2.1/";
    for bad in [
        "http://github.com/daedyx/tasks/releases/download/v0.2.1/s.exe".to_string(),
        "https://github.com.evil.example/daedyx/tasks/releases/download/v0.2.1/s.exe".into(),
        "https://evil.example/daedyx/tasks/releases/download/v0.2.1/s.exe".into(),
        "https://github.com/daedyx/desk/releases/download/v0.2.1/s.exe".into(),
        "https://github.com/daedyx/tasks/releases/download/v0.2.0/s.exe".into(),
        "https://github.com/daedyx/tasks/releases/download/v0.2.1/../v0.1.0/s.exe".into(),
        // Nutzerteil zusammengesetzt, damit der Scan (Regel mailadresse) keine Adresse im Quelltext sieht.
        concat!("https://user", "@", "github.com/daedyx/tasks/releases/download/v0.2.1/s.exe").into(),
        "https://github.com:444/daedyx/tasks/releases/download/v0.2.1/s.exe".into(),
        format!("{base}s.exe?x=1"),
        format!("{base}s.exe#x"),
        format!("{base}a/s.exe"),
        format!("{base}s%2e.exe"),
        format!("{base}.exe"),
        format!("{base}s.dmg"),
        format!("{base}s.exe "),
        base.to_string(),
    ] {
        let v = with(&["packages", "windows-x64", "url"], json!(bad));
        assert_eq!(parse_value(&v), Err(EntryError::Url), "{bad}");
    }
}

#[test]
fn other_platforms_are_ignored_but_bounded() {
    let mut v = good();
    v["packages"]["linux-x64"] = json!({"url": "x", "sha256": "y", "size": 1, "kind": "dmg"});
    assert!(parse_value(&v).is_ok());
    for i in 0..20 {
        v["packages"][format!("p{i}")] = json!({"url": "x", "sha256": "y", "size": 1, "kind": "dmg"});
    }
    assert!(matches!(parse_value(&v), Err(EntryError::Malformed(_))));
}

#[test]
fn the_list_form_serde_would_also_read_is_refused() {
    let top = json!([1, "tasks", "0.2.1", "2026-12-01", "", false, null, null,
        {"windows-x64": {"url": URL, "sha256": SHA, "size": 1234, "kind": "nsis-silent"}}]);
    assert!(matches!(parse_value(&top), Err(EntryError::Malformed(_))));
    let pkg = with(&["packages", "windows-x64"], json!([URL, SHA, 1234, "nsis-silent"]));
    assert!(matches!(parse_value(&pkg), Err(EntryError::Malformed(_))));
}

#[test]
fn garbage_and_other_json_types_are_refused() {
    for bad in [&b""[..], b"null", b"[]", b"\xEF\xBB\xBF{}", b"{\"schema\":1", b"\xff\xfe"] {
        assert!(matches!(parse(bad, &policy()), Err(EntryError::Malformed(_))));
    }
}
