//! `entry.json` streng lesen: höchstens 64 KB, unbekannte Felder und doppelte Schlüssel verboten, `schema` = 1,
//! `app` = erwartete Kennung, Version reine SemVer, Paket der eigenen Plattform auf dem eigenen Release-Pfad bei
//! GitHub. Gelesen wird erst, **nachdem** der Nachweis die Datei bestätigt hat (`check`); die Prüfungen hier
//! schützen trotzdem für sich allein (Fuzz-Ziel `catalog_entry`).
use crate::entry_raw::{object_shape, RawEntry, RawPackage};
use crate::policy::{Platform, UpdatePolicy};
use crate::version::{self, Version};

/// Höchstgröße von `entry.json`.
pub const MAX_ENTRY_BYTES: usize = 64 * 1024;
/// Absolute Obergrenze für ein Setup (SICHERHEIT 2.3).
pub const MAX_SETUP_BYTES: u64 = 512 * 1024 * 1024;
/// Höchstlänge der Änderungsnotiz in Zeichen.
pub const MAX_NOTES_CHARS: usize = 8000;

/// Art des Pakets; legt die Dateiendung fest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    NsisSilent,
    Dmg,
}

impl PackageKind {
    pub fn extension(self) -> &'static str {
        match self {
            PackageKind::NsisSilent => "exe",
            PackageKind::Dmg => "dmg",
        }
    }

    fn fits(self, platform: Platform) -> bool {
        match platform {
            Platform::WindowsX64 => self == PackageKind::NsisSilent,
            Platform::MacosArm64 | Platform::MacosX64 => self == PackageKind::Dmg,
        }
    }
}

/// Das Paket für die eigene Plattform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub url: String,
    pub sha256: [u8; 32],
    pub size: u64,
    pub kind: PackageKind,
}

/// Ein geprüfter Eintrag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub version: Version,
    pub date: String,
    /// Reiner Text, ohne Steuer- und Richtungszeichen; die Oberfläche zeigt ihn ohne Links und Markdown.
    pub notes: String,
    /// Nur ein deutlicherer Hinweis (U-F5), nie erzwungen.
    pub important: bool,
    pub min_launcher_version: Option<Version>,
    pub registry_key: Option<String>,
    pub package: Package,
}

/// Warum ein Eintrag abgelehnt wurde (englisch, nur für die Diagnose, ohne Inhalte).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EntryError {
    #[error("entry.json is larger than 64 KB")]
    TooLarge,
    #[error("entry.json is not valid: {0}")]
    Malformed(String),
    #[error("unsupported entry schema")]
    Schema,
    #[error("entry belongs to another app")]
    WrongApp,
    #[error("entry version is not plain semver")]
    Version,
    #[error("entry field {0} is not valid")]
    Field(&'static str),
    #[error("entry has no package for this platform")]
    NoPackage,
    #[error("package address is not on this app's release path")]
    Url,
}

fn printable(s: &str, allow_newline: bool) -> bool {
    s.chars().all(|c| {
        let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}');
        let control = c.is_control() && !(allow_newline && matches!(c, '\n' | '\t'));
        !bidi && !control
    })
}

fn date_ok(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10 && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
}

fn sha256_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(s, &mut out).ok()?;
    Some(out)
}

fn file_name_ok(name: &str, ext: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && name.rsplit_once('.').is_some_and(|(_, e)| e == ext)
}

fn package(raw: &RawPackage, policy: &UpdatePolicy, v: &Version) -> Result<Package, EntryError> {
    if !raw.kind.fits(policy.platform()) {
        return Err(EntryError::Field("kind"));
    }
    let prefix = format!("https://github.com{}", policy.release_path_prefix(v));
    let name = raw.url.strip_prefix(&prefix).ok_or(EntryError::Url)?;
    if !file_name_ok(name, raw.kind.extension()) {
        return Err(EntryError::Url);
    }
    let sha256 = sha256_hex(&raw.sha256).ok_or(EntryError::Field("sha256"))?;
    if raw.size == 0 || raw.size > MAX_SETUP_BYTES {
        return Err(EntryError::Field("size"));
    }
    Ok(Package { url: raw.url.clone(), sha256, size: raw.size, kind: raw.kind })
}

/// Liest und prüft `entry.json` für `policy`.
pub fn parse(bytes: &[u8], policy: &UpdatePolicy) -> Result<Entry, EntryError> {
    if bytes.len() > MAX_ENTRY_BYTES {
        return Err(EntryError::TooLarge);
    }
    let raw: RawEntry = serde_json::from_slice(bytes).map_err(|e| EntryError::Malformed(e.classify_text()))?;
    if !object_shape(bytes) {
        return Err(EntryError::Malformed("entry must be a JSON object".into()));
    }
    if raw.schema != 1 {
        return Err(EntryError::Schema);
    }
    if raw.app != policy.app_id() {
        return Err(EntryError::WrongApp);
    }
    let version = version::parse(&raw.version).map_err(|_| EntryError::Version)?;
    if !date_ok(&raw.date) {
        return Err(EntryError::Field("date"));
    }
    if raw.notes.chars().count() > MAX_NOTES_CHARS || !printable(&raw.notes, true) {
        return Err(EntryError::Field("notes"));
    }
    let min_launcher_version = match &raw.min_launcher_version {
        Some(t) => Some(version::parse(t).map_err(|_| EntryError::Field("minLauncherVersion"))?),
        None => None,
    };
    if let Some(k) = &raw.registry_key
        && (k.is_empty() || k.len() > 200 || k.contains('\\') || !printable(k, false))
    {
        return Err(EntryError::Field("registryKey"));
    }
    let raw_pkg = raw.packages.get(policy.platform().key()).ok_or(EntryError::NoPackage)?;
    let package = package(raw_pkg, policy, &version)?;
    Ok(Entry {
        version,
        date: raw.date,
        notes: raw.notes,
        important: raw.important,
        min_launcher_version,
        registry_key: raw.registry_key,
        package,
    })
}

/// Fehlerart von serde_json ohne Zeileninhalt.
trait ClassifyText {
    fn classify_text(&self) -> String;
}

impl ClassifyText for serde_json::Error {
    fn classify_text(&self) -> String {
        format!("{:?} error at line {} column {}", self.classify(), self.line(), self.column())
    }
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
