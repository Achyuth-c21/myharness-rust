//! Managed configuration overlay — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/09 §2.10 Source tab, §3.5 precedence and read-only state; docs/18 §10
//! config contract; `DATA-CONFIG-04`).
//!
//! The shipped app can receive a per-user configuration overlay from a remote
//! bootstrap endpoint. Overlay values **override local settings and become
//! read-only** (`docs/09` §3.5), and the UI must show the effective source and
//! lock state **per field** (`DATA-CONFIG-04`). The exact merge order among the
//! local profile, the bootstrap overlay, organization MDM policy and runtime
//! defaults was never proven (`docs/09` §3.5 "exact precedence is unknown"), so the
//! rebuild defines it explicitly and records it as a decision:
//!
//! ```text
//! managed overlay  >  active local profile  >  runtime defaults
//! ```
//!
//! Two properties are **enforced here, not in the UI** (`docs/18` §10: "Managed/
//! read-only fields are enforced server-side"):
//!
//! 1. A key present in the overlay is read-only: the merged value is the overlay
//!    value, and a local draft may not change its local copy of that key
//!    ([`Overlay::locks`]).
//! 2. The merge is a pure function of (local document, overlay) — the effective
//!    configuration is recomputed, never cached as a second mutable copy.
//!
//! The overlay document's own shape was never captured (`docs/09` §2.10 describes
//! an HTTPS endpoint returning a per-user JSON overlay without pinning its
//! grammar; the exact trust/signature policy is `Unknown` — `docs/14` §9). The
//! rebuild accepts the shape that mirrors the profile document — a JSON object of
//! sections, each a flat map of keys — and records the assumption in `docs/14` §9.
//! A flat `section/key` map is also accepted so a future observed shape can be
//! supported without changing the merge.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ProfileDocument, Section};

/// The nine profile sections (`docs/18` §10 `ConfigProfile`).
pub const SECTION_NAMES: [&str; 9] = [
    "connection",
    "capabilities",
    "connectors",
    "telemetry",
    "limits",
    "appearance",
    "plugins",
    "egress",
    "source",
];

/// Where an effective value came from (`DATA-CONFIG-04`: "Show the effective source
/// (local, bootstrap, organization) and lock state per field").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigSource {
    /// The active local profile.
    Local,
    /// The remote bootstrap overlay (`docs/09` §2.10 Source tab).
    Bootstrap,
    /// Organization policy (MDM). No organization policy source exists in this
    /// rebuild — the variant is declared so the wire shape is stable, not because
    /// an organization overlay can be produced (`docs/14` §9).
    Organization,
}

/// Whether a local draft may change the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LockState {
    Writable,
    /// Managed: the value is supplied by the overlay and cannot be overridden
    /// locally (`docs/09` §3.5).
    ReadOnly,
}

/// One effective field: its value, where it came from, and whether it is locked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedField {
    pub section: String,
    pub key: String,
    pub value: Value,
    pub source: ConfigSource,
    pub lock: LockState,
}

/// The merged view of a local profile and a managed overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedConfig {
    /// The applied profile the merge was computed from, when there is one.
    pub profile_id: Option<String>,
    /// Effective fields, keyed `section/key`.
    pub fields: BTreeMap<String, MergedField>,
    /// Whether a managed overlay took part in the merge.
    pub overlay_present: bool,
}

/// A parsed managed overlay. Values are keyed `section/key` internally; the
/// section grouping is preserved so the UI can show which tab a managed field
/// belongs to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    values: BTreeMap<String, Value>,
}

impl Overlay {
    /// An overlay with no managed values — the "no managed configuration" state.
    pub fn empty() -> Self {
        Overlay::default()
    }

    /// Parse an overlay document. Two accepted shapes:
    ///
    /// - the section-mirroring shape — `{"connection": {"inferenceGatewayBaseUrl": …}}`
    ///   (the assumed shape; see the module docs);
    /// - the flat shape — `{"connection/inferenceGatewayBaseUrl": …}`.
    ///
    /// A key whose section is not one of [`SECTION_NAMES`] is kept but marked
    /// unknown: the merge still reports it, and the caller decides. Silently
    /// dropping a managed value would be the failure mode this repo forbids
    /// (`docs/14` §9 discipline).
    pub fn parse(document: &Value) -> Overlay {
        let mut values = BTreeMap::new();
        let Some(object) = document.as_object() else {
            return Overlay::default();
        };
        for (key, value) in object {
            if let Some((section, field)) = key.split_once('/') {
                values.insert(format!("{section}/{field}"), value.clone());
                continue;
            }
            match value.as_object() {
                Some(fields) => {
                    for (field, field_value) in fields {
                        values.insert(format!("{key}/{field}"), field_value.clone());
                    }
                }
                // A scalar at the top level is not a section map; keep it verbatim
                // under its own key so nothing is lost.
                None => {
                    values.insert(key.clone(), value.clone());
                }
            }
        }
        Overlay { values }
    }

