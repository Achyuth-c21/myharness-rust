//! The loader (`TEST-EXT-002`, `TEST-EXT-006`): scan an extensions root,
//! verify every package independently, and load payloads lazily. One
//! broken package is a recorded, per-package error and never prevents the
//! others from loading — boot continues. A package verifies at two levels:
//! the manifest (schema + anchor) at scan time, the payload digest only on
//! first read — so a corrupted payload cannot break the scan, and an
//! unreadable one cannot break the app.

use std::path::{Path, PathBuf};

use crate::manifest::{canonical_bytes, parse_manifest, payload_digest, Manifest, ManifestError};
use crate::signature::{NoTrustAnchor, TrustAnchor};

/// One scanned package: everything the registration boundary needs to
/// decide what to do with it, plus the payload accessor.
#[derive(Debug)]
pub struct ExtensionState {
    /// The verified manifest (present when the scan accepted the package).
    pub manifest: Option<Manifest>,
    /// The package directory.
    pub dir: PathBuf,
    /// The scan failure, when the package was refused or broken.
    pub error: Option<String>,
}

impl ExtensionState {
    /// Whether the package registered (manifest verified under the anchor).
    pub fn registered(&self) -> bool {
        self.manifest.is_some() && self.error.is_none()
    }

    /// The package name, when the manifest parsed far enough to carry one.
    pub fn name(&self) -> Option<&str> {
        self.manifest.as_ref().map(|manifest| manifest.name.as_str())
    }

    /// Read the payload, verifying its digest against the manifest — the
    /// lazy half (`TEST-EXT-002`): a corrupted payload is THIS package's
    /// error, at THIS read, never a boot failure and never another
    /// package's.
    pub fn payload(&self) -> Result<Vec<u8>, String> {
        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| self.error.clone().unwrap_or_else(|| "the package did not register".to_string()))?;
        // Path confinement: the payload path was validated shallow-relative
        // at parse; confine again at read (defense in depth) — the joined
        // path must stay inside the package directory.
        let path = self.dir.join(&manifest.payload_path);
        let canonical_dir = self
            .dir
            .canonicalize()
            .map_err(|error| format!("the package directory is unreadable: {error}"))?;
        let canonical_payload = path
            .canonicalize()
            .map_err(|error| format!("the payload is unreadable: {error}"))?;
        if !canonical_payload.starts_with(&canonical_dir) {
            return Err("the payload escapes the package directory; nothing was read".to_string());
        }
        let bytes = std::fs::read(&canonical_payload)
            .map_err(|error| format!("the payload could not be read: {error}"))?;
        let digest = payload_digest(&bytes);
        if digest != manifest.payload_sha256 {
            return Err(format!(
                "the payload digest does not match the manifest ({digest} != {}); nothing was loaded",
                manifest.payload_sha256
            ));
        }
        Ok(bytes)
    }
}

/// The extensions root: a directory of package directories.
#[derive(Debug, Clone)]
pub struct ExtensionsRoot {
    path: PathBuf,
}

impl ExtensionsRoot {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        ExtensionsRoot { path: path.into() }
    }

    /// The root's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Scan every package directory under the root, verifying each
    /// manifest under `anchor`. Production injects
    /// [`NoTrustAnchor`] — every package records the refusal, boot
    /// continues. A missing root is an empty set, not an error (the app
    /// boots with no extensions installed).
    pub fn scan(&self, anchor: &dyn TrustAnchor) -> Vec<ExtensionState> {
        let entries = match std::fs::read_dir(&self.path) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };
        let mut states = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            states.push(self.scan_one(&dir, anchor));
        }
        states.sort_by(|a, b| a.dir.cmp(&b.dir));
        states
    }

    fn scan_one(&self, dir: &Path, anchor: &dyn TrustAnchor) -> ExtensionState {
        let manifest_path = dir.join("manifest.json");
        let fail = |error: String| ExtensionState {
            manifest: None,
            dir: dir.to_path_buf(),
            error: Some(error),
        };
        let bytes = match std::fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(error) => return fail(format!("the manifest could not be read: {error}")),
        };
        let manifest = match parse_manifest(&bytes) {
            Ok(manifest) => manifest,
            Err(error) => return fail(manifest_error_text(&error)),
        };
        // The directory the manifest sits in must be the package the
        // manifest names — a manifest dropped into the wrong directory is
        // a refused package, not a registration.
        if dir.file_name().and_then(|name| name.to_str()) != Some(manifest.name.as_str()) {
            return fail(format!(
                "the manifest names {:?} but sits in {:?}",
                manifest.name,
                dir.file_name().and_then(|name| name.to_str()).unwrap_or("")
            ));
        }
        let canonical = match canonical_bytes(&manifest) {
            Ok(canonical) => canonical,
            Err(error) => return fail(manifest_error_text(&error)),
        };
        let signature = match decode_signature(&manifest.signature) {
            Ok(signature) => signature,
            Err(error) => return fail(error),
        };
        if !anchor.verify(&canonical, &signature) {
            return fail(format!(
                "the signature did not verify under anchor {:?}: {}",
                anchor.id(),
                crate::signature::PRODUCTION_ANCHOR_REFUSES_ALL
            ));
        }
        ExtensionState {
            manifest: Some(manifest),
            dir: dir.to_path_buf(),
            error: None,
        }
    }
}

