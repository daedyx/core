//! Geprüfter Download des Setups: Laden in `<name>.part` mit der Größe aus `entry.json` als harter Grenze
//! (Abbruch beim ersten Byte zu viel), SHA-256 während des Ladens, erst nach bestandener Prüfung umbenennen.
//! Bei **jedem** Fehler wird `.part` gelöscht; es bleibt keine Datei liegen (SICHERHEIT 8.1 Nr. 4).
use crate::entry::PackageKind;
use crate::fetch::{self, FetchError, Transport};
use crate::policy::valid_app_id;
use crate::version::Version;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Was die Datei sein muss (aus dem geprüften Eintrag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expected {
    pub size: u64,
    pub sha256: [u8; 32],
}

/// Warum der Download scheiterte (englisch, nur Diagnose, ohne Pfade).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    #[error("download failed: {0}")]
    Fetch(FetchError),
    #[error("downloaded file has the wrong size")]
    Size,
    #[error("downloaded file has the wrong SHA-256")]
    Hash,
    #[error("not enough free space")]
    NoSpace,
    #[error("file error: {0:?}")]
    Io(ErrorKind),
}

fn io_error(e: &io::Error) -> DownloadError {
    if e.kind() == ErrorKind::StorageFull {
        DownloadError::NoSpace
    } else {
        DownloadError::Io(e.kind())
    }
}

/// Dateiname des Setups, gebaut aus geprüften Teilen (nie aus der Adresse).
pub fn setup_file_name(app_id: &str, version: &Version, kind: PackageKind) -> Option<String> {
    valid_app_id(app_id).then(|| format!("{app_id}-{version}-setup.{}", kind.extension()))
}

/// Es muss doppelt so viel frei sein wie das Setup groß ist (plan-update-bibliothek 5.4).
pub fn enough_space(free_bytes: u64, size: u64) -> bool {
    size.checked_mul(2).is_some_and(|need| free_bytes >= need)
}

struct Hashing<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(buf.get(..n).unwrap_or_default());
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn load(
    transport: &mut dyn Transport,
    url: &str,
    part: &Path,
    expected: &Expected,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), DownloadError> {
    let file = File::create(part).map_err(|e| io_error(&e))?;
    let mut sink = Hashing { inner: BufWriter::new(file), hasher: Sha256::new() };
    let got =
        fetch::get_to(transport, url, expected.size, &mut sink, &mut |n| progress(n, expected.size)).map_err(|e| {
            match e {
                FetchError::Write(ErrorKind::StorageFull) => DownloadError::NoSpace,
                FetchError::Write(k) => DownloadError::Io(k),
                other => DownloadError::Fetch(other),
            }
        })?;
    let digest: [u8; 32] = sink.hasher.finalize().into();
    let file = sink.inner.into_inner().map_err(|e| io_error(e.error()))?;
    file.sync_all().map_err(|e| io_error(&e))?;
    if got != expected.size {
        return Err(DownloadError::Size);
    }
    if digest != expected.sha256 {
        return Err(DownloadError::Hash);
    }
    Ok(())
}

/// Lädt `url` nach `dir/file_name`, geprüft gegen `expected`. `free_bytes`: freier Platz im Zielordner, falls
/// bekannt. `progress(geladen, gesamt)`. Liefert den Pfad der fertigen Datei.
pub fn download_verified(
    transport: &mut dyn Transport,
    url: &str,
    dir: &Path,
    file_name: &str,
    expected: &Expected,
    free_bytes: Option<u64>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, DownloadError> {
    if free_bytes.is_some_and(|free| !enough_space(free, expected.size)) {
        return Err(DownloadError::NoSpace);
    }
    fs::create_dir_all(dir).map_err(|e| io_error(&e))?;
    let done = dir.join(file_name);
    let part = dir.join(format!("{file_name}.part"));
    // Reste eines früheren Versuchs nie weiterverwenden.
    let _ = fs::remove_file(&part);
    let _ = fs::remove_file(&done);
    if let Err(e) = load(transport, url, &part, expected, progress) {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    if let Err(e) = fs::rename(&part, &done) {
        let _ = fs::remove_file(&part);
        return Err(io_error(&e));
    }
    Ok(done)
}

fn is_ours(name: &str) -> bool {
    name.ends_with(".part") || name.ends_with("-setup.exe") || name.ends_with("-setup.dmg")
}

/// Räumt beim Start alte Setups und abgebrochene Downloads im Update-Ordner auf. Liefert die Zahl gelöschter
/// Dateien; Unterordner und fremde Dateien bleiben. Ist `dir` eine Verknüpfung (Junction, Symlink), geschieht nichts
/// (Review N-U1-4). Nur unter der gemeinsamen Sperre (`lock`) aufrufen, sonst verschwindet ein laufender Download.
pub fn clean_leftovers(dir: &Path) -> usize {
    if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return 0;
    }
    let Ok(list) = fs::read_dir(dir) else { return 0 };
    list.filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.file_name().to_str().is_some_and(is_ours))
        .filter(|e| fs::remove_file(e.path()).is_ok())
        .count()
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
