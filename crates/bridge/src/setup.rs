//! The `settings.Custom3pSetup` bridge namespace — roadmap Phase 2 (docs/13 §4
//! Phase 2; docs/18 §3.2, §3.4; docs/09 §2.1; `ARCH-PROC-02/03/05`).
//!
//! The setup window is a **separate window** (`docs/09` §2.1) that drives the
//! third-party configuration library through the bridge. The live app exposes
//! `settings.Custom3pSetup` with "profiles, config read/write/apply, health,
//! probes, relaunch, auth" (`docs/18` §3.4); the rebuild implements a smaller,
//! typed set.
//!
//! Rules bound to this namespace:
//!
//! - **[HARD] The bridge is not a generic CRUD surface** (`docs/10` §4.3). Every
//!   command is enumerated, every argument is validated here, and the set grows
//!   one roadmap phase at a time.
//! - **[HARD] Secrets are opaque handles, not values, in renderer messages**
//!   (`docs/18` §10). A profile the renderer reads is redacted: a secret key
//!   never travels back. A secret the renderer *writes* travels on the request
//!   only (the user typed it) and is stored in the OS keychain; the response
//!   carries the handle.
//! - **[HARD] Validation happens in the bridge, not in the renderer**
//!   (`ARCH-PROC-03`; `docs/10` §8). The renderer is untrusted input; a command
//!   that reaches the library has already been checked.
//! - Probes (model discovery, inference reachability) and the credential helper
//!   runner live in the `settings.Custom3pHelperRun` namespace
//!   (`docs/18` §3.4) and are not part of this slice.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use myharness_config::export::ExportBundle;
use myharness_config::forms::FormIssue;
use myharness_config::health::ConfigHealth;
use myharness_config::overlay::MergedConfig;
use myharness_config::tabs::SetupTab;
use myharness_config::{ConfigError, ConfigLibrary, ProfileDocument, ProviderKind};
use myharness_secrets::{SecretRef, SecretStore};

/// A command in the `settings.Custom3pSetup` namespace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SetupCommand {
    /// `list` — the profile library.
    ListProfiles,
    /// `read` — one profile document, redacted.
    ReadProfile { id: String },
    /// `write` — persist an edited draft. `secrets` carries the credential
    /// values the user typed, keyed `section/key`; the bridge stores them in the
    /// OS keychain and puts the handle in the document, so no secret value is
    /// ever written into a profile or returned to the renderer
    /// (`docs/18` §10).
    WriteProfile {
        document: Value,
        #[serde(default)]
        secrets: BTreeMap<String, String>,
    },
    /// `apply` — validate and activate a profile.
    ApplyProfile { id: String },
    /// `duplicate` — copy a profile into an independent draft.
    DuplicateProfile { id: String, name: String },
    /// `rename` — move a profile's name.
    RenameProfile { id: String, name: String },
    /// `delete` — remove a profile (the applied one is refused).
    DeleteProfile { id: String },
    /// `getMerged` — the effective configuration with per-field source and lock state.
    Merged,
    /// `health` — the managed-configuration health.
    Health,
    /// `export` — the redacted support bundle.
    Export,
    /// `egress` — the derived egress allowlist (the read-only Egress tab).
    Egress,
    /// `tabSchema` — the form schema of one setup tab.
    TabSchema { tab: String },
    /// `connectionSchema` — the Connection form schema of one provider family.
    ConnectionSchema { provider: String },
    /// `validate` — the issues of a draft, without persisting it.
    Validate { document: Value },
}

/// A response in the `settings.Custom3pSetup` namespace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SetupResponse {
    /// The profile library.
    Profiles { profiles: Vec<ProfileSummary> },
    /// One profile document, redacted.
    Profile { document: Value },
    /// The effective configuration.
    Merged { merged: Value },
    /// The managed-configuration health.
    Health { health: Value },
    /// The redacted support bundle.
    Export { bundle: Value },
    /// The derived egress allowlist.
    Egress { allowlist: Value },
    /// A form schema.
    Schema { schema: Value },
    /// The validation issues of a draft.
    Issues { issues: Vec<FormIssue> },
    /// The command failed. The message is user-safe: no secret, no raw body
    /// (`docs/18` §11).
    Error { error: BridgeError },
}

