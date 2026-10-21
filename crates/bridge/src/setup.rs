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

use serde::{Deserialize, Serialize};
use serde_json::Value;

use myharness_config::export::ExportBundle;
use myharness_config::forms::FormIssue;
use myharness_config::health::ConfigHealth;
use myharness_config::overlay::MergedConfig;
use myharness_config::tabs::SetupTab;
use myharness_config::{ConfigError, ConfigLibrary, ProfileDocument, ProviderKind};

/// A command in the `settings.Custom3pSetup` namespace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SetupCommand {
    /// `list` — the profile library.
    ListProfiles,
    /// `read` — one profile document, redacted.
    ReadProfile { id: String },
    /// `write` — persist an edited draft.
    WriteProfile { document: Value },
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
    fn new(code: &str, message: impl Into<String>) -> Self {
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
}

/// A backend over the configuration library.
pub struct ConfigBackend {
    library: ConfigLibrary,
}

impl ConfigBackend {
    /// Open the library at `root`.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        ConfigBackend {
            library: ConfigLibrary::open(root),
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
        SetupCommand::WriteProfile { document } => {
            let document = decode_document(document)?;
            reject_inline_secrets(&document)?;
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
    serde_json::from_value::<SetupTab>(Value::String(tab.to_string())).map_err(|_| {
        BridgeError::new("CONFIG_VALIDATION", "unknown setup tab")
    })
}

fn parse_provider(provider: &str) -> Result<ProviderKind, BridgeError> {
    serde_json::from_value::<ProviderKind>(Value::String(provider.to_string())).map_err(|_| {
        BridgeError::new("CONFIG_VALIDATION", "unknown provider family")
    })
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
fn reject_inline_secrets(document: &ProfileDocument) -> Result<(), BridgeError> {
    let redacted = myharness_config::export::redact_document(document);
    let original = serde_json::to_value(document).unwrap_or(Value::Null);
    if redacted != original {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "a secret must be stored as a keychain handle, not as a value",
        ));
    }
    Ok(())
}

/// Redact a profile document for the renderer.
fn redact(document: &ProfileDocument) -> Value {
    myharness_config::export::redact_document(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn backend() -> (ConfigBackend, String) {
        let root = std::env::temp_dir().join(format!("mh-bridge-test-{}", uuid::Uuid::new_v4()));
        let backend = ConfigBackend::new(root);
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
            "inferenceGatewayApiKey": "sk-ant-example-value",
        }));
        backend.library().write(&document).unwrap();

        let response = dispatch(&backend, SetupCommand::ReadProfile { id });
        let SetupResponse::Profile { document } = response else {
            panic!("expected a profile response");
        };
        let text = document.to_string();
        assert!(!text.contains("sk-ant-example-value"), "{text}");
    }

    /// A renderer write that carries an inline secret is refused.
    #[test]
    fn write_with_an_inline_secret_is_refused() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
            "inferenceGatewayApiKey": "sk-ant-example-value",
        }));
        let response = dispatch(
            &backend,
            SetupCommand::WriteProfile {
                document: serde_json::to_value(&document).unwrap(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(!error.message.contains("sk-ant-example-value"));
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
