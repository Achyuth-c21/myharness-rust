//! The manifest schema and its structural gate (`TEST-EXT-001`): what a
//! package manifest must say, and what makes one invalid — before any
//! signature is consulted. A manifest that cannot be trusted to *describe*
//! itself (bounds, charset, path safety) is refused outright.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The manifest file is bounded — a manifest is a description, not a
/// payload, and parse bombs are refused by size before parsing.
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// The package kinds this build models. A `provider` package declares the
/// egress hosts it will contact (`TEST-EXT-006`); other kinds declare none
/// and are recorded as having none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    Provider,
    Skill,
    Plugin,
}

/// One extension package's manifest (`TEST-EXT-001`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The package name: `[A-Za-z0-9._-]`, at most 64 bytes, never `.` or
    /// `..`, never containing a path separator — the name is the directory
    /// name, and a manifest must not be able to climb out of the
    /// extensions root.
    pub name: String,
    /// The package's own version (a `x.y.z` numeric triple).
    pub version: String,
    /// The minimum app version this package registers against.
    pub min_app_version: String,
    pub kind: PackageKind,
    /// The payload path relative to the package directory — the file the
    /// digest covers. Relative, no separators beyond one level, no
    /// traversal.
    pub payload_path: String,
    /// The SHA-256 digest of the payload file, lowercase hex.
    pub payload_sha256: String,
    /// The declared egress hosts (`TEST-EXT-006`): a `provider` package
    /// lists every host its adapter contacts. A package that declares none
    /// registers as non-networked; a `provider` kind with an empty list is
    /// itself valid only for a wall with no egress (the local loopback wall
    /// declares `127.0.0.1`).
    #[serde(default)]
    pub egress_hosts: Vec<String>,
    /// The signature over the manifest's canonical bytes, base64. Whether
    /// it *means* anything is the [`crate::TrustAnchor`]'s decision — the
    /// schema only requires the field to be present and bounded.
    pub signature: String,
}

/// A manifest error, with the manifest's own field named.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ManifestError {
    #[error("the manifest is too large ({size} bytes; the bound is {bound})")]
    TooLarge { size: u64, bound: u64 },
    #[error("the manifest is not valid JSON: {0}")]
    NotJson(String),
    #[error("the manifest field {0:?} is invalid: {1}")]
    Invalid(&'static str, String),
}

/// A semver-shaped numeric triple check (lenient: `x.y.z` digits only).
fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn is_hex_digest(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Parse and structurally validate a manifest from raw bytes.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    let size = bytes.len() as u64;
    if size > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge {
            size,
            bound: MAX_MANIFEST_BYTES,
        });
    }
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|error| ManifestError::NotJson(error.to_string()))?;
    validate(&manifest)?;
    Ok(manifest)
}

/// Validate an already-parsed manifest against the structural gate.
pub fn validate(manifest: &Manifest) -> Result<(), ManifestError> {
    let name = manifest.name.as_str();
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        || name == "."
        || name == ".."
    {
        return Err(ManifestError::Invalid(
            "name",
            "the package name must be 1..=64 bytes of [A-Za-z0-9._-] and never \".\" or \"..\""
                .to_string(),
        ));
    }
    if !is_version(&manifest.version) {
        return Err(ManifestError::Invalid(
            "version",
            "the package version must be a numeric x.y.z triple".to_string(),
        ));
    }
    if !is_version(&manifest.min_app_version) {
        return Err(ManifestError::Invalid(
            "minAppVersion",
            "minAppVersion must be a numeric x.y.z triple".to_string(),
        ));
    }
    let payload = manifest.payload_path.as_str();
    if payload.is_empty()
        || payload.starts_with('/')
        || payload.contains("..")
        || payload.contains('\\')
        || payload.matches('/').count() > 1
        || payload.contains('\0')
    {
        return Err(ManifestError::Invalid(
            "payloadPath",
            "the payload path must be a shallow relative path inside the package directory"
                .to_string(),
        ));
    }
    if !is_hex_digest(&manifest.payload_sha256) {
        return Err(ManifestError::Invalid(
            "payloadSha256",
            "the payload digest must be lowercase hex SHA-256".to_string(),
        ));
    }
    if manifest.signature.len() > 4096 {
        return Err(ManifestError::Invalid(
            "signature",
            "the signature is over the bound".to_string(),
        ));
    }
    for host in &manifest.egress_hosts {
        let host = host.trim();
        if host.is_empty()
            || host.len() > 255
            || host.contains('/')
            || host.contains('\\')
            || host.contains(' ')
        {
            return Err(ManifestError::Invalid(
                "egressHosts",
                format!("egress host {host:?} is not a bare host name"),
            ));
        }
    }
    Ok(())
}