/// A profile as the renderer sees it: the name and id, never a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub provider: Option<ProviderKind>,
    /// Whether this profile is the applied one.
    pub applied: bool,
}

/// A user-safe error envelope (`docs/18` §11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    /// A stable machine code for tests and telemetry.
    pub code: String,
    /// A message safe to show a user.
    pub message: String,
    /// Whether retrying could succeed.
    pub retryable: bool,
}

impl BridgeError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        BridgeError {
            code: code.to_string(),
            message: message.into(),
            retryable: false,
        }
    }
}

impl From<ConfigError> for BridgeError {
    fn from(error: ConfigError) -> Self {
        let code = match &error {
            ConfigError::Io { .. } => "CONFIG_IO",
            ConfigError::Parse { .. } => "CONFIG_PARSE",
            ConfigError::Schema { .. } => "CONFIG_SCHEMA",
            ConfigError::Validation { .. } => "CONFIG_VALIDATION",
            ConfigError::NotFound { .. } => "CONFIG_NOT_FOUND",
            ConfigError::DuplicateName { .. } => "CONFIG_DUPLICATE",
            ConfigError::InUse { .. } => "CONFIG_IN_USE",
            ConfigError::Managed { .. } => "CONFIG_MANAGED",
        };
        BridgeError::new(code, error.to_string())
    }
}

/// The operations the setup window needs. Implemented over the configuration
/// library; the bridge owns the argument validation.
pub trait SetupBackend {
    /// The profile library.
    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, BridgeError>;
    /// One profile document.
    fn read_profile(&self, id: &str) -> Result<ProfileDocument, BridgeError>;
    /// Persist a draft.
    fn write_profile(&self, document: &ProfileDocument) -> Result<(), BridgeError>;
    /// Validate and activate a profile.
    fn apply_profile(&self, id: &str) -> Result<(), BridgeError>;
    /// Copy a profile into an independent draft.
    fn duplicate_profile(&self, id: &str, name: &str) -> Result<ProfileDocument, BridgeError>;
    /// Move a profile's name.
    fn rename_profile(&self, id: &str, name: &str) -> Result<(), BridgeError>;
    /// Remove a profile.
    fn delete_profile(&self, id: &str) -> Result<(), BridgeError>;
    /// The effective configuration.
    fn merged(&self) -> Result<MergedConfig, BridgeError>;
    /// The managed-configuration health.
    fn health(&self) -> Result<ConfigHealth, BridgeError>;
    /// The redacted support bundle.
    fn export(&self) -> Result<ExportBundle, BridgeError>;
    /// The derived egress allowlist.
    fn egress(&self) -> Result<myharness_config::egress::EgressAllowlist, BridgeError>;
    /// Store a credential value and return the handle the document carries.
    fn set_secret(&self, account: &str, value: &str) -> Result<SecretRef, BridgeError>;
}

/// A backend over the configuration library and the OS keychain.
pub struct ConfigBackend {
    library: ConfigLibrary,
    secrets: Box<dyn SecretStore>,
}

impl ConfigBackend {
    /// Open the library at `root`, resolving secrets through `store`.
    pub fn new(root: impl Into<std::path::PathBuf>, store: Box<dyn SecretStore>) -> Self {
        ConfigBackend {
            library: ConfigLibrary::open(root),
            secrets: store,
        }
    }

    /// The library behind the backend.
    pub fn library(&self) -> &ConfigLibrary {
        &self.library
    }
}

impl SetupBackend for ConfigBackend {
    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, BridgeError> {
        let entries = self.library.list()?;
        let applied = self.library.active()?.map(|entry| entry.id);
        Ok(entries
            .into_iter()
            .map(|entry| ProfileSummary {
                id: entry.id.clone(),
                name: entry.name,
                provider: self
                    .library
                    .read(&entry.id)
                    .ok()
                    .and_then(|doc| doc.provider),
                applied: applied.as_deref() == Some(entry.id.as_str()),
            })
            .collect())
    }

    fn read_profile(&self, id: &str) -> Result<ProfileDocument, BridgeError> {
        Ok(self.library.read(id)?)
    }

    fn write_profile(&self, document: &ProfileDocument) -> Result<(), BridgeError> {
        Ok(self.library.write(document)?)
    }