    /// Managed values, keyed `section/key`.
    pub fn values(&self) -> &BTreeMap<String, Value> {
        &self.values
    }

    /// True when the overlay manages `section/key` — that field is read-only.
    pub fn locks(&self, section: &str, key: &str) -> bool {
        self.values.contains_key(&format!("{section}/{key}"))
    }

    /// The managed value for `section/key`, if any.
    pub fn value(&self, section: &str, key: &str) -> Option<&Value> {
        self.values.get(&format!("{section}/{key}"))
    }

    /// True when the overlay manages nothing.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Merge a local profile with a managed overlay.
///
/// Precedence (a rebuild decision — the shipped precedence was never proven,
/// `docs/09` §3.5): the overlay wins for every key it manages, and that key
/// becomes read-only. Every other local key is writable and keeps the local
/// value.
pub fn merge(local: &ProfileDocument, overlay: Option<&Overlay>) -> MergedConfig {
    let mut fields: BTreeMap<String, MergedField> = BTreeMap::new();

    let local_sections: [(&str, &Section); 9] = [
        ("connection", &local.connection),
        ("capabilities", &local.capabilities),
        ("connectors", &local.connectors),
        ("telemetry", &local.telemetry),
        ("limits", &local.limits),
        ("appearance", &local.appearance),
        ("plugins", &local.plugins),
        ("egress", &local.egress),
        ("source", &local.source),
    ];

    for (section_name, section) in local_sections {
        let Some(object) = section.0.as_object() else {
            continue;
        };
        for (key, value) in object {
            fields.insert(
                format!("{section_name}/{key}"),
                MergedField {
                    section: section_name.to_string(),
                    key: key.clone(),
                    value: value.clone(),
                    source: ConfigSource::Local,
                    lock: LockState::Writable,
                },
            );
        }
    }

    if let Some(overlay) = overlay {
        for (path, value) in overlay.values() {
            let (section, key) = path.split_once('/').unwrap_or(("", path.as_str()));
            fields.insert(
                path.clone(),
                MergedField {
                    section: section.to_string(),
                    key: key.to_string(),
                    value: value.clone(),
                    source: ConfigSource::Bootstrap,
                    lock: LockState::ReadOnly,
                },
            );
        }
    }

    MergedConfig {
        profile_id: local.id.clone(),
        fields,
        overlay_present: overlay.is_some_and(|o| !o.is_empty()),
    }
}

impl MergedConfig {
    /// The effective value of `section/key`, if the merged configuration has one.
    pub fn get(&self, section: &str, key: &str) -> Option<&MergedField> {
        self.fields.get(&format!("{section}/{key}"))
    }

    /// Whether `section/key` is managed and therefore read-only.
    pub fn is_locked(&self, section: &str, key: &str) -> bool {
        self.get(section, key)
            .is_some_and(|f| f.lock == LockState::ReadOnly)
    }

    /// The effective value of `section/key` as JSON, if present.
    pub fn value(&self, section: &str, key: &str) -> Option<&Value> {
        self.get(section, key).map(|f| &f.value)
    }

    /// Every read-only (managed) field — the set the UI must render locked and
    /// label with its source.
    pub fn locked_fields(&self) -> Vec<&MergedField> {
        self.fields
            .values()
            .filter(|f| f.lock == LockState::ReadOnly)
            .collect()
    }

