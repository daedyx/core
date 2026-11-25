//! Versionen: nur reine SemVer `X.Y.Z` (keine Vorab- oder Bau-Kennung, kein `v`), und der Schutz gegen
//! Herabstufung: angeboten wird nur, was echt neuer als installiert und nicht kleiner als die höchste je geprüft
//! gesehene Version ist (plan-update-bibliothek 5.2).
pub use semver::Version;

/// Die Versionsangabe ist keine reine SemVer `X.Y.Z`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("version is not plain semver X.Y.Z")]
pub struct BadVersion;

/// Liest `X.Y.Z` ohne Vorab- und Bau-Kennung.
pub fn parse(text: &str) -> Result<Version, BadVersion> {
    let v = Version::parse(text).map_err(|_| BadVersion)?;
    if v.pre.is_empty() && v.build.is_empty() {
        Ok(v)
    } else {
        Err(BadVersion)
    }
}

/// `true`, wenn `candidate` echt neuer ist als `installed`.
pub fn is_newer(candidate: &Version, installed: &Version) -> bool {
    candidate > installed
}

/// Ergebnis des Versionsvergleichs einer geprüften neuesten Version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Echt neuer als installiert und nicht unter der höchsten gesehenen Version.
    Newer,
    /// Nichts zu tun (gleich oder älter als installiert).
    UpToDate,
    /// Die Quelle liefert weniger als schon einmal gesehen: melden, nie installieren.
    WentBack,
}

/// Entscheidet über eine geprüfte neueste Version `latest`.
pub fn decide(latest: &Version, installed: &Version, highest_seen: Option<&Version>) -> Decision {
    if highest_seen.is_some_and(|h| latest < h) {
        Decision::WentBack
    } else if is_newer(latest, installed) {
        Decision::Newer
    } else {
        Decision::UpToDate
    }
}

/// Neue höchste gesehene Version nach einer bestandenen Prüfung.
pub fn highest(seen: Option<&Version>, latest: &Version) -> Version {
    match seen {
        Some(h) if h > latest => h.clone(),
        _ => latest.clone(),
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