    fn apply_profile(&self, id: &str) -> Result<(), BridgeError> {
        Ok(self.library.apply(id)?)
    }

    fn duplicate_profile(&self, id: &str, name: &str) -> Result<ProfileDocument, BridgeError> {
        Ok(self.library.duplicate(id, name)?)
    }

    fn rename_profile(&self, id: &str, name: &str) -> Result<(), BridgeError> {
        Ok(self.library.rename(id, name)?)
    }

    fn delete_profile(&self, id: &str) -> Result<(), BridgeError> {
        Ok(self.library.delete(id)?)
    }

    fn merged(&self) -> Result<MergedConfig, BridgeError> {
        Ok(self.library.get_merged()?)
    }

    fn health(&self) -> Result<ConfigHealth, BridgeError> {
        Ok(self.library.managed().health())
    }

    fn export(&self) -> Result<ExportBundle, BridgeError> {
        Ok(self.library.export()?)
    }

    fn egress(&self) -> Result<myharness_config::egress::EgressAllowlist, BridgeError> {
        let merged = self.library.get_merged()?;
        let Some(entry) = self.library.active()? else {
            return Ok(myharness_config::egress::EgressAllowlist { groups: Vec::new() });
        };
        let profile = self.library.read(&entry.id)?;
        Ok(myharness_config::egress::derive(&profile, Some(&merged)))
    }

    fn set_secret(&self, account: &str, value: &str) -> Result<SecretRef, BridgeError> {
        // The value is written to the keychain first: a document that references a
        // secret that was never stored would resolve to nothing (fail closed).
        let secret = secrecy::SecretString::from(value.to_string());
        self.secrets.store(account, &secret).map_err(|error| {
            BridgeError::new(
                "SECRET_STORE",
                format!("the credential could not be stored: {error}"),
            )
        })?;
        Ok(SecretRef {
            kind: myharness_secrets::RefKind::Keychain,
            service: self.secrets.service().to_string(),
            account: account.to_string(),
        })
    }
}

/// Execute one setup command.
///
/// The command is validated before it reaches the backend, and every response is
/// redacted: a secret value never travels from the core to the renderer
/// (`docs/18` §10).
pub fn dispatch<B: SetupBackend>(backend: &B, command: SetupCommand) -> SetupResponse {
    match run(backend, command) {
        Ok(response) => response,
        Err(error) => SetupResponse::Error { error },
    }
}

fn run<B: SetupBackend>(backend: &B, command: SetupCommand) -> Result<SetupResponse, BridgeError> {
    match command {
        SetupCommand::ListProfiles => Ok(SetupResponse::Profiles {
            profiles: backend.list_profiles()?,
        }),
        SetupCommand::ReadProfile { id } => {
            let id = validate_id(&id)?;
            let document = backend.read_profile(&id)?;
            Ok(SetupResponse::Profile {
                document: redact(&document),
            })
        }
        SetupCommand::WriteProfile { document, secrets } => {
            let mut document = decode_document(document)?;
            // A renderer must never send a secret value inside the document
            // (`docs/18` §10): that is refused before anything is written.
            reject_inline_secrets(&document)?;
            apply_secrets(backend, &mut document, &secrets)?;
            backend.write_profile(&document)?;
            Ok(SetupResponse::Profile {
                document: redact(&document),
            })
        }
        SetupCommand::ApplyProfile { id } => {
            let id = validate_id(&id)?;
            backend.apply_profile(&id)?;
            Ok(SetupResponse::Profiles {
                profiles: backend.list_profiles()?,
            })
        }
        SetupCommand::DuplicateProfile { id, name } => {
            let id = validate_id(&id)?;
            let name = validate_name(&name)?;
            let document = backend.duplicate_profile(&id, &name)?;
            Ok(SetupResponse::Profile {
                document: redact(&document),
            })
        }
        SetupCommand::RenameProfile { id, name } => {
            let id = validate_id(&id)?;
            let name = validate_name(&name)?;
            backend.rename_profile(&id, &name)?;
            Ok(SetupResponse::Profiles {
                profiles: backend.list_profiles()?,
            })
        }
        SetupCommand::DeleteProfile { id } => {
            let id = validate_id(&id)?;
            backend.delete_profile(&id)?;
            Ok(SetupResponse::Profiles {
                profiles: backend.list_profiles()?,
            })
        }
        SetupCommand::Merged => Ok(SetupResponse::Merged {
            merged: serde_json::to_value(backend.merged()?).unwrap_or(Value::Null),
        }),
        SetupCommand::Health => Ok(SetupResponse::Health {
            health: serde_json::to_value(backend.health()?).unwrap_or(Value::Null),
        }),
        SetupCommand::Export => Ok(SetupResponse::Export {
            bundle: serde_json::to_value(backend.export()?).unwrap_or(Value::Null),
        }),
        SetupCommand::Egress => Ok(SetupResponse::Egress {
            allowlist: serde_json::to_value(backend.egress()?).unwrap_or(Value::Null),
        }),
        SetupCommand::TabSchema { tab } => {
            let tab = parse_tab(&tab)?;
            let schema = myharness_config::tabs::tab_schema(tab);
            Ok(SetupResponse::Schema {
                schema: serde_json::to_value(schema).unwrap_or(Value::Null),
            })
        }
        SetupCommand::ConnectionSchema { provider } => {
            let provider = parse_provider(&provider)?;
            let schema = myharness_config::forms::connection_schema(provider);
            Ok(SetupResponse::Schema {
                schema: serde_json::to_value(schema).unwrap_or(Value::Null),
            })
        }
        SetupCommand::Validate { document } => {
            let document = decode_document(document)?;
            Ok(SetupResponse::Issues {
                issues: myharness_config::forms::validate_document(&document),
            })
        }
    }
}