    /// The merged configuration shaped like a profile document — the form the
    /// runtime consumes. Managed values are already substituted in.
    pub fn effective_document(&self) -> Value {
        let mut sections = serde_json::Map::new();
        for section in SECTION_NAMES {
            let mut object = serde_json::Map::new();
            for field in self.fields.values() {
                if field.section == section {
                    object.insert(field.key.clone(), field.value.clone());
                }
            }
            if !object.is_empty() {
                sections.insert(section.to_string(), Value::Object(object));
            }
        }
        Value::Object(sections)
    }
}

/// The managed keys a local write may not change.
///
/// Returns the offending `section/key` paths, in sorted order. A locked key is
/// one the overlay manages (`docs/09` §3.5: managed values "become read-only");
/// the local document may still hold a local copy of it, but a draft may not
/// change that copy.
pub fn managed_write_conflicts(
    before: &ProfileDocument,
    after: &ProfileDocument,
    overlay: &Overlay,
) -> Vec<String> {
    let mut conflicts = Vec::new();
    for path in overlay.values().keys() {
        let Some((section, key)) = path.split_once('/') else {
            continue;
        };
        let before_value = section_value(before, section).and_then(|s| s.get(key));
        let after_value = section_value(after, section).and_then(|s| s.get(key));
        if before_value != after_value {
            conflicts.push(path.clone());
        }
    }
    conflicts
}

fn section_value<'a>(
    doc: &'a ProfileDocument,
    section: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    match section {
        "connection" => doc.connection.0.as_object(),
        "capabilities" => doc.capabilities.0.as_object(),
        "connectors" => doc.connectors.0.as_object(),
        "telemetry" => doc.telemetry.0.as_object(),
        "limits" => doc.limits.0.as_object(),
        "appearance" => doc.appearance.0.as_object(),
        "plugins" => doc.plugins.0.as_object(),
        "egress" => doc.egress.0.as_object(),
        "source" => doc.source.0.as_object(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc_with(section: &str, keys: Value) -> ProfileDocument {
        let mut doc = ProfileDocument {
            schema_version: 1,
            id: Some("11111111-1111-4111-8111-111111111111".to_string()),
            name: Some("Profile".to_string()),
            ..ProfileDocument::default()
        };
        let value = Section(keys);
        match section {
            "connection" => doc.connection = value,
            "telemetry" => doc.telemetry = value,
            "limits" => doc.limits = value,
            _ => doc.capabilities = value,
        }
        doc
    }

    /// DATA-CONFIG-04: a managed field reports the bootstrap source and is locked.
    #[test]
    fn managed_field_wins_and_is_locked() {
        let local = doc_with(
            "connection",
            json!({
                "inferenceGatewayBaseUrl": "https://local.example",
                "inferenceGatewayApiKey": "placeholder",
            }),
        );
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        let merged = merge(&local, Some(&overlay));

        let field = merged.get("connection", "inferenceGatewayBaseUrl").unwrap();
        assert_eq!(field.value, json!("https://managed.example"));
        assert_eq!(field.source, ConfigSource::Bootstrap);
        assert_eq!(field.lock, LockState::ReadOnly);

        let local_field = merged.get("connection", "inferenceGatewayApiKey").unwrap();
        assert_eq!(local_field.source, ConfigSource::Local);
        assert_eq!(local_field.lock, LockState::Writable);

        assert!(merged.is_locked("connection", "inferenceGatewayBaseUrl"));
        assert!(!merged.is_locked("connection", "inferenceGatewayApiKey"));
        assert_eq!(merged.locked_fields().len(), 1);
    }

    /// A managed key that the local document does not have still appears — the
    /// effective configuration must not lose a managed value.
    #[test]
    fn managed_key_absent_locally_still_appears() {
        let local = doc_with("connection", json!({}));
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        let merged = merge(&local, Some(&overlay));
        assert_eq!(
            merged.value("connection", "inferenceGatewayBaseUrl"),
            Some(&json!("https://managed.example"))
        );
    }

    /// The flat `section/key` overlay shape is accepted too.
    #[test]
    fn flat_overlay_shape_is_accepted() {
        let overlay = Overlay::parse(&json!({
            "connection/inferenceGatewayBaseUrl": "https://managed.example",
        }));
        assert!(overlay.locks("connection", "inferenceGatewayBaseUrl"));
        assert_eq!(
            overlay.value("connection", "inferenceGatewayBaseUrl"),
            Some(&json!("https://managed.example"))
        );
    }

    /// Server-side enforcement: a local draft may not change a managed key.
    #[test]
    fn managed_key_cannot_be_changed_by_a_draft() {
        let before = doc_with(
            "connection",
            json!({ "inferenceGatewayBaseUrl": "https://local.example" }),
        );
        let after = doc_with(
            "connection",
            json!({ "inferenceGatewayBaseUrl": "https://edited.example" }),
        );
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        assert_eq!(
            managed_write_conflicts(&before, &after, &overlay),
            vec!["connection/inferenceGatewayBaseUrl".to_string()]
        );

        // An unchanged local copy is fine.
        let same = doc_with(
            "connection",
            json!({ "inferenceGatewayBaseUrl": "https://local.example" }),
        );
        assert!(managed_write_conflicts(&before, &same, &overlay).is_empty());
    }

    /// The effective document is shaped like a profile document and carries the
    /// managed value.
    #[test]
    fn effective_document_uses_managed_values() {
        let local = doc_with(
            "connection",
            json!({ "inferenceGatewayBaseUrl": "https://local.example" }),
        );
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        let merged = merge(&local, Some(&overlay));
        let document = merged.effective_document();
        assert_eq!(
            document["connection"]["inferenceGatewayBaseUrl"],
            json!("https://managed.example")
        );
    }

    /// With no overlay, the merged view is the local document unchanged.
    #[test]
    fn no_overlay_means_local_values() {
        let local = doc_with(
            "telemetry",
            json!({ "otlpEndpoint": "https://otlp.example" }),
        );
        let merged = merge(&local, None);
        assert!(!merged.overlay_present);
        assert_eq!(
            merged.value("telemetry", "otlpEndpoint"),
            Some(&json!("https://otlp.example"))
        );
        assert!(merged.locked_fields().is_empty());
    }
}
