//! Sperrdatei je App (aus `launcher-core::lock` umgezogen): verhindert, dass App und Launcher (oder zwei
//! Instanzen) gleichzeitig dieselbe App aktualisieren. Gemeinsamer Ort für alle: `<lokale Daten>\daedyx\locks`.
//! Eine Sperre gilt nach einer Stunde ohne Lebenszeichen als übrig geblieben; wer lange lädt, ruft `touch`.
use std::collections::hash_map::RandomState;
use std::fs::OpenOptions;
use std::hash::{BuildHasher, Hasher};
use std::io::ErrorKind;
use std::io::Write;
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

/// Gehaltene Sperre; wird beim Fallenlassen freigegeben, aber nur, solange die Datei noch die eigene Kennung trägt
/// (Review N-U1-2: nach einer Übernahme gehört sie dem Nachfolger).
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
    token: String,
}

fn is_stale(path: &Path, now: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| now.duration_since(t).ok())
        .is_some_and(|age| age > STALE_AFTER)
}

/// Kennung aus Prozessnummer und Zufall (`RandomState` ist je Aufruf neu gesät), ohne weitere Abhängigkeit.
fn new_token() -> String {
    let random = RandomState::new().build_hasher().finish();
    format!("{}-{random:016x}", std::process::id())
}

/// Übernimmt eine übrig gebliebene Sperre, deren Inhalt `seen` war: erst auf einen eigenen Namen umbenennen (das
/// gelingt nur einem), dann prüfen, ob es wirklich noch die alte war. War sie inzwischen ersetzt, wird sie ohne
/// Überschreiben zurückgelegt. `true`, wenn die alte Sperre weg ist.
pub(crate) fn take_over(path: &Path, seen: &[u8], token: &str) -> std::io::Result<bool> {
    let aside = path.with_extension(format!("stale-{token}"));
    match std::fs::rename(path, &aside) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    }
    if std::fs::read(&aside)? == seen {
        std::fs::remove_file(&aside)?;
        return Ok(true);
    }
    // Frische Sperre eines anderen erwischt: zurücklegen. Hat inzwischen ein Dritter eine angelegt, bleibt dessen.
    let _ = std::fs::hard_link(&aside, path);
    std::fs::remove_file(&aside)?;
    Ok(false)
}

impl Lock {
    pub fn acquire(dir: &Path, app_id: &str) -> Result<Lock, LockError> {
        if !valid_app_id(app_id) {
            return Err(LockError::BadAppId);
        }
        let io = |e: std::io::Error| LockError::Io(e.kind());
        std::fs::create_dir_all(dir).map_err(io)?;
        let path = dir.join(format!("{app_id}.lock"));
        let token = new_token();
        for _ in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    f.write_all(token.as_bytes()).and_then(|()| f.sync_all()).map_err(io)?;
                    return Ok(Lock { path, token });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    let seen = std::fs::read(&path).unwrap_or_default();
                    if !is_stale(&path, SystemTime::now()) || !take_over(&path, &seen, &token).map_err(io)? {
                        return Err(LockError::Busy);
                    }
                }
                Err(e) => return Err(io(e)),
            }
        }
        Err(LockError::Busy)
    }

    fn still_mine(&self) -> bool {
        std::fs::read(&self.path).is_ok_and(|c| c == self.token.as_bytes())
    }

    /// Lebenszeichen während eines langen Downloads, damit niemand die Sperre für übrig geblieben hält.
    pub fn touch(&self) {
        if self.still_mine()
            && let Ok(f) = OpenOptions::new().write(true).open(&self.path)
        {
            let _ = f.set_modified(SystemTime::now());
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if self.still_mine() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
