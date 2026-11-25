//! Sperrdatei je App (aus `launcher-core::lock` umgezogen): verhindert, dass App und Launcher (oder zwei
//! Instanzen) gleichzeitig dieselbe App aktualisieren. Gemeinsamer Ort für alle: `<lokale Daten>\daedyx\locks`.
//! Eine Sperre gilt nach einer Stunde ohne Lebenszeichen als übrig geblieben; wer lange lädt, ruft `touch`.
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::policy::valid_app_id;

const STALE_AFTER: Duration = Duration::from_secs(60 * 60);

/// Warum die Sperre nicht geholt wurde.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    #[error("an update for this app is already running")]
    Busy,
    #[error("invalid app id")]
    BadAppId,
    #[error("lock file error: {0:?}")]
    Io(ErrorKind),
}

/// Gemeinsamer Sperrordner unter dem lokalen Datenordner des Benutzers (Windows: `%LOCALAPPDATA%`).
pub fn shared_lock_dir(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join("daedyx").join("locks")
}

/// Gehaltene Sperre; wird beim Fallenlassen freigegeben.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

fn is_stale(path: &Path, now: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| now.duration_since(t).ok())
        .is_some_and(|age| age > STALE_AFTER)
}

impl Lock {
    pub fn acquire(dir: &Path, app_id: &str) -> Result<Lock, LockError> {
        if !valid_app_id(app_id) {
            return Err(LockError::BadAppId);
        }
        std::fs::create_dir_all(dir).map_err(|e| LockError::Io(e.kind()))?;
        let path = dir.join(format!("{app_id}.lock"));
        for _ in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Lock { path }),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    if !is_stale(&path, SystemTime::now()) {
                        return Err(LockError::Busy);
                    }
                    let _ = std::fs::remove_file(&path);
                }
                Err(e) => return Err(LockError::Io(e.kind())),
            }
        }
        Err(LockError::Busy)
    }

    /// Lebenszeichen während eines langen Downloads, damit niemand die Sperre für übrig geblieben hält.
    pub fn touch(&self) {
        if let Ok(f) = OpenOptions::new().write(true).open(&self.path) {
            let _ = f.set_modified(SystemTime::now());
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