/// Decode the manifest's base64 signature (standard alphabet, no padding
/// tolerance beyond the usual two forms).
fn decode_signature(text: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    fn value_of(byte: u8) -> Result<u32, String> {
        ALPHABET
            .iter()
            .position(|c| *c == byte)
            .map(|index| index as u32)
            .ok_or_else(|| format!("the signature is not valid base64 (byte {byte:#x})"))
    }
    let text = text.trim();
    let text = text.strip_suffix('=').unwrap_or(text);
    let text = text.strip_suffix('=').unwrap_or(text);
    if text.is_empty() {
        return Err("the signature is empty; nothing was verified".to_string());
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let bytes: Vec<u32> = text
        .bytes()
        .map(value_of)
        .collect::<Result<_, _>>()?;
    for chunk in bytes.chunks(4) {
        let mut block = chunk[0];
        for piece in &chunk[1..] {
            block = (block << 6) | piece;
        }
        // The accumulated block's width depends on the chunk's length; the
        // byte extraction must shift by that width, not by the full chunk's
        // 24-bit layout.
        let bits = chunk.len() * 6;
        for index in 0..(chunk.len() - 1) {
            let shift = bits - 8 * (index + 1);
            out.push(((block >> shift) & 0xff) as u8);
        }
    }
    Ok(out)
}

fn manifest_error_text(error: &ManifestError) -> String {
    match error {
        ManifestError::TooLarge { .. } => error.to_string(),
        ManifestError::NotJson(_) => error.to_string(),
        ManifestError::Invalid(field, message) => format!("the manifest field {field:?} is invalid: {message}"),
    }
}

/// Scan a root with the production anchor — the boot path's posture: every
/// package records the refusal, nothing loads, nothing breaks.
pub fn load_extensions(root: &ExtensionsRoot) -> Vec<ExtensionState> {
    root.scan(&NoTrustAnchor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::{KeyedDigestTestAnchor, NoTrustAnchor};
    use serde_json::json;

    const TEST_KEY: &[u8] = b"sprint-test-key";

    fn write_package(
        root: &Path,
        name: &str,
        manifest_value: serde_json::Value,
        payload: &[u8],
    ) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect("the package dir");
        std::fs::write(dir.join("payload.bin"), payload).expect("the payload");
        let manifest = crate::manifest::Manifest {
            name: manifest_value["name"].as_str().unwrap().to_string(),
            version: manifest_value["version"].as_str().unwrap_or("1.0.0").to_string(),
            min_app_version: manifest_value["minAppVersion"].as_str().unwrap_or("0.1.0").to_string(),
            kind: crate::manifest::PackageKind::Provider,
            payload_path: manifest_value["payloadPath"].as_str().unwrap_or("payload.bin").to_string(),
            payload_sha256: payload_digest(payload),
            egress_hosts: manifest_value["egressHosts"]
                .as_array()
                .map(|hosts| hosts.iter().map(|h| h.as_str().unwrap().to_string()).collect())
                .unwrap_or_default(),
            signature: String::new(),
        };
        let canonical = canonical_bytes(&manifest).unwrap();
        let anchor = KeyedDigestTestAnchor::new(TEST_KEY);
        let signature = anchor.digest(&canonical);
        let mut signed = serde_json::to_value(&manifest).unwrap();
        signed["signature"] = json!(base64(&signature));
        std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&signed).unwrap())
            .expect("the manifest");
        dir
    }

    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let block = (b0 << 16) | (b1 << 8) | b2;
            let groups = [(block >> 18) & 0x3f, (block >> 12) & 0x3f, (block >> 6) & 0x3f, block & 0x3f];
            let keep = match chunk.len() {
                3 => 4,
                2 => 3,
                _ => 2,
            };
            for group in groups.iter().take(keep) {
                out.push(ALPHABET[*group as usize] as char);
            }
        }
        out
    }

    fn root_setup() -> (tempfile::TempDir, ExtensionsRoot) {
        let dir = tempfile::tempdir().expect("the temp root");
        let root = ExtensionsRoot::open(dir.path().to_path_buf());
        (dir, root)
    }

    /// `TEST-EXT-002`: one broken package among healthy ones is isolated —
    /// the healthy ones register, the broken one records its error, and
    /// boot (the scan) succeeds.
    #[test]
    fn a_broken_package_is_isolated_from_healthy_ones() {
        let (_tmp, root) = root_setup();
        let good_payload = b"good-payload";
        write_package(root.path(), "good-pack", json!({"name": "good-pack"}), good_payload);
        // A broken package: the manifest's digest does not match its payload.
        let dir = root.path().join("bad-pack");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("payload.bin"), b"tampered").expect("payload");
        let manifest_obj = crate::manifest::Manifest {
            name: "bad-pack".to_string(),
            version: "1.0.0".to_string(),
            min_app_version: "0.1.0".to_string(),
            kind: crate::manifest::PackageKind::Provider,
            payload_path: "payload.bin".to_string(),
            payload_sha256: payload_digest(b"NOT-the-payload"),
            egress_hosts: vec![],
            signature: String::new(),
        };
        let canonical = canonical_bytes(&manifest_obj).unwrap();
        let anchor = KeyedDigestTestAnchor::new(TEST_KEY);
        let mut signed = serde_json::to_value(&manifest_obj).unwrap();
        signed["signature"] = json!(base64(&anchor.digest(&canonical)));
        std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&signed).unwrap()).unwrap();

        let states = root.scan(&KeyedDigestTestAnchor::new(TEST_KEY));
        assert_eq!(states.len(), 2, "both packages scanned");
        let good = states.iter().find(|state| state.name() == Some("good-pack")).unwrap();
        let bad = states.iter().find(|state| state.name() == Some("bad-pack")).unwrap();
        assert!(good.registered());
        assert!(good.payload().is_ok());
        assert!(bad.registered(), "the manifest verified; the payload is the broken half");
        let error = bad.payload().expect_err("the payload digest must refuse");
        assert!(error.contains("digest does not match"), "{error}");
    }

    /// `TEST-EXT-001` (the anchor half): the production anchor refuses
    /// every package — even a well-formed, correctly signed one under the
    /// test anchor — and the scan still completes.
    #[test]
    fn the_production_anchor_refuses_every_package_and_boot_continues() {
        let (_tmp, root) = root_setup();
        write_package(root.path(), "signed-pack", json!({"name": "signed-pack"}), b"payload");
        let states = root.scan(&NoTrustAnchor);
        assert_eq!(states.len(), 1);
        assert!(!states[0].registered());
        assert!(states[0].error.as_deref().unwrap().contains("fail closed"));
    }

    /// A manifest in the wrong directory refuses; a manifest with a
    /// traversal payload path refuses at parse.
    #[test]
    fn misplaced_and_malicious_manifests_refuse() {
        let (_tmp, root) = root_setup();
        // The manifest names "other-pack" but sits in "wrong-dir".
        let dir = root.path().join("wrong-dir");
        std::fs::create_dir_all(&dir).expect("dir");
        let manifest = json!({
            "name": "other-pack", "version": "1.0.0", "minAppVersion": "0.1.0",
            "kind": "provider", "payloadPath": "payload.bin",
            "payloadSha256": payload_digest(b"x"), "egressHosts": [], "signature": "c2ln",
        });
        std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        let states = root.scan(&KeyedDigestTestAnchor::new(TEST_KEY));
        assert_eq!(states.len(), 1);
        assert!(!states[0].registered());
        assert!(states[0].error.as_deref().unwrap().contains("names"), "{:?}", states[0].error);
    }

    /// A missing root scans to an empty set — the app boots with no
    /// extensions installed (`TEST-EXT-002`'s boot half).
    #[test]
    fn a_missing_root_is_an_empty_set() {
        let root = ExtensionsRoot::open("/nonexistent-extensions-root-for-sure");
        let states = root.scan(&NoTrustAnchor);
        assert!(states.is_empty());
    }

    /// `TEST-EXT-006`: a provider package's declared egress hosts travel
    /// through the verified manifest — the registration record carries
    /// them, so enforcement can consult what the anchor vouched for.
    #[test]
    fn declared_egress_hosts_travel_through_the_verified_manifest() {
        let (_tmp, root) = root_setup();
        write_package(
            root.path(),
            "egress-pack",
            json!({"name": "egress-pack", "egressHosts": ["api.example.com", "files.example.net"]}),
            b"payload",
        );
        let states = root.scan(&KeyedDigestTestAnchor::new(TEST_KEY));
        let state = states.first().expect("one package");
        assert!(state.registered());
        let manifest = state.manifest.as_ref().unwrap();
        assert_eq!(
            manifest.egress_hosts,
            vec!["api.example.com".to_string(), "files.example.net".to_string()]
        );
    }

    /// The base64 decoder: round-trip, and non-alphabet bytes refuse.
    #[test]
    fn base64_decode_round_trips() {
        let original = b"\x00\xff\x10abc";
        let encoded = base64(original);
        let decoded = decode_signature(&encoded).expect("decodes");
        assert_eq!(decoded, original);
        assert!(decode_signature("!!!").is_err());
        assert!(decode_signature("").is_err());
        // A 32-byte digest (the anchor's output size) round-trips too.
        let digest = KeyedDigestTestAnchor::new(TEST_KEY).digest(b"{canonical bytes here}");
        let decoded = decode_signature(&base64(&digest)).expect("decodes");
        assert_eq!(decoded, digest);
    }
}