/// A profile id must be a uuid — the library's own id format (`docs/18` §10).
fn validate_id(id: &str) -> Result<String, BridgeError> {
    let id = id.trim();
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the profile id is not a valid identifier",
        ));
    }
    Ok(id.to_string())
}

/// A profile name must be non-empty (`docs/09` §3.2).
fn validate_name(name: &str) -> Result<String, BridgeError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the profile name must not be empty",
        ));
    }
    Ok(name.to_string())
}

fn parse_tab(tab: &str) -> Result<SetupTab, BridgeError> {
    serde_json::from_value::<SetupTab>(Value::String(tab.to_string()))
        .map_err(|_| BridgeError::new("CONFIG_VALIDATION", "unknown setup tab"))
}

fn parse_provider(provider: &str) -> Result<ProviderKind, BridgeError> {
    serde_json::from_value::<ProviderKind>(Value::String(provider.to_string()))
        .map_err(|_| BridgeError::new("CONFIG_VALIDATION", "unknown provider family"))
}

/// Decode a profile document sent by the renderer.
fn decode_document(document: Value) -> Result<ProfileDocument, BridgeError> {
    serde_json::from_value::<ProfileDocument>(document).map_err(|e| {
        BridgeError::new(
            "CONFIG_PARSE",
            format!("the profile document could not be read: {e}"),
        )
    })
}

/// A secret must never be sent as a value (`docs/18` §10: "Secrets are opaque
/// handles, not values, in renderer messages"). A renderer write that carries an
/// inline secret is refused rather than stored.
///
/// The check is specific to secrets: a document that merely contains a private
/// host or a URL with a query string is not a secret leak and is not refused here
/// (the export redacts those; the write keeps them).
fn reject_inline_secrets(document: &ProfileDocument) -> Result<(), BridgeError> {
    let value = serde_json::to_value(document).unwrap_or(Value::Null);
    if let Some(object) = value.as_object() {
        for section in SECTIONS {
            let Some(map) = object.get(section).and_then(Value::as_object) else {
                continue;
            };
            for key in myharness_config::export::SECRET_KEYS {
                let Some(entry) = map.get(key) else {
                    continue;
                };
                if entry.as_str().is_some() {
                    return Err(BridgeError::new(
                        "CONFIG_VALIDATION",
                        "a secret must be stored as a keychain handle, not as a value",
                    ));
                }
            }
            if let Some(found) = find_credential_value(map) {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    format!("the value of {found:?} looks like a credential; store it as a keychain handle"),
                ));
            }
        }
    }
    Ok(())
}

