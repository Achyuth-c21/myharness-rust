//! OS-keychain credential store and profile secret references — roadmap Phase 2
//! (docs/13 §4 Phase 2; STACK.md storage row: SQLite + keyring for secrets).
//!
//! Requirements bound to this module:
//! - [`DATA-CONFIG-02` in `docs/09` §5]: "Store credentials in the OS keychain; never
//!   expose them to the renderer."
//! - `docs/11` §3: "Profiles can contain secrets. Restrict file permissions and prefer
//!   a keychain reference." Inline plaintext inside profiles is the *observed* shipped
//!   behavior (the complete gateway profile carried the API key in the document), so it
//!   must keep resolving for legacy/foreign profiles — while new writes go through
//!   [`SecretRef`] handles (`docs/19` IMP register: no `AI-ADAPTER-12` repeat).
//! - **[HARD] `RULES.md` §4**: the renderer never links this crate; secrets never cross
//!   the `docs/10` §8 trust boundary as values — renderer-visible artifacts carry only
//!   [`SecretRef`] handles and storage status.
//! - `docs/09` §4: never log keys/tokens; every [`SecretError`] below reports account
//!   names only, never values.
//! - `STACK.md`: values in memory are `secrecy::SecretString` (zeroize-on-drop).
//!
//! Negative specimen honored: `AI-ADAPTER-12` (`docs/12` §7) records that the patched
//! adapter kept custom provider JSON "not keychain-backed". This crate is the
//! keychain-backed replacement: references outlive profile rewrites, rotation is an
//! overwrite on the same account, and no secret value ever round-trips through a
//! profile file in the preferred path.
//!
//! [`helper`] owns the **helper-script** credential kind: it runs the configured
//! program under a timeout, treats its output as data (a credential, never an
//! instruction), and fails closed — a helper failure never falls back to another
//! credential source (`TEST-CFG-005`).
//!
//! Deliberately NOT in this slice: production availability detection + KeyringStore vs
//! file fallback wiring (bridge/Phase-2 integration), renderer-side redaction of whole
//! profile documents (bridge slice — it needs `crates/config` types), and export-time
//! redaction (`docs/09` §4 export warning). [`MemoryStore`] is a **test/headless**
//! backend only; the production fallback when no OS keychain exists is mode-`0600`
//! inline profile fields (observed shipped behavior, `docs/09` §4) — never a
//! process-local map.

use std::collections::BTreeMap;
use std::sync::Mutex;

pub mod helper;

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};

/// Default keychain service name for the rebuild. Distinct from the shipped app's
/// service so the two never share entries ("myharness", matching the `myharness://`
/// namespace the shell already owns).
pub const DEFAULT_SERVICE: &str = "myharness";

/// Handle to a keychain entry, safe to embed in a profile document and to send to the
/// renderer (docs/18 §10: "Secrets are opaque handles, not values, in renderer
/// messages"). Resolution happens only in main-process code that links this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRef {
    pub kind: RefKind,
    pub service: String,
    pub account: String,
}

/// Reference kinds. Single variant today (keychain); future kinds (e.g. a
/// consent-gated file vault) must fail closed in [`SecretSlot::resolve`], never
/// silently degrade to another store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefKind {
    Keychain,
}

/// Credential-kind suffix vocabulary (`docs/09` §2.2): static key, interactive
/// sign-in, or helper script. Accounts derive from these names so each kind owns a
/// distinct keychain account under one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    Static,
    Interactive,
    Helper,
}

/// Deterministic keychain account for a profile field
/// (`profile/<profile-id>/<field>`). Stable across rewrites so Apply/rotation
/// overwrite the same entry instead of leaking duplicates, and deterministic across
/// machines so profiles stay portable. `field` is caller-supplied dotted notation
/// (e.g. `connection.inferenceGatewayApiKey`), passed BY the caller because the
/// profile schema lives in `crates/config` — this crate stays dependency-light.
pub fn derive_account(profile_id: &str, field: &str) -> Result<String, SecretError> {
    let (id, key) = (profile_id.trim(), field.trim());
    let invalid = |detail: &str| SecretError::Format {
        detail: detail.to_string(),
    };
    if id.is_empty() || key.is_empty() {
        return Err(invalid("derive_account: profile id and field must be non-empty"));
    }
    if id.contains('/') {
        return Err(invalid("derive_account: profile id must not contain '/'"));
    }
    Ok(format!("profile/{id}/{key}"))
}

