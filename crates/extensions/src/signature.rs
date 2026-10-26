//! The trust-anchor seam (`TEST-EXT-001`): whether a manifest's signature
//! *means* anything is this module's decision, never the manifest's own
//! claim.
//!
//! The production anchor is [`NoTrustAnchor`] and **refuses every
//! package**. No production signing credential or captured trust anchor
//! exists in this build; a verifier that accepted an unknown key would be
//! a policy bypass — the recorded fail-closed posture of the catalog
//! signature (`docs/14` §9 item 20) and the update feed (item 70).
//! Provisioning a real anchor is an owner decision with a key ceremony;
//! until then the framework is complete and the gate is shut.
//!
//! The test-only keyed-digest anchor exercises the verification path end to
//! end without introducing a real signature scheme: it is documented as a
//! stand-in, never registered as production trust.

use sha2::{Digest, Sha256};

/// Whether a manifest's signature verifies under this anchor. The bytes are
/// the manifest's canonical form (the signature field excluded).
pub trait TrustAnchor: Send + Sync {
    /// The anchor's identity, for records and refusal messages.
    fn id(&self) -> &'static str;
    /// Verify `signature` over `canonical_manifest_bytes`.
    fn verify(&self, canonical_manifest_bytes: &[u8], signature: &[u8]) -> bool;
}

/// The production anchor: **refuses every signature** (`docs/14` §9). The
/// constant is exported for tests and docs so the refusal is pinned by
/// name, not by accident.
pub const PRODUCTION_ANCHOR_REFUSES_ALL: &str =
    "the production trust anchor is not provisioned; every extension signature is refused (fail closed)";

pub struct NoTrustAnchor;

impl TrustAnchor for NoTrustAnchor {
    fn id(&self) -> &'static str {
        "none-provisioned"
    }

    fn verify(&self, _canonical: &[u8], _signature: &[u8]) -> bool {
        false
    }
}

/// The **test-only** anchor: a keyed SHA-256 digest (key || canonical ||
/// key) over the manifest's canonical bytes. It exists so the verification
/// path — key mismatch, tampered bytes, matching signature — is exercisable
/// in fixtures. It is NOT a signature scheme, NOT registered as production
/// trust, and never ships behind a public constructor; the loader takes any
/// `TrustAnchor` by injection, and production injects [`NoTrustAnchor`].
pub struct KeyedDigestTestAnchor {
    key: Vec<u8>,
}

impl KeyedDigestTestAnchor {
    /// Construct the test anchor with a synthetic key (fixtures only).
    pub fn new(key: &[u8]) -> Self {
        KeyedDigestTestAnchor {
            key: key.to_vec(),
        }
    }

    /// The test anchor's own digest of canonical bytes.
    pub fn digest(&self, canonical: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.key);
        hasher.update(canonical);
        hasher.update(&self.key);
        hasher.finalize().to_vec()
    }
}

impl TrustAnchor for KeyedDigestTestAnchor {
    fn id(&self) -> &'static str {
        "keyed-digest-test-only"
    }

    fn verify(&self, canonical: &[u8], signature: &[u8]) -> bool {
        constant_time_eq(&self.digest(canonical), signature)
    }
}

/// Constant-time comparison for digest answers — the verification path
/// must not leak a prefix match through timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The production anchor refuses everything, by construction and by
    /// name (`docs/14` §9).
    #[test]
    fn the_production_anchor_refuses_everything() {
        let anchor = NoTrustAnchor;
        assert!(!anchor.verify(b"anything", b"anysignature"));
        assert!(!anchor.verify(b"", b""));
        assert_eq!(anchor.id(), "none-provisioned");
        assert!(!PRODUCTION_ANCHOR_REFUSES_ALL.is_empty());
    }

    /// The test anchor verifies a matching keyed digest and refuses a
    /// mismatched one — the verification path is real, the anchor is not
    /// production trust.
    #[test]
    fn the_test_anchor_verifies_only_its_own_keyed_digest() {
        let anchor = KeyedDigestTestAnchor::new(b"test-key");
        let canonical = b"{\"name\":\"x\"}";
        let signature = anchor.digest(canonical);
        assert!(anchor.verify(canonical, &signature));
        assert!(!anchor.verify(canonical, &signature[..signature.len() - 1]));
        assert!(!anchor.verify(b"tampered", &signature));
        let other = KeyedDigestTestAnchor::new(b"other-key");
        assert!(!other.verify(canonical, &signature));
    }
}