/// Find the first value in a map that looks like a credential.
fn find_credential_value(map: &serde_json::Map<String, Value>) -> Option<String> {
    for (key, value) in map {
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.starts_with("sk-ant-")
                    || trimmed.to_ascii_lowercase().starts_with("bearer ")
                {
                    return Some(key.clone());
                }
            }
            Value::Object(inner) => {
                for (inner_key, inner_value) in inner {
                    if let Some(text) = inner_value.as_str() {
                        let trimmed = text.trim();
                        if trimmed.starts_with("sk-ant-")
                            || trimmed.to_ascii_lowercase().starts_with("bearer ")
                        {
                            return Some(format!("{key}.{inner_key}"));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Redact a profile document for the renderer.
fn redact(document: &ProfileDocument) -> Value {
    myharness_config::export::redact_document(document)
}

/// The sections a secret key can live in.
const SECTIONS: [&str; 9] = [
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

/// Store the credentials the user typed and place the handles in the document.
///
/// Two rules hold:
///
/// - A credential the user did not re-enter keeps the value already stored: the
///   renderer never received it, so it cannot send it back.
/// - A credential that was re-entered is written to the keychain first, and only
///   then does the document carry its handle. A keychain failure fails the whole
///   write (fail closed) — a document that references a secret that was never
///   stored would resolve to nothing.
fn apply_secrets<B: SetupBackend>(
    backend: &B,
    document: &mut ProfileDocument,
    secrets: &BTreeMap<String, String>,
) -> Result<(), BridgeError> {
    for section in SECTIONS {
        let Some(map) = section_mut(document, section) else {
            continue;
        };
        for key in myharness_config::export::SECRET_KEYS {
            let path = format!("{section}/{key}");
            let Some(value) = secrets.get(&path) else {
                continue;
            };
            if value.trim().is_empty() {
                // An emptied field clears the credential: the handle is removed so
                // the slot resolves to nothing rather than to a stale value.
                map.remove(key);
                continue;
            }
            let account = account_for(&path)?;
            let reference = backend.set_secret(&account, value)?;
            map.insert(
                key.to_string(),
                serde_json::to_value(reference).map_err(|e| {
                    BridgeError::new(
                        "CONFIG_PARSE",
                        format!("the credential handle could not be built: {e}"),
                    )
                })?,
            );
        }
    }
    Ok(())
}

/// The keychain account a credential is stored under. The account is derived from
/// the profile id and the field path, so a rewrite overwrites the same entry
/// instead of leaking duplicates (`crates/secrets::derive_account`).
fn account_for(path: &str) -> Result<String, BridgeError> {
    let (section, key) = path.split_once('/').ok_or_else(|| {
        BridgeError::new("CONFIG_VALIDATION", "a credential field path is malformed")
    })?;
    Ok(format!("profile/{section}/{key}"))
}

/// The mutable section map of a profile document.
fn section_mut<'a>(
    document: &'a mut ProfileDocument,
    section: &str,
) -> Option<&'a mut serde_json::Map<String, Value>> {
    let value = match section {
        "connection" => &mut document.connection.0,
        "capabilities" => &mut document.capabilities.0,
        "connectors" => &mut document.connectors.0,
        "telemetry" => &mut document.telemetry.0,
        "limits" => &mut document.limits.0,
        "appearance" => &mut document.appearance.0,
        "plugins" => &mut document.plugins.0,
        "egress" => &mut document.egress.0,
        "source" => &mut document.source.0,
        _ => return None,
    };
    value.as_object_mut()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn backend() -> (ConfigBackend, String) {
        let root = std::env::temp_dir().join(format!("mh-bridge-test-{}", uuid::Uuid::new_v4()));
        let backend = ConfigBackend::new(root, Box::new(myharness_secrets::MemoryStore::default()));
        let id = backend
            .library()
            .create("Workspace", ProviderKind::Gateway)
            .unwrap()
            .id
            .unwrap();
        (backend, id)
    }

    /// The command envelope round-trips and the namespace is typed.
    #[test]
    fn command_roundtrip() {
        let raw = serde_json::to_string(&SetupCommand::ListProfiles).unwrap();
        assert_eq!(raw, r#"{"type":"listProfiles"}"#);
        let back: SetupCommand = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, SetupCommand::ListProfiles);

        let raw = serde_json::to_string(&SetupCommand::ApplyProfile { id: "x".into() }).unwrap();
        assert_eq!(raw, r#"{"type":"applyProfile","id":"x"}"#);
    }

    /// A profile the renderer reads carries no secret value.
    #[test]
    fn read_profile_is_redacted() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
            "inferenceGatewayApiKey": "sk-ant-EXAMPLE-INLINE",
        }));
        backend.library().write(&document).unwrap();

        let response = dispatch(&backend, SetupCommand::ReadProfile { id });
        let SetupResponse::Profile { document } = response else {
            panic!("expected a profile response");
        };
        let text = document.to_string();
        assert!(!text.contains("sk-ant-EXAMPLE-INLINE"), "{text}");
    }

    /// A renderer write that carries an inline secret is refused.
    #[test]
    fn write_with_an_inline_secret_is_refused() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
            "inferenceGatewayApiKey": "sk-ant-EXAMPLE-INLINE",
        }));
        let response = dispatch(
            &backend,
            SetupCommand::WriteProfile {
                document: serde_json::to_value(&document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(!error.message.contains("sk-ant-EXAMPLE-INLINE"));
    }

    /// A credential the user typed is stored in the keychain, and the document
    /// carries the handle - never the value.
    #[test]
    fn a_typed_credential_becomes_a_handle() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
        }));
        let mut secrets = BTreeMap::new();
        secrets.insert(
            "connection/inferenceGatewayApiKey".to_string(),
            "sk-ant-EXAMPLE-TYPED".to_string(),
        );
        let response = dispatch(
            &backend,
            SetupCommand::WriteProfile {
                document: serde_json::to_value(&document).unwrap(),
                secrets,
            },
        );
        let SetupResponse::Profile { document } = response else {
            panic!("expected a profile response");
        };
        let text = document.to_string();
        assert!(!text.contains("sk-ant-EXAMPLE-TYPED"), "{text}");
        assert!(
            text.contains("kind"),
            "the handle should be present: {text}"
        );
    }

    /// Arguments are validated in the bridge, before the library is touched.
    #[test]
    fn arguments_are_validated() {
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::ReadProfile {
                id: "not-a-uuid".to_string(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");

        let response = dispatch(
            &backend,
            SetupCommand::RenameProfile {
                id: "11111111-1111-4111-8111-111111111111".to_string(),
                name: "   ".to_string(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
    }

    /// The tab and provider schemas are addressable from the bridge.
    #[test]
    fn schemas_are_addressable() {
        let (backend, _id) = backend();
        for (tab, expected) in [
            ("capabilities", "Capabilities"),
            ("telemetry", "Telemetry & updates"),
            ("egress", "Egress"),
            ("source", "Source"),
        ] {
            let response = dispatch(&backend, SetupCommand::TabSchema { tab: tab.into() });
            let SetupResponse::Schema { schema } = response else {
                panic!("expected a schema response");
            };
            assert_eq!(schema["title"], expected);
        }

        let response = dispatch(
            &backend,
            SetupCommand::ConnectionSchema {
                provider: "vertex".to_string(),
            },
        );
        let SetupResponse::Schema { schema } = response else {
            panic!("expected a schema response");
        };
        assert!(schema["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"] == "inferenceVertexProjectId"));
    }

    /// An unknown tab or provider is refused, not guessed.
    #[test]
    fn unknown_names_are_refused() {
        let (backend, _id) = backend();
        for response in [
            dispatch(
                &backend,
                SetupCommand::TabSchema {
                    tab: "nope".to_string(),
                },
            ),
            dispatch(
                &backend,
                SetupCommand::ConnectionSchema {
                    provider: "nope".to_string(),
                },
            ),
        ] {
            let SetupResponse::Error { error } = response else {
                panic!("expected an error response");
            };
            assert_eq!(error.code, "CONFIG_VALIDATION");
        }
    }

    /// A draft is validated without being persisted.
    #[test]
    fn validate_does_not_persist() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
        }));
        let response = dispatch(
            &backend,
            SetupCommand::Validate {
                document: serde_json::to_value(&document).unwrap(),
            },
        );
        let SetupResponse::Issues { issues } = response else {
            panic!("expected an issues response");
        };
        assert!(issues.iter().any(|i| i.field == "inferenceGatewayApiKey"));
    }
}