/// The shape a credential takes inside a profile document: legacy inline plaintext
/// (observed shipped profiles) or the preferred keychain reference (`docs/11` §3).
/// Untagged: a raw string is inline, an object is a handle — foreign/forward profiles
/// keep working either way.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum SecretSlot {
    /// Inline plaintext, e.g. `"sk-ant-api03-…"`. Kept ONLY for legacy/foreign
    /// compatibility; new writes by the rebuild emit [`SecretSlot::Reference`].
    Inline(String),
    /// Keychain handle — the preferred, renderer-safe shape (`DATA-CONFIG-02`).
    Reference(SecretRef),
}

impl SecretSlot {
    /// Debug never carries the value (`docs/09` §4: never log API keys/tokens). A
    /// derived impl would print inline plaintext into any `{:?}` caller.
    fn fmt_redacted(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretSlot::Inline(_) => write!(f, "SecretSlot::Inline(<redacted>)"),
            SecretSlot::Reference(r) => write!(f, "SecretSlot::Reference({r:?})"),
        }
    }

    /// Resolve this slot into memory-only value material (`SecretString`,
    /// zeroized on drop). Fails closed: a missing keychain entry or a service
    /// mismatch is an error, never an empty string (`docs/01` fail-closed rule).
    pub fn resolve(&self, store: &dyn SecretStore) -> Result<SecretString, SecretError> {
        match self {
            SecretSlot::Inline(value) => Ok(SecretString::from(value.clone())),
            SecretSlot::Reference(r) => {
                if r.kind != RefKind::Keychain {
                    return Err(SecretError::Format {
                        detail: format!("unsupported secret ref kind {:?}", safe_kind(r.kind)),
                    });
                }
                if r.service != store.service() {
                    return Err(SecretError::ServiceMismatch {
                        expected: store.service().to_string(),
                        found: r.service.clone(),
                    });
                }
                store
                    .load(&r.account)?
                    .ok_or_else(|| SecretError::Missing {
                        account: r.account.clone(),
                    })
            }
        }
    }
}

impl std::fmt::Debug for SecretSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_redacted(f)
    }
}

fn safe_kind(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Keychain => "keychain",
    }
}

/// Backend contract. One method per keychain primitive; no batching/reading of
/// foreign accounts (principle of least surface at the trust boundary, `docs/10` §8).
pub trait SecretStore {
    /// Service namespace this store resolves against (refs must match it).
    fn service(&self) -> &str;
    /// Create or overwrite the entry. Overwrite = rotation (`AI-ADAPTER-12` note).
    fn store(&self, account: &str, value: &SecretString) -> Result<(), SecretError>;
    /// Ok(None) means the account does not exist — materially different from a
    /// backend failure (missing ≠ broken; `docs/14` §9 discipline).
    fn load(&self, account: &str) -> Result<Option<SecretString>, SecretError>;
    /// Deletes the entry. `Ok(true)` existed+deleted, `Ok(false)` was already absent.
    fn delete(&self, account: &str) -> Result<bool, SecretError>;
}

/// Real OS keychain via `keyring` (Secret Service on Linux desktops, Apple native,
/// Windows native — per the crate features in `Cargo.toml`).
///
/// Thin glue only; backend/availability wiring happens at bridge integration (this
/// slice stays compile-verified without claiming runtime behavior on any platform —
/// see `docs/17` §4.8 note on owned rows).
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        KeyringStore {
            service: service.into(),
        }
    }
}

impl SecretStore for KeyringStore {
    fn service(&self) -> &str {
        &self.service
    }

    fn store(&self, account: &str, value: &SecretString) -> Result<(), SecretError> {
        let entry = entry(&self.service, account)?;
        entry.set_password(value.expose_secret()).map_err(|e| backend(account, e))
    }

    fn load(&self, account: &str) -> Result<Option<SecretString>, SecretError> {
        let entry = entry(&self.service, account)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(backend(account, e)),
        }
    }

    fn delete(&self, account: &str) -> Result<bool, SecretError> {
        let entry = entry(&self.service, account)?;
        match entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(backend(account, e)),
        }
    }
}

fn entry(service: &str, account: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(service, account)
        .map_err(|e| backend(account, e))
}

fn backend(account: &str, e: keyring::Error) -> SecretError {
    SecretError::Store {
        account: account.to_string(),
        message: e.to_string(),
    }
}

/// Process-local map — TESTS AND HEADLESS TOOLING ONLY. Never a production fallback:
/// the production fallback for a machine without a keychain is the observed shipped
/// pattern (inline profile fields under mode `0600`, `docs/09` §4 / `docs/11` §3).
#[derive(Default)]
pub struct MemoryStore {
    entries: Mutex<BTreeMap<String, SecretString>>,
}

