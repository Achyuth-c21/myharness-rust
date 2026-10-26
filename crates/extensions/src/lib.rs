//! The extension-package machinery (`docs/13` §"Phase 8"; `TEST-EXT-001`,
//! `TEST-EXT-002`, `TEST-EXT-004`, `TEST-EXT-006`): what it takes for an
//! extension package to be *registrable*, and how a broken one stays
//! isolated from app boot.
//!
//! A package is a directory under the extensions root:
//!
//! ```text
//! <root>/<name>/manifest.json     the signed manifest (this crate's schema)
//! <root>/<name>/<payload-path>    the payload the manifest's digest covers
//! ```
//!
//! The manifest carries the package's identity (name, version,
//! min-app-version), its kind (`provider` packages declare the egress hosts
//! they will contact), a SHA-256 digest of the payload, and a signature.
//! [`TrustAnchor`] is the seam that decides whether a signature is trusted;
//! the production anchor is [`NoTrustAnchor`] and **refuses every
//! package**: no production signing credential or captured trust anchor
//! exists in this build, and a verifier that accepted an unknown key would
//! be a policy bypass — the same fail-closed posture as the catalog
//! signature (`docs/14` §9 item 20) and the update feed (item 70).
//! Provisioning a real anchor moves the refusal aside; it does not weaken
//! the gate. **Nothing is ever accepted because a signature field exists.**
//!
//! The loader (`loader`) reads every package directory independently: one
//! broken package is a recorded, per-package error and never prevents the
//! others from loading — boot continues (`TEST-EXT-002`). Packages load
//! **lazily**: the manifest verifies at scan time, the payload verifies
//! only when first read, so a corrupted payload cannot break the scan.
//!
//! Registration without provider hard-coding (`TEST-EXT-007`) lives at the
//! bridge: the family *catalog* (ids, labels, form keys, offered credential
//! kinds, capability flags) is data the setup surface reads; the genuinely
//! wire-specific parts — the adapter constructors — remain registered
//! per-family in the bridge. The boundary this crate pins: **the manifest
//! and the catalog describe; the bridge constructs.**

pub mod loader;
pub mod manifest;
pub mod signature;

pub use loader::{load_extensions, ExtensionState, ExtensionsRoot};
pub use manifest::{Manifest, ManifestError, PackageKind, MAX_MANIFEST_BYTES};
pub use signature::{NoTrustAnchor, TrustAnchor, PRODUCTION_ANCHOR_REFUSES_ALL};
