//! `updates.json` im Datenordner der App: Schalter „Beim Start suchen“, Zeit der letzten Suche und die höchste je
//! geprüft gesehene Version (Herabstufungsschutz, bleibt auch nach Neuinstallation einer älteren Fassung).
//! Ersetzt die Browser-Schlüssel `daedyx.updates.*`. Schreiben über Nebendatei und Umbenennen.
use crate::version::{self, Version};
use std::io::ErrorKind;
use std::path::Path;

pub const STATE_FILE: &str = "updates.json";
const MAX_STATE_BYTES: u64 = 4096;
/// Automatische Suche höchstens einmal am Tag (U-F2).
pub const AUTO_CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateState {
    /// Voreinstellung an (U-F2), abschaltbar.
    #[serde(default = "yes")]
    pub auto_check: bool,
    /// Unix-Sekunden der letzten Suche.
    #[serde(default)]
    pub last_check: Option<i64>,
    #[serde(default)]
    highest_seen: Option<String>,
}

impl Default for UpdateState {
    fn default() -> UpdateState {
        UpdateState { auto_check: true, last_check: None, highest_seen: None }
    }
}

/// Warum der Zustand nicht gelesen oder geschrieben werden konnte.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("updates.json is damaged")]
    Damaged,
    #[error("file error: {0:?}")]
    Io(ErrorKind),
}

impl UpdateState {
    /// Liest den Zustand; fehlt die Datei, gilt die Voreinstellung.
    pub fn load(dir: &Path) -> Result<UpdateState, StateError> {
        let path = dir.join(STATE_FILE);
        match std::fs::metadata(&path) {
            Ok(m) if m.len() > MAX_STATE_BYTES => return Err(StateError::Damaged),
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(UpdateState::default()),
            Err(e) => return Err(StateError::Io(e.kind())),
        }
        let bytes = std::fs::read(&path).map_err(|e| StateError::Io(e.kind()))?;
        if !matches!(serde_json::from_slice(&bytes), Ok(serde_json::Value::Object(_))) {
            return Err(StateError::Damaged);
        }
        let state: UpdateState = serde_json::from_slice(&bytes).map_err(|_| StateError::Damaged)?;
        if state.highest_seen.as_deref().is_some_and(|v| version::parse(v).is_err()) {
            return Err(StateError::Damaged);
        }
        Ok(state)
    }

    /// Wie `load`, aber ein beschädigter oder unlesbarer Zustand ergibt die Voreinstellung.
    pub fn load_or_default(dir: &Path) -> UpdateState {
        UpdateState::load(dir).unwrap_or_default()
    }

    /// Schreibt den Zustand (erst Nebendatei, dann umbenennen, damit nie eine halbe Datei bleibt). Die höchste
    /// gesehene Version wird mit dem Stand auf der Platte zusammengeführt, damit ein zweiter Schreiber (Launcher
    /// und App, ab U3) sie nie senkt (Review N-U1-3). Den Wettlauf zwischen Lesen und Umbenennen schließt nur die
    /// gemeinsame Sperre (`lock`): Aufrufer speichern unter ihr.
    pub fn save(&self, dir: &Path) -> Result<(), StateError> {
        let io = |e: std::io::Error| StateError::Io(e.kind());
        std::fs::create_dir_all(dir).map_err(io)?;
        let mut merged = self.clone();
        if let Some(on_disk) = UpdateState::load(dir).ok().and_then(|s| s.highest_seen()) {
            merged.note_seen(&on_disk);
        }
        // Eigene Nebendatei je Prozess, damit sich zwei Schreiber nicht die halbe Datei überschreiben.
        let tmp = dir.join(format!("{STATE_FILE}.{}.tmp", std::process::id()));
        let json = serde_json::to_vec_pretty(&merged).map_err(|_| StateError::Damaged)?;
        std::fs::write(&tmp, json).map_err(io)?;
        std::fs::rename(&tmp, dir.join(STATE_FILE)).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(e)
        })
    }

    pub fn highest_seen(&self) -> Option<Version> {
        self.highest_seen.as_deref().and_then(|v| version::parse(v).ok())
    }

    /// Vermerkt eine geprüfte neueste Version (nur aufwärts).
    pub fn note_seen(&mut self, latest: &Version) {
        self.highest_seen = Some(version::highest(self.highest_seen().as_ref(), latest).to_string());
    }

    /// Vermerkt die Zeit einer Suche.
    pub fn note_check(&mut self, now: i64) {
        self.last_check = Some(now);
    }

    /// `true`, wenn beim Start automatisch gesucht werden soll: Schalter an und letzte Suche mindestens 24 h her.
    /// Liegt die letzte Suche mehr als einen Tag in der Zukunft (Uhr war falsch), wird ebenfalls gesucht.
    pub fn due_for_auto_check(&self, now: i64) -> bool {
        self.auto_check
            && self.last_check.is_none_or(|t| {
                now.saturating_sub(t) >= AUTO_CHECK_INTERVAL_SECS || t > now.saturating_add(AUTO_CHECK_INTERVAL_SECS)
            })
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