impl SecretStore for MemoryStore {
    fn service(&self) -> &str {
        "memory"
    }

    fn store(&self, account: &str, value: &SecretString) -> Result<(), SecretError> {
        self.entries
            .lock()
            .expect("memory store poisoned")
            .insert(account.to_string(), SecretString::from(value.expose_secret().to_string()));
        Ok(())
    }

    fn load(&self, account: &str) -> Result<Option<SecretString>, SecretError> {
        Ok(self
            .entries
            .lock()
            .expect("memory store poisoned")
            .get(account)
            .map(|v| SecretString::from(v.expose_secret().to_string())))
    }

    fn delete(&self, account: &str) -> Result<bool, SecretError> {
        Ok(self
            .entries
            .lock()
            .expect("memory store poisoned")
            .remove(account)
            .is_some())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// Backend failure. Carries account + backend message only — the VALUE can
    /// never be in scope for a backend error (`docs/09` §4 never-log rule).
    #[error("SECRET_STORE: backend rejected an operation for account {account:?}: {message}")]
    Store { account: String, message: String },
    /// The reference points at a different service than the store resolves
    /// (`SecretSlot::resolve` refuses cross-service reads).
    #[error("SECRET_MISMATCH: reference targets service {found:?} but the store is {expected:?}")]
    ServiceMismatch { expected: String, found: String },
    /// The keychain has no entry for the account (fail closed during resolution).
    #[error("SECRET_MISSING: no secret stored for account {account:?}")]
    Missing { account: String },
    /// Malformed slot/identifier (not plaintext string, not a handle, invalid
    /// derivation inputs).
    #[error("SECRET_FORMAT: {detail}")]
    Format { detail: String },
    /// The credential helper failed. Carries the program and a status — never the
    /// helper's output, because a helper that failed to deliver a credential can
    /// print one to stderr (`docs/09` §4 never-log rule). There is no fallback to
    /// another credential source (`TEST-CFG-005`).
    #[error("SECRET_HELPER: helper {program:?} failed: {detail}")]
    HelperFailed { program: String, detail: String },
    /// The credential helper exceeded its timeout and was killed.
    #[error("SECRET_HELPER_TIMEOUT: helper {program:?} exceeded its {timeout_secs}s timeout")]
    HelperTimeout { program: String, timeout_secs: u64 },
    /// The cached helper credential is past its TTL and silent refresh is off, so
    /// it must not be used. Fail closed: a stale credential is never returned
    /// (`docs/14` §9 item 15).
    #[error("SECRET_HELPER_EXPIRED: the cached credential for account {account:?} is past its TTL and cannot be refreshed silently")]
    HelperExpired { account: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_value() -> SecretString {
        SecretString::from("sk-ant-api03-EXAMPLE-VALUE".to_string())
    }

    /// Store→load→overwrite cycle: rotation replaces the value on the same account.
    #[test]
    fn memory_store_round_trip_and_overwrite() {
        let store = MemoryStore::default();
        store.store("profile/abc/connection.inferenceGatewayApiKey", &sample_value()).unwrap();
        let first = store
            .load("profile/abc/connection.inferenceGatewayApiKey")
            .unwrap()
            .expect("stored secret must resolve");
        assert_eq!(first.expose_secret(), "sk-ant-api03-EXAMPLE-VALUE");

        // Rotation overwrites deterministically (no duplicate accounts).
        store
            .store(
                "profile/abc/connection.inferenceGatewayApiKey",
                &SecretString::from("rotated-value".to_string()),
            )
            .unwrap();
        let second = store
            .load("profile/abc/connection.inferenceGatewayApiKey")
            .unwrap()
            .unwrap();
        assert_eq!(second.expose_secret(), "rotated-value");
    }

    /// Missing account is `Ok(None)` — materially different from a backend failure.
    #[test]
    fn missing_account_is_none_not_error() {
        let store = MemoryStore::default();
        assert!(store.load("profile/none/field").unwrap().is_none());
        assert!(!store.delete("profile/none/field").unwrap(), "delete of nothing reports false");
    }

    /// Delete semantics: true when it existed, false afterwards, gone for reads.
    #[test]
    fn delete_reports_existence() {
        let store = MemoryStore::default();
        let account = derive_account("abc", "connection.inferenceApiKey").unwrap();
        store.store(&account, &sample_value()).unwrap();
        assert!(store.delete(&account).unwrap());
        assert!(!store.delete(&account).unwrap());
        assert!(store.load(&account).unwrap().is_none());
    }

    /// Slots resolve inline (legacy observed profiles) and via reference; a wrong
    /// service fails closed instead of cross-reading another store.
    #[test]
    fn slots_resolve_and_refuse_wrong_service() {
        let store = MemoryStore::default();
        let account = derive_account("abc", "connection.inferenceGatewayApiKey").unwrap();
        store.store(&account, &sample_value()).unwrap();

        let legacy = SecretSlot::Inline("legacy-inline-key".to_string());
        assert_eq!(legacy.resolve(&store).unwrap().expose_secret(), "legacy-inline-key");

        let r = SecretSlot::Reference(SecretRef {
            kind: RefKind::Keychain,
            service: "memory".to_string(),
            account: account.clone(),
        });
        assert_eq!(r.resolve(&store).unwrap().expose_secret(), "sk-ant-api03-EXAMPLE-VALUE");

        let other = SecretSlot::Reference(SecretRef {
            kind: RefKind::Keychain,
            service: "other-service".to_string(),
            account,
        });
        let err = other.resolve(&store).unwrap_err();
        assert!(format!("{err}").contains("SECRET_MISMATCH"));
    }

    /// A reference to an absent keychain entry fails CLOSED (missing ≠ empty string).
    #[test]
    fn missing_reference_is_an_error_not_an_empty_value() {
        let store = MemoryStore::default();
        let r = SecretSlot::Reference(SecretRef {
            kind: RefKind::Keychain,
            service: "memory".to_string(),
            account: "profile/ghost/connection.inferenceGatewayApiKey".to_string(),
        });
        let err = r.resolve(&store).unwrap_err();
        assert!(format!("{err}").contains("SECRET_MISSING"));
    }

    /// Error surfaces and Debug never carry the value (`docs/09` §4 never-log rule;
    /// stack discipline: secrecy zeroizes + redacts on drop/debug).
    #[test]
    fn error_and_debug_surfaces_never_leak_values() {
        let store = MemoryStore::default();
        let r = SecretSlot::Reference(SecretRef {
            kind: RefKind::Keychain,
            service: "memory".to_string(),
            account: "profile/x/f".to_string(),
        });
        let err = r.resolve(&store).unwrap_err();
        assert!(!format!("{err:?}").contains("sk-ant"));
        assert!(!format!("{err}").contains("sk-ant"));

        let slot = SecretSlot::Inline("sk-ant-api03-INLINE".to_string());
        let debugged = format!("{slot:?}");
        assert!(!debugged.contains("sk-ant-api03-INLINE"), "slot debug must redact: {debugged}");
        let resolved = slot.resolve(&store).unwrap();
        let resolved_debug = format!("{resolved:?}");
        assert!(!resolved_debug.contains("sk-ant"), "resolved debug must redact: {resolved_debug}");
    }

    /// Account derivation is deterministic and rejects malformed inputs.
    #[test]
    fn derive_account_is_deterministic_and_validated() {
        assert_eq!(
            derive_account("p1", "connection.inferenceGatewayApiKey").unwrap(),
            "profile/p1/connection.inferenceGatewayApiKey"
        );
        assert_eq!(derive_account("p1", "connection.inferenceGatewayApiKey").unwrap(), derive_account("p1", "connection.inferenceGatewayApiKey").unwrap());
        assert!(derive_account("", "f").is_err());
        assert!(derive_account("p1", " ").is_err());
        assert!(derive_account("a/b", "f").is_err(), "ids must not embed path separators");
    }

    /// `SecretRef` wire format is camelCase with an explicit kind
    /// (`docs/11` §3 "prefer a keychain reference"; `docs/20` camelCase vocabulary).
    #[test]
    fn reference_wire_format_is_camelcase() {
        let r = SecretRef {
            kind: RefKind::Keychain,
            service: "myharness".to_string(),
            account: "profile/p1/f".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""kind":"keychain""#), "explicit kind on the wire: {json}");
        assert!(json.contains(r#""service":"myharness""#), "service travels with the ref: {json}");
        assert!(json.contains(r#""account":"profile/p1/f""#), "account travels with the ref: {json}");
        // Untagged slot: raw string = inline, object = handle
        assert!(matches!(
            serde_json::from_str::<SecretSlot>("\"legacy-key\"").unwrap(),
            SecretSlot::Inline(_)
        ));
        assert!(matches!(
            serde_json::from_str::<SecretSlot>(
                r#"{"kind":"keychain","service":"myharness","account":"profile/p1/f"}"#
            )
            .unwrap(),
            SecretSlot::Reference(_)
        ));
    }
}
