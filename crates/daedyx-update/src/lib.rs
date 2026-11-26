//! Daedyx Update-Bibliothek (plan-update-bibliothek, Auftrag U1): gemeinsam für Tasks, Desk und Launcher.
//! Ohne eigenes Netz und ohne Oberfläche; ohne Schlüssel, Zugangsdaten oder Token. Vertrauen entsteht nur aus dem
//! GitHub-Nachweis (Sigstore) und der fest eingetragenen Identität (`policy`). Echtes Netz gibt es mit dem
//! Merkmal `net` (`net::UreqTransport`, U3); den Start des Setups liefern die Aufrufer.

pub mod attest;
pub mod check;
pub mod download;
pub mod entry;
mod entry_raw;
pub mod fetch;
pub mod lock;
#[cfg(feature = "net")]
pub mod net;
pub mod policy;
pub mod reason;
pub mod state;
mod text;
pub mod version;

#[cfg(feature = "fuzzing")]
pub mod fuzzing;

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod test_support;
