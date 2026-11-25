use super::*;
use crate::test_support::{Fake, FakeServer};
use sha2::{Digest, Sha256};

const URL: &str = "https://github.com/daedyx/tasks/releases/download/v0.2.1/daedyx-tasks-0.2.1-setup.exe";
const NAME: &str = "tasks-0.2.1-setup.exe";

fn expected_for(data: &[u8]) -> Expected {
    Expected { size: data.len() as u64, sha256: Sha256::digest(data).into() }
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn run(server: &mut FakeServer, dir: &Path, expected: &Expected) -> Result<PathBuf, DownloadError> {
    download_verified(server, URL, dir, NAME, expected, None, &mut |_, _| {})
}

#[test]
fn a_correct_file_is_kept_under_its_final_name() {
    let dir = tempfile::tempdir().unwrap();
    let data = b"erfundenes Setup".to_vec();
    let mut s = FakeServer::default().with(URL, Fake::ok(&data));
    let path = run(&mut s, dir.path(), &expected_for(&data)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), data);
    assert_eq!(files(dir.path()), [NAME]);
}

#[test]
fn every_kind_of_tampering_leaves_no_file() {
    let data = b"erfundenes Setup".to_vec();
    let want = expected_for(&data);
    let mut flipped = data.clone();
    flipped[0] ^= 1;
    let cases: Vec<(Fake, DownloadError)> = vec![
        (Fake::ok(&flipped), DownloadError::Hash),
        (Fake::ok(&data[..data.len().saturating_sub(1)]), DownloadError::Size),
        (Fake::ok(&[data.clone(), b"!".to_vec()].concat()), DownloadError::Fetch(FetchError::TooLarge)),
        (Fake::Body([data.clone(), b"!".to_vec()].concat(), None), DownloadError::Fetch(FetchError::TooLarge)),
        (Fake::ok(b""), DownloadError::Size),
        (
            Fake::BodyThenError(data[..5].to_vec(), io::ErrorKind::ConnectionReset),
            DownloadError::Fetch(FetchError::Interrupted),
        ),
        (Fake::Status(404), DownloadError::Fetch(FetchError::Status(404))),
        (Fake::Redirect("https://evil.example/s.exe".into()), DownloadError::Fetch(FetchError::ForeignHost)),
    ];
    for (reply, err) in cases {
        let dir = tempfile::tempdir().unwrap();
        let mut s = FakeServer::default().with(URL, reply);
        assert_eq!(run(&mut s, dir.path(), &want), Err(err.clone()));
        assert!(files(dir.path()).is_empty(), "{err:?}: nichts bleibt liegen");
    }
}

#[test]
fn a_wrong_size_in_the_entry_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let data = b"abcdef".to_vec();
    let mut s = FakeServer::default().with(URL, Fake::ok(&data));
    let mut want = expected_for(&data);
    want.size = 7;
    assert_eq!(run(&mut s, dir.path(), &want), Err(DownloadError::Size));
    want.size = 5;
    assert_eq!(run(&mut s, dir.path(), &want), Err(DownloadError::Fetch(FetchError::TooLarge)));
    assert!(files(dir.path()).is_empty());
}

#[test]
fn old_leftovers_are_never_reused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(NAME), b"alt").unwrap();
    fs::write(dir.path().join(format!("{NAME}.part")), b"halb").unwrap();
    let mut s = FakeServer::default().with(URL, Fake::Status(500));
    assert!(run(&mut s, dir.path(), &expected_for(b"neu")).is_err());
    assert!(files(dir.path()).is_empty());
}

#[test]
fn too_little_space_stops_before_loading() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = FakeServer::default().with(URL, Fake::ok(b"1234"));
    let want = expected_for(b"1234");
    let r = download_verified(&mut s, URL, dir.path(), NAME, &want, Some(7), &mut |_, _| {});
    assert_eq!(r, Err(DownloadError::NoSpace));
    assert!(s.requests.is_empty());
    assert!(download_verified(&mut s, URL, dir.path(), NAME, &want, Some(8), &mut |_, _| {}).is_ok());
    assert!(enough_space(u64::MAX, u64::MAX / 2) && !enough_space(u64::MAX, u64::MAX));
}

#[test]
fn file_names_come_from_checked_parts_only() {
    let v = Version::new(0, 2, 1);
    assert_eq!(setup_file_name("tasks", &v, PackageKind::NsisSilent).as_deref(), Some("tasks-0.2.1-setup.exe"));
    assert_eq!(setup_file_name("desk", &v, PackageKind::Dmg).as_deref(), Some("desk-0.2.1-setup.dmg"));
    assert_eq!(setup_file_name("../x", &v, PackageKind::Dmg), None);
}

#[test]
fn leftovers_are_cleaned_but_foreign_files_stay() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["tasks-0.2.0-setup.exe", "x.part", "desk-0.1.0-setup.dmg", "notiz.txt"] {
        fs::write(dir.path().join(f), b"x").unwrap();
    }
    fs::create_dir(dir.path().join("sub.part")).unwrap();
    assert_eq!(clean_leftovers(dir.path()), 3);
    assert_eq!(files(dir.path()), ["notiz.txt", "sub.part"]);
    assert_eq!(clean_leftovers(&dir.path().join("fehlt")), 0);
}
