//! Wem eine Update-Quelle vertraut: Besitzer, Repo, deren **numerische** GitHub-Kennungen (Schutz gegen Neuanlage
//! nach Löschen/Umbenennen, Spike P4), Workflow-Pfad und Zweig. Für Apps fest im Programm; im Launcher aus der
//! attestierten `apps.json`. Feste Teile (Workflow, Zweig) sind keine Felder, damit sie niemand lockert.
use std::fmt;

/// Der einzige erlaubte Release-Workflow (im selben Repo).
pub const WORKFLOW: &str = ".github/workflows/release.yml";
/// Der einzige erlaubte Zweig, exakt (kein Tag, kein Muster).
pub const GIT_REF: &str = "refs/heads/main";

/// Ziel-Plattform eines Pakets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Platform {
    WindowsX64,
    MacosArm64,
    MacosX64,
}

impl Platform {
    /// Schlüssel in `entry.json`.
    pub fn key(self) -> &'static str {
        match self {
            Platform::WindowsX64 => "windows-x64",
            Platform::MacosArm64 => "macos-arm64",
            Platform::MacosX64 => "macos-x64",
        }
    }

    /// Die Plattform dieses Programms, `None` auf nicht unterstützten Systemen.
    pub fn current() -> Option<Platform> {
        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Platform::WindowsX64)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Platform::MacosArm64)
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some(Platform::MacosX64)
        } else {
            None
        }
    }
}

/// Ungültige Angabe beim Anlegen einer Policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid update policy: {0}")]
pub struct BadPolicy(&'static str);

/// Vertrauensregel einer Update-Quelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePolicy {
    owner: String,
    repo: String,
    owner_id: u64,
    repo_id: u64,
    app_id: String,
    platform: Platform,
}

/// GitHub-Namen: Buchstaben, Ziffern, `-`, `_`, `.`; nicht `.`/`..`, höchstens 100 Zeichen.
fn valid_github_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// App-Kennungen stehen in Datei- und Sperrnamen: nur `a-z`, `0-9`, `-`, erstes Zeichen kein `-`, höchstens 40.
pub fn valid_app_id(id: &str) -> bool {
    id.len() <= 40
        && id.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl UpdatePolicy {
    /// Legt die Regel an. Die Kennungen dürfen nicht 0 sein.
    pub fn new(
        owner: &str,
        repo: &str,
        owner_id: u64,
        repo_id: u64,
        app_id: &str,
        platform: Platform,
    ) -> Result<UpdatePolicy, BadPolicy> {
        if !valid_github_name(owner) || !valid_github_name(repo) {
            return Err(BadPolicy("owner or repo name"));
        }
        if owner_id == 0 || repo_id == 0 {
            return Err(BadPolicy("numeric ids must be set"));
        }
        if !valid_app_id(app_id) {
            return Err(BadPolicy("app id"));
        }
        Ok(UpdatePolicy {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            owner_id,
            repo_id,
            app_id: app_id.to_owned(),
            platform,
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn repo(&self) -> &str {
        &self.repo
    }
    pub fn owner_id(&self) -> u64 {
        self.owner_id
    }
    pub fn repo_id(&self) -> u64 {
        self.repo_id
    }
    pub fn app_id(&self) -> &str {
        &self.app_id
    }
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Adresse einer Datei des neuesten Releases (GitHub leitet auf den Download-Rechner weiter).
    pub fn latest_asset_url(&self, file: &str) -> String {
        format!("https://github.com/{}/{}/releases/latest/download/{file}", self.owner, self.repo)
    }

    /// Pfad-Anfang, unter dem die Pakete der Version `version` liegen müssen.
    pub fn release_path_prefix(&self, version: &semver::Version) -> String {
        format!("/{}/{}/releases/download/v{version}/", self.owner, self.repo)
    }

    /// Release-Seite für den Hinweis „bitte von Hand laden“ (feste Adresse, nie aus Fremddaten).
    pub fn releases_page(&self) -> String {
        format!("https://github.com/{}/{}/releases/latest", self.owner, self.repo)
    }
}

impl fmt::Display for UpdatePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} ({}/{}) {WORKFLOW}@{GIT_REF}", self.owner, self.repo, self.owner_id, self.repo_id)
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