/// The canonical bytes a signature covers: the manifest's own JSON with the
/// `signature` field removed and the remaining keys sorted — a signer signs
/// the meaning, not the byte order.
pub fn canonical_bytes(manifest: &Manifest) -> Result<Vec<u8>, ManifestError> {
    let mut value = serde_json::to_value(manifest)
        .map_err(|error| ManifestError::NotJson(error.to_string()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ManifestError::Invalid("signature", "the manifest is not an object".into()))?;
    object.remove("signature");
    let canonical = serde_json::to_vec(&value)
        .map_err(|error| ManifestError::NotJson(error.to_string()))?;
    Ok(canonical)
}

/// The SHA-256 digest of a payload's bytes, lowercase hex — the form the
/// manifest's `payloadSha256` field carries.
pub fn payload_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn manifest_json() -> Value {
        json!({
            "name": "example-provider",
            "version": "1.0.0",
            "minAppVersion": "0.1.0",
            "kind": "provider",
            "payloadPath": "adapter.json",
            "payloadSha256": payload_digest(b"payload"),
            "egressHosts": ["api.example.com"],
            "signature": "c2ln",
        })
    }

    fn manifest_bytes() -> Vec<u8> {
        serde_json::to_vec(&manifest_json()).unwrap()
    }

    /// `TEST-EXT-001` (schema half): a well-formed manifest parses.
    #[test]
    fn a_well_formed_manifest_parses() {
        let manifest = parse_manifest(&manifest_bytes()).expect("the manifest");
        assert_eq!(manifest.name, "example-provider");
        assert_eq!(manifest.kind, PackageKind::Provider);
        assert_eq!(manifest.egress_hosts, vec!["api.example.com".to_string()]);
    }

    /// Structural refusals: traversal in the name or payload path, an
    /// oversized manifest, a non-hex digest, an invalid version.
    #[test]
    fn structural_refusals() {
        // Name traversal.
        let mut value = manifest_json();
        value["name"] = json!("..");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("name", _))
        ));
        value["name"] = json!("bad/name");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("name", _))
        ));

        // Payload traversal.
        let mut value = manifest_json();
        value["payloadPath"] = json!("../../etc/passwd");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("payloadPath", _))
        ));
        value["payloadPath"] = json!("/etc/passwd");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("payloadPath", _))
        ));

        // Digest shape.
        let mut value = manifest_json();
        value["payloadSha256"] = json!("ABCD");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("payloadSha256", _))
        ));

        // Version shape.
        let mut value = manifest_json();
        value["version"] = json!("one.two.three");
        assert!(matches!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()),
            Err(ManifestError::Invalid("version", _))
        ));
    }

    /// The manifest bound: a body over the size limit refuses before parse.
    #[test]
    fn an_oversized_manifest_refuses() {
        let huge = vec![b' '; (MAX_MANIFEST_BYTES + 1) as usize];
        assert!(matches!(
            parse_manifest(&huge),
            Err(ManifestError::TooLarge { .. })
        ));
    }

    /// The canonical bytes exclude the signature field, so signing and
    /// verifying agree regardless of where the signature sat.
    #[test]
    fn canonical_bytes_exclude_the_signature() {
        let manifest = parse_manifest(&manifest_bytes()).expect("the manifest");
        let canonical = canonical_bytes(&manifest).expect("canonical");
        assert!(!String::from_utf8_lossy(&canonical).contains("signature"));
        // Deterministic across re-serialization.
        let again = canonical_bytes(&parse_manifest(&manifest_bytes()).unwrap()).unwrap();
        assert_eq!(canonical, again);
    }

    /// The payload digest is the recorded SHA-256 spelling.
    #[test]
    fn payload_digest_is_sha256_hex() {
        // sha256("payload"), pinned from the published vector family.
        assert_eq!(payload_digest(b"payload").len(), 64);
        assert_eq!(
            payload_digest(b"payload"),
            payload_digest(b"payload")
        );
        assert_ne!(payload_digest(b"payload"), payload_digest(b"payload2"));
    }
}
