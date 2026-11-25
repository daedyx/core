//! Ablauf einer Suche und des Downloads nach dem Klick (plan-update-bibliothek 4). Jede Stufe bricht ab; nichts
//! wird gelesen, bevor der Nachweis es bestätigt hat, und nichts bleibt liegen.
//! 1. `entry.json` (≤ 64 KB) und Bündel (≤ 256 KB) holen.
//! 2. Nachweis für `entry.json`, dann Eintrag lesen, dann Nachweis für das Setup-Subjekt aus dem Eintrag.
//! 3. Version gegen installiert und höchste gesehene Version.
//! 4. Erst nach Klick: Setup laden, Größe und SHA-256 prüfen, Subjekt noch einmal gegen das Bündel prüfen.
use crate::attest::{Provenance, MAX_BUNDLE_BYTES};
use crate::download::{self, Expected};
use crate::entry::{self, Entry, MAX_ENTRY_BYTES};
use crate::fetch::{self, Transport};
use crate::policy::UpdatePolicy;
use crate::reason::{BlockKind, Blocked};
use crate::version::{decide, Decision, Version};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Dateiname des Eintrags im Release.
pub const ENTRY_FILE: &str = "entry.json";
/// Dateiname des Bündels im Release.
pub const BUNDLE_FILE: &str = "provenance.sigstore.json";

/// Ein geprüftes Angebot. Felder privat: niemand kann Eintrag und Bündel nachträglich vertauschen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    entry: Entry,
    bundle: Vec<u8>,
}

impl Offer {
    pub fn entry(&self) -> &Entry {
        &self.entry
    }
}

/// Ergebnis einer Suche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Geprüft, nichts Neueres. `latest` darf als gesehen vermerkt werden.
    UpToDate {
        latest: Version,
    },
    /// Geprüft und neuer; Download erst nach Klick (`fetch_setup`).
    Available(Box<Offer>),
    Blocked(Blocked),
}

impl Outcome {
    /// Die geprüfte neueste Version, die in `state` als gesehen vermerkt werden darf.
    pub fn verified_latest(&self) -> Option<&Version> {
        match self {
            Outcome::UpToDate { latest } => Some(latest),
            Outcome::Available(o) => Some(&o.entry.version),
            Outcome::Blocked(_) => None,
        }
    }
}

fn sha(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn checked_offer(
    transport: &mut dyn Transport,
    provenance: &dyn Provenance,
    policy: &UpdatePolicy,
) -> Result<Offer, Blocked> {
    let entry_bytes = fetch::get_bytes(transport, &policy.latest_asset_url(ENTRY_FILE), MAX_ENTRY_BYTES as u64)?;
    let bundle = fetch::get_bytes(transport, &policy.latest_asset_url(BUNDLE_FILE), MAX_BUNDLE_BYTES as u64)?;
    provenance.verify(&sha(&entry_bytes), &bundle)?;
    let entry = entry::parse(&entry_bytes, policy)?;
    provenance.verify(&entry.package.sha256, &bundle)?;
    Ok(Offer { entry, bundle })
}

/// Sucht nach einer neueren Version. `installed`: laufende Version; `highest_seen` aus `state`.
pub fn check(
    transport: &mut dyn Transport,
    provenance: &dyn Provenance,
    policy: &UpdatePolicy,
    installed: &Version,
    highest_seen: Option<&Version>,
) -> Outcome {
    let offer = match checked_offer(transport, provenance, policy) {
        Ok(o) => o,
        Err(b) => return Outcome::Blocked(b),
    };
    match decide(&offer.entry.version, installed, highest_seen) {
        Decision::Newer => Outcome::Available(Box::new(offer)),
        Decision::UpToDate => Outcome::UpToDate { latest: offer.entry.version },
        Decision::WentBack => {
            Outcome::Blocked(Blocked::new(BlockKind::SourceWentBack, "latest release is older than one seen before"))
        }
    }
}

/// Lädt das Setup eines Angebots nach `dir` (eigener Update-Ordner der App, nicht `%TEMP%`). Die Sperre
/// (`lock::Lock`) hält der Aufrufer vom Download bis zum Start des Setups.
pub fn fetch_setup(
    transport: &mut dyn Transport,
    provenance: &dyn Provenance,
    policy: &UpdatePolicy,
    offer: &Offer,
    dir: &Path,
    free_bytes: Option<u64>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, Blocked> {
    let pkg = &offer.entry.package;
    let name = download::setup_file_name(policy.app_id(), &offer.entry.version, pkg.kind)
        .ok_or_else(|| Blocked::new(BlockKind::NotTrusted, "app id"))?;
    let expected = Expected { size: pkg.size, sha256: pkg.sha256 };
    let path = download::download_verified(transport, &pkg.url, dir, &name, &expected, free_bytes, progress)?;
    if let Err(e) = provenance.verify(&expected.sha256, &offer.bundle) {
        let _ = std::fs::remove_file(&path);
        return Err(e.into());
    }
    Ok(path)
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
