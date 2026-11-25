use super::*;

fn age(path: &Path, secs: u64) {
    let old = SystemTime::now().checked_sub(Duration::from_secs(secs)).unwrap();
    OpenOptions::new().write(true).open(path).unwrap().set_modified(old).unwrap();
}

#[test]
fn a_second_lock_for_the_same_app_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let _first = Lock::acquire(dir.path(), "desk").unwrap();
    assert_eq!(Lock::acquire(dir.path(), "desk").unwrap_err(), LockError::Busy);
}

#[test]
fn another_app_is_not_locked() {
    let dir = tempfile::tempdir().unwrap();
    let _first = Lock::acquire(dir.path(), "desk").unwrap();
    assert!(Lock::acquire(dir.path(), "tasks").is_ok());
}

#[test]
fn dropping_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    drop(Lock::acquire(dir.path(), "desk").unwrap());
    assert!(Lock::acquire(dir.path(), "desk").is_ok());
}

#[test]
fn a_left_over_lock_is_taken_over_a_fresh_one_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desk.lock");
    std::fs::write(&path, b"").unwrap();
    age(&path, 30 * 60);
    assert_eq!(Lock::acquire(dir.path(), "desk").unwrap_err(), LockError::Busy);
    age(&path, 2 * 60 * 60);
    assert!(Lock::acquire(dir.path(), "desk").is_ok());
}

#[test]
fn touching_keeps_a_long_download_locked() {
    let dir = tempfile::tempdir().unwrap();
    let held = Lock::acquire(dir.path(), "desk").unwrap();
    age(&dir.path().join("desk.lock"), 2 * 60 * 60);
    held.touch();
    assert_eq!(Lock::acquire(dir.path(), "desk").unwrap_err(), LockError::Busy);
}

#[test]
fn app_ids_cannot_leave_the_lock_folder() {
    let dir = tempfile::tempdir().unwrap();
    for bad in ["../desk", "a/b", "", "Desk", "a\\b"] {
        assert_eq!(Lock::acquire(dir.path(), bad).unwrap_err(), LockError::BadAppId, "{bad}");
    }
}

#[test]
fn the_shared_folder_is_the_same_for_app_and_launcher() {
    let base = Path::new("lokal");
    assert_eq!(shared_lock_dir(base), base.join("daedyx").join("locks"));
}
