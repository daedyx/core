//! Dünne Hülle um `attestation-verify` (Spike S-ATT, D5): Prüft offline, dass eine Datei (über ihren SHA-256)
//! im GitHub-Nachweis steht, den genau unser Release-Workflow auf `main` erzeugt hat. Vertrauenswurzel ist die
//! eigene Datei `trusted_root.json` (Schnappschuss `sigstore/root-signing`, nicht die Kopie der Bibliothek).
//! Ein Tausch gegen `sigstore-verify` betrifft nur diese Datei.
use crate::policy::{UpdatePolicy, GIT_REF, WORKFLOW};
use attestation_verify::{
    Bundle, CertificateError, CheckpointOriginPolicy, Error, GithubPolicy, RefPolicy, RepositoryIdentity, SignerPolicy,
    SourcePolicy, Subject, TimestampError, TransparencyError, TrustError, TrustStore, Verifier, WorkflowPath,
    WorkflowRevisionPolicy,
};

/// Höchstgröße eines Bündels (`provenance.sigstore.json`).
pub const MAX_BUNDLE_BYTES: usize = 256 * 1024;
/// Signierter Name des Rekor-v1-Logs (Checkpoint-Herkunft).
const REKOR_ORIGIN: &str = "rekor.sigstore.dev - 1193050959916656506";
const PREDICATE: &str = "https://slsa.dev/provenance/v1";
static TRUSTED_ROOT: &[u8] = include_bytes!("trusted_root.json");

/// Art des Fehlschlags; daraus wählt die Oberfläche den deutschen Text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestKind {
    /// Bündel zu groß oder kein gültiges Bündel.
    Malformed,
    /// Nachweis gültig, aber aus anderem Repo, Workflow oder Zweig.
    WrongOrigin,
    /// Die Datei steht nicht im Nachweis.
    NotInBundle,
    /// Kette, Signatur, Log oder Zertifikat falsch.
    Invalid,
    /// Diese Version kann den Nachweis nicht prüfen (Rekor v2, neue Wurzel): von Hand aktualisieren.
    Unsupported,
}

/// Fehlschlag mit englischem Grund für die Diagnose (ohne Pfade und Inhalte).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {detail}")]
pub struct AttestError {
    pub kind: AttestKind,
    pub detail: String,
}

impl AttestError {
    fn new(kind: AttestKind, detail: impl ToString) -> AttestError {
        AttestError { kind, detail: detail.to_string() }
    }
}

/// Prüf-Schnittstelle; in Tests der Ablauf-Schicht durch eine Attrappe ersetzbar.
pub trait Provenance {
    /// `Ok`, wenn `sha256` im Bündel steht und das Bündel echt von unserer Quelle stammt.
    fn verify(&self, sha256: &[u8; 32], bundle: &[u8]) -> Result<(), AttestError>;
}

/// Schlägt fehl, wenn eine neuere Wurzel oder ein neues Log-Format nötig wäre (Text „von Hand aktualisieren“).
fn needs_newer_verifier(e: &Error) -> bool {
    matches!(
        e,
        Error::Unsupported(_)
            | Error::Trust(TrustError::NoTrustedKeyValidAt)
            | Error::Certificate(CertificateError::OutsideCaValidity | CertificateError::UnknownCtLog)
            | Error::Transparency(TransparencyError::UnknownLogKey)
            | Error::Timestamp(TimestampError::IntegratedTimeOutsideLogKeyValidity)
    )
}

fn classify(e: &Error) -> AttestError {
    let kind = match e {
        _ if needs_newer_verifier(e) => AttestKind::Unsupported,
        Error::Parse(_) | Error::ResourceLimit(_) => AttestKind::Malformed,
        Error::Policy(_) => AttestKind::WrongOrigin,
        Error::ContentBinding(attestation_verify::ContentBindingError::SubjectNotFound) => AttestKind::NotInBundle,
        _ => AttestKind::Invalid,
    };
    AttestError::new(kind, e)
}

/// Prüfer für GitHub-Nachweise einer Quelle.
pub struct GithubAttestation {
    verifier: Verifier,
    repo: String,
}

fn repo_identity(policy: &UpdatePolicy) -> Result<RepositoryIdentity, Error> {
    Ok(RepositoryIdentity::new(policy.owner(), policy.repo())?
        .with_owner_id(policy.owner_id())
        .with_repository_id(policy.repo_id()))
}

impl GithubAttestation {
    /// Prüfer mit der eingebauten Vertrauenswurzel.
    pub fn new(policy: &UpdatePolicy) -> Result<GithubAttestation, AttestError> {
        GithubAttestation::with_root(policy, TRUSTED_ROOT)
    }

    pub(crate) fn with_root(policy: &UpdatePolicy, root: &[u8]) -> Result<GithubAttestation, AttestError> {
        let build = || -> Result<Verifier, Error> {
            let trust = TrustStore::from_json(root)?;
            let rules = GithubPolicy::builder()
                .source(SourcePolicy {
                    repository: repo_identity(policy)?,
                    git_ref: RefPolicy::Exact(GIT_REF.to_owned()),
                    commit: None,
                })
                // Numerische IDs nur auf der Quellseite: Fulcio beglaubigt für das Signer-Repo nur den Namen.
                // Der Workflow muss im selben Repo liegen; das prüft `verify` zusätzlich.
                .signer(SignerPolicy {
                    repository: RepositoryIdentity::new(policy.owner(), policy.repo())?,
                    path: WorkflowPath::new(WORKFLOW)?,
                    revision: WorkflowRevisionPolicy::Any,
                })
                .build()?;
            // Das erste Log der Wurzel ist Rekor v1 (Spike 6); ohne Log scheitert der Bau des Prüfers unten.
            let mut origins = CheckpointOriginPolicy::builder();
            if let Some(log) = trust.tlogs.first() {
                origins = origins.allow_origin(log, REKOR_ORIGIN)?;
            }
            let origins = origins.build()?;
            Verifier::builder().trust_store(trust).github_policy(rules).checkpoint_origin_policy(origins).build()
        };
        let verifier = build().map_err(|e| AttestError::new(AttestKind::Unsupported, e))?;
        Ok(GithubAttestation { verifier, repo: format!("{}/{}", policy.owner(), policy.repo()) })
    }
}

impl Provenance for GithubAttestation {
    fn verify(&self, sha256: &[u8; 32], bundle: &[u8]) -> Result<(), AttestError> {
        if bundle.len() > MAX_BUNDLE_BYTES {
            return Err(AttestError::new(AttestKind::Malformed, "bundle larger than 256 KB"));
        }
        let bundle = Bundle::from_json(bundle).map_err(|e| classify(&e))?;
        let subject = Subject::from_digest_hex(&hex::encode(sha256)).map_err(|e| classify(&e))?;
        let report = self.verifier.verify_digest(&subject, &bundle).map_err(|e| classify(&e))?;
        // Zweite Sicherung: die bestätigten Angaben noch einmal selbst vergleichen.
        let s = &report.signer;
        let same = |a: &str| a.eq_ignore_ascii_case(&self.repo);
        if !(same(&s.source_repository)
            && same(&s.signer_repository)
            && s.source_ref == GIT_REF
            && s.signer_workflow_path == WORKFLOW
            && report.statement.predicate_type == PREDICATE)
        {
            return Err(AttestError::new(AttestKind::WrongOrigin, "verified identity differs from policy"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "attest_tests.rs"]
mod tests;
