//! Rohform von `entry.json`. Unbekannte Felder sind verboten; doppelte Felder lehnt serde bei Strukturen selbst ab,
//! bei der Paket-Tabelle sorgt `unique_map` dafür (sonst gewönne still der letzte Schlüssel).
use crate::entry::PackageKind;
use serde::de::{Deserializer, Error as _, MapAccess, Visitor};
use std::collections::BTreeMap;
use std::fmt;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawEntry {
    pub schema: u32,
    pub app: String,
    pub version: String,
    pub date: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub important: bool,
    #[serde(default, rename = "minLauncherVersion")]
    pub min_launcher_version: Option<String>,
    #[serde(default, rename = "registryKey")]
    pub registry_key: Option<String>,
    #[serde(deserialize_with = "unique_map")]
    pub packages: BTreeMap<String, RawPackage>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawPackage {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub kind: PackageKind,
}

/// Höchstzahl der Pakete (Plattformen) in einem Eintrag.
const MAX_PACKAGES: usize = 16;

struct UniqueMap;

impl<'de> Visitor<'de> for UniqueMap {
    type Value = BTreeMap<String, RawPackage>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a map of platform keys to packages")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut out = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, RawPackage>()? {
            if out.len() >= MAX_PACKAGES {
                return Err(A::Error::custom("too many packages"));
            }
            if out.insert(key, value).is_some() {
                return Err(A::Error::custom("duplicate platform key"));
            }
        }
        Ok(out)
    }
}

/// serde liest Strukturen auch aus JSON-Listen (`[1, "tasks", …]`). Erlaubt ist nur die Objekt-Form: oben ein
/// Objekt und jedes Paket ein Objekt.
pub(crate) fn object_shape(bytes: &[u8]) -> bool {
    let Ok(serde_json::Value::Object(top)) = serde_json::from_slice::<serde_json::Value>(bytes) else { return false };
    match top.get("packages") {
        Some(serde_json::Value::Object(p)) => p.values().all(serde_json::Value::is_object),
        _ => true,
    }
}

fn unique_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, RawPackage>, D::Error> {
    d.deserialize_map(UniqueMap)
}
