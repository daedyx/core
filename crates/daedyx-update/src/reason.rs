//! Warum eine Suche oder ein Download nichts installiert. Die Oberfläche wählt daraus den deutschen Text
//! (plan-update-bibliothek 7); `detail` ist englisch und nur für die Diagnose (ohne Pfade und Inhalte).
use crate::attest::{AttestError, AttestKind};
use crate::download::DownloadError;
use crate::entry::EntryError;
use crate::fetch::FetchError;

/// Art der Sperre, eine Zeile in der Tabelle der Nutzertexte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// Kein Netz, Zeitüberschreitung: still „später erneut“.
    Offline,
    /// GitHub antwortet mit 404/5xx.
    SourceUnavailable,
    /// Nachweis, Eintrag oder Datei passen nicht: „stammt nicht sicher von Daedyx“.
    NotTrusted,
    /// Diese Version kann den Nachweis nicht prüfen. Der Text nennt die erwartete Quelle
    /// (`UpdatePolicy::expected_source`), sagt nie „sicher“ und zeigt keinen Link (Review M-U1-1).
    NeedsManualUpdate,
    /// Die Quelle liefert eine ältere Version als schon gesehen: „bitte melden“.
    SourceWentBack,
    /// Download unterbrochen: Knopf „Erneut“.
    Interrupted,
    /// Zu wenig Speicherplatz.
    NoSpace,
    /// Lesen oder Schreiben im eigenen Ordner scheiterte.
    LocalFile,
}

/// Sperre mit Grund.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {detail}")]
pub struct Blocked {
    pub kind: BlockKind,
    pub detail: String,
}

impl Blocked {
    pub(crate) fn new(kind: BlockKind, detail: impl ToString) -> Blocked {
        Blocked { kind, detail: detail.to_string() }
    }
}

impl From<FetchError> for Blocked {
    fn from(e: FetchError) -> Blocked {
        let kind = match e {
            FetchError::Offline | FetchError::Timeout => BlockKind::Offline,
            FetchError::Status(_) => BlockKind::SourceUnavailable,
            FetchError::Interrupted => BlockKind::Interrupted,
            FetchError::Write(_) => BlockKind::LocalFile,
            // Fremder Rechner, kaputte Adresse, zu groß, zu viele Weiterleitungen: nicht unsere Quelle.
            FetchError::BadAddress | FetchError::ForeignHost | FetchError::TooManyRedirects | FetchError::TooLarge => {
                BlockKind::NotTrusted
            }
        };
        Blocked::new(kind, e)
    }
}

impl From<AttestError> for Blocked {
    fn from(e: AttestError) -> Blocked {
        let kind = if e.kind == AttestKind::Unsupported { BlockKind::NeedsManualUpdate } else { BlockKind::NotTrusted };
        Blocked::new(kind, e)
    }
}

impl From<EntryError> for Blocked {
    fn from(e: EntryError) -> Blocked {
        Blocked::new(BlockKind::NotTrusted, e)
    }
}

impl From<DownloadError> for Blocked {
    fn from(e: DownloadError) -> Blocked {
        match e {
            DownloadError::Fetch(f) => f.into(),
            DownloadError::Size | DownloadError::Hash => Blocked::new(BlockKind::NotTrusted, e),
            DownloadError::NoSpace => Blocked::new(BlockKind::NoSpace, e),
            DownloadError::Io(_) => Blocked::new(BlockKind::LocalFile, e),
        }
    }
}
