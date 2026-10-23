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

use myharness_provider_anthropic::AnthropicAdapter;
use myharness_provider_gateway::{GatewayAdapter, GatewayClientConfig, ProbeStatus};
use myharness_provider_sdk::{AuthContext, ErrorClass, ProviderAdapter, ProviderError};
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
    /// `testConnection` — the Connection tab's *Test connection* (`docs/13`
    /// Phase 3): a real one-token inference against the gateway through the
    /// native adapter. The typed credential rides `secrets` exactly like the
    /// write path. The draft is neither persisted nor logged, and no stored
    /// credential is ever substituted for a failure path.
    TestConnection {
        document: Value,
        #[serde(default)]
        secrets: BTreeMap<String, String>,
    },
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
    /// The *Test connection* verdict: the probe's real usage, latency, and
    /// the native stop reason. Failures surface as `Error`.
    Probe {
        probe: myharness_provider_gateway::ProbeStatus,
    },
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
    /// The Connection tab's *Test connection*: a real one-token inference
    /// through the native gateway adapter (`docs/12` §8.3; `docs/13` Phase 3).
    ///
    /// The draft is never persisted and never modified; the credential is
    /// resolved per request and exists only inside this call; a failed
    /// resolution is an error, never a substitution.
    fn test_connection(
        &self,
        document: &ProfileDocument,
        typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError>;
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

    fn test_connection(
        &self,
        document: &ProfileDocument,
        typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError> {
        match document.provider.unwrap_or(ProviderKind::Gateway) {
            ProviderKind::Gateway => self.test_connection_gateway(document, typed),
            ProviderKind::ClaudeApi => self.test_connection_claude_api(document, typed),
            ProviderKind::Bedrock => self.test_connection_bedrock(document, typed),
            ProviderKind::Vertex => self.test_connection_vertex(document, typed),
            // The remaining families have no native adapter in this slice;
            // their probe lanes arrive with their adapters (roadmap order:
            // Mantle, Foundry — `docs/13` Phase 3).
            ProviderKind::BedrockMantle | ProviderKind::Foundry => {
                Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    "this probe verifies the gateway, Claude API, Bedrock, and Vertex families only",
                ))
            }
        }
    }
}

impl ConfigBackend {
    /// The gateway lane: the draft's gateway base URL and scheme decide the
    /// wire; the credential resolves strictly in the family's slot.
    fn test_connection_gateway(
        &self,
        document: &ProfileDocument,
        typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError> {
        let connection = &document.connection.0;
        let base = connection
            .get("inferenceGatewayBaseUrl")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|base| !base.is_empty());
        let Some(base) = base else {
            // A missing endpoint blocks before any request is sent.
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                "the gateway base URL is required for the connection probe",
            ));
        };
        let scheme = parse_probe_scheme(connection.get("inferenceGatewayAuthScheme"))?;
        let idle = connection
            .get("inferenceStreamIdleTimeoutSec")
            .and_then(Value::as_u64)
            .map(std::time::Duration::from_secs)
            .unwrap_or(myharness_provider_gateway::DEFAULT_IDLE_TIMEOUT);
        let custom_headers = custom_headers_for_probe(connection)?;

        // The credential kind decides the source (`docs/09` §2.2): a static
        // slot (typed value first, stored handle second), or the helper
        // script run once. An interactive sign-in cannot be probed and is
        // never emulated.
        let credential = resolve_probe_credential(
            connection,
            typed,
            self.secrets.as_ref(),
            "inferenceGatewayApiKey",
            true,
        )?;
        let secret = match credential {
            ProbeCredential::Value(secret) => secret,
            ProbeCredential::Helper(config) => {
                myharness_secrets::helper::run_helper(&config, cfg!(windows))
                    .map_err(bridge_secret_error)?
            }
        };

        let config = GatewayClientConfig {
            base_url: base.to_string(),
            auth_scheme: scheme,
            custom_headers,
            idle_timeout: idle,
            betas: Vec::new(),
        };
        let adapter = GatewayAdapter::new(config, reqwest::Client::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                BridgeError::new(
                    "CONFIG_IO",
                    format!("the probe runtime could not start: {error}"),
                )
            })?;
        runtime.block_on(async {
            let auth_label = "setup test connection";
            // The probe infers with a real model id: the draft's list when it
            // carries one that the Anthropic model policy accepts, discovery
            // over the same credential otherwise.
            let model_id = match first_legal_probe_model(connection) {
                Some(model) => model,
                None => {
                    let models = adapter
                        .list_models(&AuthContext::new(secret.clone(), auth_label))
                        .await
                        .map_err(bridge_provider_error)?;
                    models
                        .iter()
                        .map(|model| model.key.model_id.clone())
                        .find(|id| myharness_config::forms::validate_anthropic_model_id(id).is_ok())
                        .ok_or_else(|| {
                            BridgeError::new(
                                "PROVIDER_ERROR",
                                "the gateway advertised no Anthropic-legal model for a probe",
                            )
                        })?
                }
            };
            // Two calls, one probe session, one credential: discovery picks
            // the target, the probe infers against it — a one-token request
            // whose result must echo the requested model.
            adapter
                .probe_inference(&AuthContext::new(secret.clone(), auth_label), &model_id)
                .await
                .map_err(bridge_provider_error)
        })
    }
}

impl ConfigBackend {
    /// The Claude API lane (`docs/09` §2.2 "Claude API"; `docs/18` §6): the
    /// family's native first-party endpoint with optional base URL — the
    /// field's on-wire spelling is unverified
    /// (`UNVERIFIED_ANTHROPIC_BASE_URL` in `crates/config/src/forms.rs`;
    /// recorded in `docs/14` §9) — and the family's native `x-api-key` auth.
    /// The credential kinds the family offers are static key and interactive
    /// sign-in only; helper scripts are not part of this family's form
    /// (`docs/12` §1).
    fn test_connection_claude_api(
        &self,
        document: &ProfileDocument,
        typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError> {
        let connection = &document.connection.0;
        let base: Option<String> = match connection.get("inferenceAnthropicBaseUrl") {
            None | Some(Value::Null) => None,
            Some(Value::String(raw)) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(other) => {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    format!("the Anthropic base URL expects a string, found {other:?}"),
                ));
            }
        };
        let custom_headers = custom_headers_for_probe(connection)?;
        let credential = resolve_probe_credential(
            connection,
            typed,
            self.secrets.as_ref(),
            "inferenceAnthropicApiKey",
            false,
        )?;
        let secret = match credential {
            ProbeCredential::Value(secret) => secret,
            // The resolver refuses `helper-script` before any request for a
            // family that does not offer it; this arm exists for exhaustiveness.
            ProbeCredential::Helper(_) => {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    "helper scripts are not offered for the Claude API family",
                ));
            }
        };

        let config = myharness_provider_anthropic::AnthropicClientConfig {
            // Absent means the first-party host (`docs/12` §1); the adapter
            // owns the default and the trailing-slash normalization.
            base_url: base,
            custom_headers,
            // The Claude API card carries no idle-timeout field; the crate's
            // documented default applies (`docs/14` §9).
            idle_timeout: myharness_provider_anthropic::DEFAULT_IDLE_TIMEOUT,
            betas: Vec::new(),
        };
        let adapter = AnthropicAdapter::new(config, reqwest::Client::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                BridgeError::new(
                    "CONFIG_IO",
                    format!("the probe runtime could not start: {error}"),
                )
            })?;
        runtime.block_on(async {
            let auth_label = "setup test connection";
            // Same probe shape as the gateway lane: the draft's legal model
            // id, or discovery over the same credential to pick one. At most
            // four requests travel in total (discovery pages + one inference).
            let model_id = match first_legal_probe_model(connection) {
                Some(model) => model,
                None => {
                    let models = adapter
                        .list_models(&AuthContext::new(secret.clone(), auth_label))
                        .await
                        .map_err(bridge_provider_error)?;
                    models
                        .iter()
                        .map(|model| model.key.model_id.clone())
                        .find(|id| myharness_config::forms::validate_anthropic_model_id(id).is_ok())
                        .ok_or_else(|| {
                            BridgeError::new(
                                "PROVIDER_ERROR",
                                "the provider advertised no Anthropic-legal model for a probe",
                            )
                        })?
                }
            };
            adapter
                .probe_inference(&AuthContext::new(secret.clone(), auth_label), &model_id)
                .await
                .map_err(bridge_provider_error)
                // The response type is the renderer's shared probe shape
                // (`apps/desktop/src/setup/client.ts`): the field sets are
                // identical, so the value maps field-for-field — no
                // translation, just the typed seam.
                .map(probe_status_to_renderer)
        })
    }
}

impl ConfigBackend {
    /// The Bedrock lane (`docs/09` §2.2 "Bedrock"; `docs/18` §6): the
    /// family's runtime endpoint derived from the required region, an
    /// optional override base URL, and the family's native bearer credential
    /// (`inferenceBedrockBearerToken`; `docs/09` §2.2 "static bearer").
    /// Credential kinds the family offers but whose sign-in flows were never
    /// captured — AWS profile/SSO, web identity, vendor profile — are
    /// refused here, never emulated (`docs/14` §9).
    fn test_connection_bedrock(
        &self,
        document: &ProfileDocument,
        typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError> {
        let connection = &document.connection.0;
        let region = connection
            .get("inferenceBedrockRegion")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|region| !region.is_empty());
        let Some(region) = region else {
            // The region forms both endpoints (`docs/12` §1): absent, no
            // request is formed.
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                "the Bedrock region is required for the connection probe",
            ));
        };
        let override_base: Option<String> = match connection.get("inferenceBedrockBaseUrl") {
            None | Some(Value::Null) => None,
            Some(Value::String(raw)) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(other) => {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    format!("the Bedrock base URL expects a string, found {other:?}"),
                ));
            }
        };
        // One override replaces both endpoints (`docs/14` §9); when absent,
        // the region derives both (`docs/12` §1 host shapes).
        let (runtime_base, control_base) = if let Some(base) = &override_base {
            (base.clone(), base.clone())
        } else {
            (
                myharness_provider_bedrock::default_runtime_base_url(region)
                    .map_err(bridge_provider_error)?
                    .trim_end_matches('/')
                    .to_string(),
                myharness_provider_bedrock::default_control_base_url(region)
                    .map_err(bridge_provider_error)?
                    .trim_end_matches('/')
                    .to_string(),
            )
        };
        let custom_headers = custom_headers_for_probe(connection)?;

        // The credential kinds the family offers: static bearer and helper
        // script probe; profile/SSO (`sso`), web identity (`external-idp`),
        // and vendor profile (`vendor-profile`) are identity-sign-in kinds
        // this rebuild cannot resolve — the resolver refuses them, never
        // substitutes (`docs/12` §1 credential-kind column; `docs/14` §9).
        let credential = resolve_probe_credential(
            connection,
            typed,
            self.secrets.as_ref(),
            "inferenceBedrockBearerToken",
            true,
        )?;
        let secret = match credential {
            ProbeCredential::Value(secret) => secret,
            ProbeCredential::Helper(config) => {
                myharness_secrets::helper::run_helper(&config, cfg!(windows))
                    .map_err(bridge_secret_error)?
            }
        };

        let config = myharness_provider_bedrock::BedrockClientConfig {
            runtime_base_url: runtime_base,
            control_base_url: control_base,
            custom_headers,
            // The Bedrock card carries no idle-timeout field; the crate's
            // documented default applies (`docs/14` §9).
            idle_timeout: myharness_provider_bedrock::DEFAULT_IDLE_TIMEOUT,
        };
        let adapter =
            myharness_provider_bedrock::BedrockAdapter::new(config, reqwest::Client::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                BridgeError::new(
                    "CONFIG_IO",
                    format!("the probe runtime could not start: {error}"),
                )
            })?;
        runtime.block_on(async {
            let auth_label = "setup test connection";
            // Same probe shape as the other lanes: the draft's legal model
            // id, or the control plane's listing over the same credential to
            // pick one. At most four requests travel in total (discovery
            // pages + one inference).
            let model_id = match first_legal_probe_model_where(connection, |id| {
                myharness_config::forms::validate_bedrock_model_id(id).is_ok()
            }) {
                Some(model) => model,
                None => {
                    let models = adapter
                        .list_models(&AuthContext::new(secret.clone(), auth_label))
                        .await
                        .map_err(bridge_provider_error)?;
                    models
                        .iter()
                        .map(|model| model.key.model_id.clone())
                        .find(|id| myharness_config::forms::validate_bedrock_model_id(id).is_ok())
                        .ok_or_else(|| {
                            BridgeError::new(
                                "PROVIDER_ERROR",
                                "the provider advertised no Bedrock-legal model for a probe",
                            )
                        })?
                }
            };
            adapter
                .probe_inference(&AuthContext::new(secret.clone(), auth_label), &model_id)
                .await
                .map_err(bridge_provider_error)
                // The response type is the renderer's shared probe shape
                // (`apps/desktop/src/setup/client.ts`): the field sets are
                // identical, so the value maps field-for-field — no
                // translation, just the typed seam.
                .map(bedrock_probe_status_to_renderer)
        })
    }

    /// The Vertex lane (`docs/09` §2.2 "Vertex AI"; Phase 3 slice 4): the
    /// regional host derived from the required region
    /// (`https://{region}-aiplatform.googleapis.com`), the required project
    /// id (both travel in the request path), and an optional override base
    /// URL that replaces the derived host in full. The credential kind is
    /// resolved strictly per the family's kind table: the helper-script kind
    /// runs once and its output is the Google bearer the adapter sends; the
    /// sign-in/identity kinds are refused (`resolve_vertex_probe_credential`,
    /// `docs/14` §9 item 28). **No discovery exists for this family** — model
    /// discovery is disabled for every credential kind it offers
    /// (`docs/09` §2.2), so the draft must list the `claude-` model id the
    /// probe targets; exactly one request travels.
    fn test_connection_vertex(
        &self,
        document: &ProfileDocument,
        _typed: &BTreeMap<String, String>,
    ) -> Result<ProbeStatus, BridgeError> {
        let connection = &document.connection.0;
        let region = connection
            .get("inferenceVertexRegion")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|region| !region.is_empty());
        let Some(region) = region else {
            // The region forms the host (`docs/12` §1): absent, no request
            // is formed.
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                "the Vertex region is required for the connection probe",
            ));
        };
        let project = connection
            .get("inferenceVertexProjectId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|project| !project.is_empty());
        let Some(project) = project else {
            // The project forms the request path: absent, no request is
            // formed.
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                "the Vertex project id is required for the connection probe",
            ));
        };
        let override_base: Option<String> = match connection.get("inferenceVertexBaseUrl") {
            None | Some(Value::Null) => None,
            Some(Value::String(raw)) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(other) => {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    format!("the Vertex base URL expects a string, found {other:?}"),
                ));
            }
        };
        let base = match override_base {
            Some(base) => base,
            None => myharness_provider_vertex::default_base_url(region)
                .map_err(bridge_provider_error)?
                .trim_end_matches('/')
                .to_string(),
        };
        let custom_headers = custom_headers_for_probe(connection)?;

        // No discovery fallback exists: the draft must name the model
        // (`docs/09` §2.2 "Vertex AI" disables discovery for every kind the
        // family offers). The family's own policy gate decides which ids
        // are legal (`docs/12` §2.2; `TEST-AI-003`). The gate runs before
        // the credential so a refused draft never executes the helper.
        let model_id = first_legal_probe_model_where(connection, |id| {
            myharness_config::forms::validate_vertex_model_id(id).is_ok()
        })
        .ok_or_else(|| {
            BridgeError::new(
                "PROVIDER_ERROR",
                "the draft carries no claude- model id for the Vertex probe: model discovery is unavailable for this family's credential kinds (docs/09 §2.2 \"Vertex AI\")",
            )
        })?;

        // The credential: strictly per the family's kind table, above. The
        // helper script runs once; its output is never stored.
        let credential = resolve_vertex_probe_credential(connection)?;
        let secret = match credential {
            VertexProbeCredential::Helper(config) => {
                myharness_secrets::helper::run_helper(&config, cfg!(windows))
                    .map_err(bridge_secret_error)?
            }
        };

        let config = myharness_provider_vertex::VertexClientConfig {
            base_url: base,
            project_id: project.to_string(),
            region: region.to_string(),
            custom_headers,
            // The Vertex card carries no idle-timeout field; the crate's
            // documented default applies (`docs/14` §9 item 28).
            idle_timeout: myharness_provider_vertex::DEFAULT_IDLE_TIMEOUT,
        };
        let adapter = myharness_provider_vertex::VertexAdapter::new(config, reqwest::Client::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                BridgeError::new(
                    "CONFIG_IO",
                    format!("the probe runtime could not start: {error}"),
                )
            })?;
        runtime.block_on(async {
            let auth_label = "setup test connection";
            adapter
                .probe_inference(&AuthContext::new(secret, auth_label), &model_id)
                .await
                .map_err(bridge_provider_error)
                // Identical field sets, one typed seam per family.
                .map(vertex_probe_status_to_renderer)
        })
    }
}

/// Map the Bedrock adapter's probe verdict onto the setup response's probe
/// type (`Probe { probe }`): identical fields, one typed seam per family.
fn bedrock_probe_status_to_renderer(
    status: myharness_provider_bedrock::ProbeStatus,
) -> ProbeStatus {
    ProbeStatus {
        ok: status.ok,
        model: status.model,
        stop_reason: status.stop_reason,
        input_tokens: status.input_tokens,
        output_tokens: status.output_tokens,
        latency_ms: status.latency_ms,
    }
}

/// Map the Vertex adapter's probe verdict onto the setup response's probe
/// type (`Probe { probe }`): identical fields, one typed seam per family.
fn vertex_probe_status_to_renderer(
    status: myharness_provider_vertex::ProbeStatus,
) -> ProbeStatus {
    ProbeStatus {
        ok: status.ok,
        model: status.model,
        stop_reason: status.stop_reason,
        input_tokens: status.input_tokens,
        output_tokens: status.output_tokens,
        latency_ms: status.latency_ms,
    }
}

/// The Vertex probe's credential source. The family offers interactive
/// (Google sign-in), Workforce, helper-script, and vendor-profile kinds
/// (`docs/09` §2.2), and **no static slot** — there is no secret field on
/// the form for a stored key. The sign-in / identity kinds carry flows that
/// were never captured: they are refused here, never emulated, and the
/// adapter takes only material the caller resolves (`docs/14` §9 item 28).
enum VertexProbeCredential {
    Helper(myharness_secrets::helper::HelperConfig),
}

/// Resolve the Vertex probe's credential, strictly: the draft must name a
/// kind, and only the helper-script kind resolves — its command runs once
/// inside `test_connection` (`run_helper`, never the caching resolver: a
/// probe must not persist anything, `docs/14` §9 item 21's reading) and its
/// output is the bearer the adapter sends. No raw value is logged or
/// carried in an error message.
fn resolve_vertex_probe_credential(
    connection: &Value,
) -> Result<VertexProbeCredential, BridgeError> {
    let kind = connection
        .get("inferenceCredentialKind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|kind| !kind.is_empty());
    match kind {
        Some("helper-script") => {
            let config = myharness_secrets::helper::HelperConfig::from_connection(connection)
                .map_err(bridge_secret_error)?
                .ok_or_else(|| {
                    BridgeError::new(
                        "CONFIG_VALIDATION",
                        "helper script is required for the helper credential kind",
                    )
                })?;
            Ok(VertexProbeCredential::Helper(config))
        }
        Some("interactive") | Some("workforce") | Some("external-idp") | Some("vendor-profile") => {
            Err(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("credential kind {kind:?} cannot be probed"),
            ))
        }
        Some(other) => Err(BridgeError::new(
            "CONFIG_VALIDATION",
            format!(
                "credential kind {other:?} is not one of the kinds the Vertex form offers (docs/09 §2.2 \"Vertex AI\")"
            ),
        )),
        None => Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the Vertex connection names no credential kind; the form offers Google sign-in, Workforce, helper script, and vendor profile, and the probe resolves the helper script only",
        )),
    }
}

/// Map the Claude API adapter's probe verdict onto the setup response's probe
/// type (`Probe { probe }`): identical fields, one typed seam.
fn probe_status_to_renderer(status: myharness_provider_anthropic::ProbeStatus) -> ProbeStatus {
    ProbeStatus {
        ok: status.ok,
        model: status.model,
        stop_reason: status.stop_reason,
        input_tokens: status.input_tokens,
        output_tokens: status.output_tokens,
        latency_ms: status.latency_ms,
    }
}

/// The probe's credential source. `Helper` carries the configured helper; its
/// script runs once inside `test_connection` and its output is never stored
/// (a probe must not persist anything — the TTL/cache machinery belongs to
/// the runtime, not to a draft check).
enum ProbeCredential {
    Value(secrecy::SecretString),
    Helper(myharness_secrets::helper::HelperConfig),
}

/// Resolve the credential the probe will use, strictly (`docs/09` §2.2): the
/// typed value first, the draft's own slot value second, a stored keychain
/// handle last — and only one of them. A missing or unreachable credential is
/// an error; nothing is substituted, nothing is executed, and no raw value is
/// logged or carried in an error message. The slot key is the family's own
/// spelling (`inferenceGatewayApiKey` / `inferenceAnthropicApiKey`), and
/// `allow_helper` is the family's offered-kind set (`docs/12` §1).
fn resolve_probe_credential(
    connection: &Value,
    typed: &BTreeMap<String, String>,
    store: &dyn SecretStore,
    slot_key: &'static str,
    allow_helper: bool,
) -> Result<ProbeCredential, BridgeError> {
    let kind = connection
        .get("inferenceCredentialKind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .unwrap_or("static");

    match kind {
        "static" => {}
        "helper-script" => {
            // A family whose form does not offer helper scripts refuses the
            // kind outright (`docs/12` §1 credential-kind column).
            if !allow_helper {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    "helper scripts are not offered for this provider family",
                ));
            }
            let config = myharness_secrets::helper::HelperConfig::from_connection(connection)
                .map_err(bridge_secret_error)?
                .ok_or_else(|| {
                    BridgeError::new(
                        "CONFIG_VALIDATION",
                        "helper script is required for the helper credential kind",
                    )
                })?;
            return Ok(ProbeCredential::Helper(config));
        }
        // An interactive sign-in cannot be probed without completing the
        // sign-in flow, and the app does not emulate it (docs/14 §9).
        "interactive" => {
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                "interactive sign-in cannot be verified by this probe; finish signing in, then test again with a saved profile",
            ));
        }
        other => {
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("credential kind {other:?} cannot be probed"),
            ));
        }
    }

    // The static lane: the typed value wins, then the draft's own value, then
    // the keychain handle. `run` never subsitutes one for another.
    let typed_key = format!("connection/{slot_key}");
    if let Some(value) = typed.get(&typed_key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(ProbeCredential::Value(secrecy::SecretString::from(
                trimmed.to_string(),
            )));
        }
    }
    match connection.get(slot_key) {
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(BridgeError::new(
                    "CONFIG_VALIDATION",
                    "the api key field carries no value",
                ));
            }
            Ok(ProbeCredential::Value(secrecy::SecretString::from(
                trimmed.to_string(),
            )))
        }
        Some(value @ Value::Object(_)) => {
            let reference: SecretRef = serde_json::from_value(value.clone()).map_err(|_| {
                BridgeError::new(
                    "CONFIG_VALIDATION",
                    "the api key field is not a value nor a credential handle",
                )
            })?;
            match store.load(&reference.account) {
                Ok(Some(secret)) => Ok(ProbeCredential::Value(secret)),
                Ok(None) => Err(BridgeError::new(
                    "SECRET_MISSING",
                    "no credential is stored for this profile's api key; save the profile, then test again",
                )),
                Err(error) => Err(bridge_secret_error(error)),
            }
        }
        _ => Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "a credential is required for the connection probe",
        )),
    }
}

/// The probe's auth scheme: the form's plain values are the wire spellings.
fn parse_probe_scheme(value: Option<&Value>) -> Result<crate::helper::AuthScheme, BridgeError> {
    match value {
        None => Ok(crate::helper::AuthScheme::Bearer),
        Some(Value::String(raw)) => match raw.trim() {
            "bearer" => Ok(crate::helper::AuthScheme::Bearer),
            "x-api-key" => Ok(crate::helper::AuthScheme::XApiKey),
            other => Err(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("auth scheme {other:?} is not one of the contract's bearer / x-api-key"),
            )),
        },
        Some(other) => Err(BridgeError::new(
            "CONFIG_VALIDATION",
            format!("the auth scheme field expects a string, found {other:?}"),
        )),
    }
}

/// The routing headers the probe sends. Validate-time rules hold: a
/// credential-shaped name or value is refused instead of sent (TEST-SEC-004
/// discipline; the credential section is the only credential lane).
fn custom_headers_for_probe(connection: &Value) -> Result<Vec<(String, String)>, BridgeError> {
    let Some(headers) = connection
        .get("inferenceCustomHeaders")
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    let mut collected = Vec::new();
    for (name, value) in headers {
        let Some(raw) = value.as_str() else {
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("custom header {name:?} expects a string value"),
            ));
        };
        let lower = name.trim().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "authorization" | "proxy-authorization" | "x-api-key" | "anthropic-api-key"
        ) || raw.trim().to_ascii_lowercase().starts_with("bearer ")
        {
            return Err(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("custom header {name:?} carries credentials; use the credential section instead"),
            ));
        }
        collected.push((name.trim().to_string(), raw.to_string()));
    }
    Ok(collected)
}

/// The first model id of the draft's list that the Anthropic model policy
/// accepts. A non-Anthropic id is not probed (the policy refuses it); an
/// empty or absent list means discovery decides.
fn first_legal_probe_model(connection: &Value) -> Option<String> {
    first_legal_probe_model_where(connection, |id| {
        myharness_config::forms::validate_anthropic_model_id(id).is_ok()
    })
}

/// The first model id of the draft's list that the family's own policy
/// accepts (`docs/12` §2.2 family validators). A non-conforming id is not
/// probed; an empty or absent list means the caller falls back to discovery.
fn first_legal_probe_model_where(
    connection: &Value,
    legal: impl Fn(&str) -> bool,
) -> Option<String> {
    let list = connection
        .get("inferenceModels")
        .and_then(Value::as_array)?;
    list.iter().find_map(|entry| {
        let id = entry.as_str().map(str::trim).filter(|id| !id.is_empty());
        id.filter(|id| legal(id)).map(str::to_string)
    })
}

/// Map a provider error onto the setup bridge's codes, with the provider's
/// message verbatim — the same code strings the helper wrapper maps
/// (`docs/18` §11: no body text, no secret material).
pub(crate) fn bridge_provider_error(error: ProviderError) -> BridgeError {
    if let ErrorClass::Configuration = error.class {
        return BridgeError::new("CONFIG_VALIDATION", error.message);
    }
    let code = match error.class {
        ErrorClass::Unreachable => "PROVIDER_UNREACHABLE",
        _ => "PROVIDER_ERROR",
    };
    BridgeError::new(code, error.message)
}

/// Map a secret-source error onto the bridge: the `SECRET_*` prefix of the
/// crate's own Display is the bridge's code, the rest is the (value-free)
/// message (`docs/09` §4: no value ever appears in an error).
fn bridge_secret_error(error: myharness_secrets::SecretError) -> BridgeError {
    // The secret crate's errors never carry a credential value.
    match error.to_string().split_once(": ") {
        Some((code, rest)) if code.starts_with("SECRET_") => BridgeError::new(code, rest),
        _ => BridgeError::new("SECRET_STORE", "the credential could not be resolved"),
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
        SetupCommand::TestConnection { document, secrets } => {
            let document = decode_document(document)?;
            // Nothing here persists: the probe resolves against the draft and
            // the keychain, infers once, and returns the verdict.
            Ok(SetupResponse::Probe {
                probe: backend.test_connection(&document, &secrets)?,
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

    // -----------------------------------------------------------------------
    // Test connection (docs/13 Phase 3) — the setup-path battery
    // -----------------------------------------------------------------------

    /// The body of a canned response.
    fn response_bytes(value: &serde_json::Value) -> Vec<u8> {
        value.to_string().into_bytes()
    }

    /// One canned HTTP response.
    fn canned(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    /// A local gateway recording how many connections it answered. The queue
    /// order is the flow order; the flat count exposes *any* traffic a
    /// refused draft must not produce.
    struct StubGateway {
        base: String,
        hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for StubGateway {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl StubGateway {
        fn serve(canned: Vec<Vec<u8>>) -> StubGateway {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
            let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let handle = {
                let hits = hits.clone();
                let stop = stop.clone();
                std::thread::spawn(move || {
                    let _ = listener.set_nonblocking(true);
                    let mut next = 0usize;
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        let Ok((mut stream, _)) = listener.accept() else {
                            std::thread::sleep(std::time::Duration::from_millis(3));
                            continue;
                        };
                        let mut buffer = [0u8; 4096];
                        let _ = stream.read(&mut buffer);
                        hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if next < canned.len() {
                            let _ = stream.write_all(&canned[next]);
                            let _ = stream.flush();
                            next += 1;
                        }
                    }
                })
            };
            StubGateway {
                base,
                hits,
                stop,
                handle: Some(handle),
            }
        }

        fn hits(&self) -> usize {
            self.hits.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    /// A gateway draft over a real library entry, pointed at `stub`.
    fn backend_again() -> (ConfigBackend, String) {
        backend()
    }

    fn gateway_document(stub: &str) -> ProfileDocument {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": stub,
            "inferenceGatewayAuthScheme": "bearer",
            "inferenceModels": ["claude-opus-4-1"],
        }));
        document
    }

    /// The probe refuses a draft for a family whose adapter is not in the
    /// build yet, before anything sends. The probe-capable families unlock
    /// one adapter at a time (roadmap order, `docs/13` Phase 3): gateway,
    /// then the Claude API family, then Bedrock, then Vertex; Mantle and
    /// Foundry arrive last.
    #[test]
    fn test_connection_refuses_an_unreleased_family_draft() {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.provider = Some(ProviderKind::BedrockMantle);
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error
            .message
            .contains("verifies the gateway, Claude API, Bedrock, and Vertex families only"));

        // The draft never changed: the probe must not persist anything.
        let reread = backend.library().read(&id).unwrap();
        assert!(reread.connection.0.get("inferenceModels").is_none());
    }

    /// An interactive credential is refused before anything sends — the app
    /// does not emulate an interactive sign-in (`docs/14` §9), and a refusal
    /// carries no request and no substitute credential.
    #[test]
    fn test_connection_refuses_interactive_without_a_request() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = gateway_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        connection.insert("inferenceCredentialKind".to_string(), json!("interactive"));
        connection.insert(
            "inferenceGatewayApiKey".to_string(),
            json!("sk-ant-EXAMPLE"),
        );
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("cannot be verified by this probe"));
        assert_eq!(stub.hits(), 0, "a refused kind must send nothing");
    }

    /// A draft with no credential blocks before anything sends: the missing
    /// key is an error, never a stored-secret substitution, never a fallback.
    #[test]
    fn test_connection_blocks_without_a_credential_and_without_a_request() {
        let stub = StubGateway::serve(Vec::new());
        let document = gateway_document(&stub.base);
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("a credential is required"));
        assert_eq!(stub.hits(), 0, "a missing credential sends nothing");
    }

    /// The happy path: the draft's model list picks the target and the probe
    /// is a real inference — one request, tokens and latency reported.
    #[test]
    fn test_connection_sends_a_real_inference() {
        let completion = json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        });
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&completion),
        )]);
        let mut document = gateway_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        connection.insert(
            "inferenceGatewayApiKey".to_string(),
            json!("sk-ant-EXAMPLE-TYPED"),
        );
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(probe.model, "claude-opus-4-1");
        assert_eq!(probe.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(probe.input_tokens, 3);
        assert_eq!(probe.output_tokens, 1);
        assert!(probe.latency_ms > 0);
        assert_eq!(stub.hits(), 1, "the draft's model list skips discovery");
    }

    /// A typed value beats everything: the draft may even carry a stored
    /// handle — the probe uses the typed value and never substitutes the
    /// store's credential in its place.
    #[test]
    fn test_connection_uses_the_typed_value_over_the_stored_slot() {
        let completion = json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        });
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&completion),
        )]);
        let mut document = gateway_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        // A handle for a slot that holds a DIFFERENT credential.
        connection.insert(
            "inferenceGatewayApiKey".to_string(),
            json!({ "kind": "keychain", "service": "probe", "account": "decoy/account" }),
        );
        let (backend, _id) = backend();
        let mut secrets = BTreeMap::new();
        secrets.insert(
            "connection/inferenceGatewayApiKey".to_string(),
            "sk-ant-EXAMPLE-TYPED".to_string(),
        );
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets,
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok, "the typed value must be the one used");
        // The decoy was never unused-and-silent: a probe must not persist
        // anything, so the decoy account stays untouched even on success.
        assert_eq!(stub.hits(), 1);
    }

    /// A stored handle resolves through the keychain — and a slot that points
    /// nowhere is an explicit error (`SECRET_MISSING`), never an omission.
    #[test]
    fn test_connection_resolves_the_stored_slot_fail_closed() {
        let (backend, id) = backend();
        let reference = backend
            .set_secret("probe/connection/inferenceGatewayApiKey", "sk-ant-STORED")
            .unwrap();
        let mut document = backend.library().read(&id).unwrap();
        document.connection = myharness_config::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway.example",
            "inferenceGatewayAuthScheme": "bearer",
            "inferenceModels": ["claude-opus-4-1"],
            "inferenceGatewayApiKey": {
                "kind": "keychain",
                "service": reference.service,
                "account": reference.account,
            },
        }));
        // The resolvable slot reaches a live gateway...
        let completion = json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        });
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&completion),
        )]);
        let document = {
            let value = serde_json::to_value(&document).unwrap();
            let mut value = value;
            value
                .as_object_mut()
                .unwrap()
                .get_mut("connection")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("inferenceGatewayBaseUrl".to_string(), json!(stub.base));
            serde_json::from_value::<ProfileDocument>(value).unwrap()
        };
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(stub.hits(), 1);

        // ...and a slot pointing nowhere fails closed with a real code.
        {
            let (fresh, _id) = backend_again();
            let mut document = gateway_document("https://gateway.example");
            document.connection.0.as_object_mut().unwrap().insert(
                "inferenceGatewayApiKey".to_string(),
                json!({ "kind": "keychain", "service": "nowhere", "account": "nowhere/account" }),
            );
            let response = dispatch(
                &fresh,
                SetupCommand::TestConnection {
                    document: serde_json::to_value(document).unwrap(),
                    secrets: BTreeMap::new(),
                },
            );
            let SetupResponse::Error { error } = response else {
                panic!("expected an error response");
            };
            assert_eq!(error.code, "SECRET_MISSING");
            assert!(error.message.contains("no credential is stored"));
        }
    }

    /// The draft's non-Anthropic id is skipped: the model policy decides the
    /// target, and the probe's echo check proves the requested id ran.
    #[test]
    fn test_connection_skips_a_non_anthropic_draft_entry() {
        let completion = json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        });
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&completion),
        )]);
        let mut document = gateway_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        // A whole other family (`gpt-…`) is exactly what the Anthropic model
        // policy refuses — the probe skips it and takes the legal entry.
        connection.insert(
            "inferenceModels".to_string(),
            json!(["gpt-4o", "claude-opus-4-1"]),
        );
        connection.insert(
            "inferenceGatewayApiKey".to_string(),
            json!("sk-ant-EXAMPLE-TYPED"),
        );
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok, "the echo check must hold for the legal id");
        assert_eq!(stub.hits(), 1);
    }

    /// The helper credential kind: the script's output is the probe's
    /// credential, run once, never stored (unix only — the script is a
    /// platform binary trick for the fixture).
    #[cfg(unix)]
    #[test]
    fn test_connection_runs_the_helper_kind_without_persisting_it() {
        let completion = json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        });
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&completion),
        )]);
        let mut document = gateway_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        connection.insert(
            "inferenceCredentialKind".to_string(),
            json!("helper-script"),
        );
        connection.insert("inferenceCredentialHelper".to_string(), json!("/bin/echo"));
        connection.insert(
            "inferenceCredentialHelperArgs".to_string(),
            json!("sk-ant-HELPER-OUTPUT"),
        );
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(stub.hits(), 1);
        // A probe must not persist anything: the helper's output was never
        // written to the keychain under a draft-only account.
    }

    /// A Claude API draft over a live stub, pointed at the family's optional
    /// base URL field (the field's spelling is a `docs/14` §9 unknown — the
    /// rebuild contract writes `inferenceAnthropicBaseUrl`).
    fn claude_document(stub: &str) -> ProfileDocument {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.provider = Some(ProviderKind::ClaudeApi);
        document.connection = myharness_config::Section(json!({
            "inferenceAnthropicBaseUrl": stub,
            "inferenceAnthropicApiKey": "sk-ant-EXAMPLE-FAMILY",
            "inferenceModels": ["claude-opus-4-1"],
        }));
        document
    }

    /// A Claude API completion for the stub: the same native one-token probe
    /// answer any Anthropic Messages host gives.
    fn claude_completion() -> serde_json::Value {
        json!({
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 1 }
        })
    }

    /// `TEST-SETUP-CFG-001` (claude-api lane) — the probe verifies the Claude
    /// API family through its native adapter: a real inference, the probe
    /// result intact, nothing persisted.
    #[test]
    fn test_connection_probes_the_claude_api_family() {
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&claude_completion()),
        )]);
        let document = claude_document(&stub.base);
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(probe.model, "claude-opus-4-1");
        assert_eq!(probe.input_tokens, 3);
        assert_eq!(probe.output_tokens, 1);
        assert!(probe.latency_ms > 0);
        assert_eq!(stub.hits(), 1, "one-token inference, no discovery");
    }

    /// The family's own slot key decides the lane: the typed value under
    /// `connection/inferenceAnthropicApiKey` is the one sent, and a gateway-
    /// slot typed value alone is not a Claude API credential.
    #[test]
    fn claude_api_probe_uses_the_family_slot() {
        // The draft slot carries a DECOY handle with no stored secret; only
        // the typed family value can succeed — proving it is the one used.
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&claude_completion()),
        )]);
        let mut document = claude_document(&stub.base);
        document.connection.0.as_object_mut().unwrap().insert(
            "inferenceAnthropicApiKey".to_string(),
            json!({ "kind": "keychain", "service": "probe", "account": "decoy/account" }),
        );
        let (backend, _id) = backend();
        let mut secrets = BTreeMap::new();
        secrets.insert(
            "connection/inferenceAnthropicApiKey".to_string(),
            "sk-ant-EXAMPLE-TYPED".to_string(),
        );
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets,
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok, "the typed family value must be the one used");
        assert_eq!(stub.hits(), 1);

        // A gateway slot alone is not this family's credential: no Claude
        // API credential resolves, nothing sends.
        let (fresh, _id) = backend_again();
        let mut document = {
            let mut document = claude_document("https://gateway.example");
            document
                .connection
                .0
                .as_object_mut()
                .unwrap()
                .remove("inferenceAnthropicApiKey");
            document
        };
        document.connection.0.as_object_mut().unwrap().insert(
            "inferenceGatewayApiKey".to_string(),
            json!("sk-ant-EXAMPLE-GATEWAY-ONLY"),
        );
        let response = dispatch(
            &fresh,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("a credential is required"));
    }

    /// The Claude API family's offered kinds are static and interactive only
    /// (`docs/12` §1): a helper-script draft is refused before any request.
    #[test]
    fn claude_api_probe_refuses_helper_and_interactive_without_a_request() {
        // Helper: not offered for this family.
        let stub = StubGateway::serve(Vec::new());
        let mut document = claude_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        connection.insert(
            "inferenceCredentialKind".to_string(),
            json!("helper-script"),
        );
        connection.insert("inferenceCredentialHelper".to_string(), json!("/bin/echo"));
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(
            error.message.contains("helper scripts are not offered"),
            "the refusal names the family's kinds: {error:?}"
        );
        assert_eq!(stub.hits(), 0);

        // Interactive: the family offers the sign-in, but the probe cannot
        // complete one, and nothing is emulated (`docs/14` §9).
        let mut document = claude_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .insert("inferenceCredentialKind".to_string(), json!("interactive"));
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("interactive sign-in"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }

    /// The Claude API probe's discovery fallback: an empty model list runs
    /// the native listing first and infers against a policy-legal row.
    #[test]
    fn claude_api_probe_discovers_when_the_draft_has_no_models() {
        let listing = json!({
            "data": [
                { "id": "claude-opus-4-1", "display_name": "Opus 4.1" }
            ],
            "has_more": false,
        });
        let stub = StubGateway::serve(vec![
            canned(200, "application/json", &response_bytes(&listing)),
            canned(
                200,
                "application/json",
                &response_bytes(&claude_completion()),
            ),
        ]);
        let mut document = claude_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceModels");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok, "discovery picked the policy-legal model");
        assert_eq!(probe.model, "claude-opus-4-1");
        assert_eq!(stub.hits(), 2, "one listing, one inference — nothing else");
    }

    // -----------------------------------------------------------------------
    // The Bedrock family's probe lane (`docs/09` §2.2 "Bedrock"; Phase 3
    // slice 3)
    // -----------------------------------------------------------------------

    /// A Bedrock draft over a live stub, pointed at the family's optional
    /// base override (one override answers both the runtime and the
    /// control-plane listing — `docs/14` §9) with the required region set.
    fn bedrock_document(stub: &str) -> ProfileDocument {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.provider = Some(ProviderKind::Bedrock);
        document.connection = myharness_config::Section(json!({
            "inferenceBedrockRegion": "us-east-1",
            "inferenceBedrockBaseUrl": stub,
            "inferenceBedrockBearerToken": "BEDROCK-BEARER-EXAMPLE",
            "inferenceModels": ["anthropic.claude-sonnet-4-5"],
        }));
        document
    }

    /// A Bedrock invoke answer: the native one-token probe response. The
    /// model field is carrier-dependent on this family (`docs/14` §9), so
    /// the fixture omits it — the probe reports the requested id.
    fn bedrock_completion() -> serde_json::Value {
        json!({
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        })
    }

    /// The live probe: one-token inference against the drafted model, no
    /// discovery when the draft lists a legal id.
    #[test]
    fn test_connection_probes_the_bedrock_family() {
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&bedrock_completion()),
        )]);
        let document = bedrock_document(&stub.base);
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(probe.model, "anthropic.claude-sonnet-4-5");
        assert_eq!(probe.input_tokens, 7);
        assert_eq!(probe.output_tokens, 12);
        assert_eq!(probe.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(stub.hits(), 1, "one-token inference, no discovery");
    }

    /// The region is the family's routing key: absent, no endpoint can be
    /// formed and no request travels.
    #[test]
    fn test_connection_bedrock_requires_a_region() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = bedrock_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceBedrockRegion");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("region"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }

    /// The family's identity kinds — profile/SSO, web identity, vendor
    /// profile — are refused before anything sends: their sign-in flows were
    /// never captured, so they are never emulated (`docs/14` §9).
    #[test]
    fn test_connection_bedrock_refuses_the_identity_kinds() {
        for kind in ["sso", "external-idp", "vendor-profile"] {
            let stub = StubGateway::serve(Vec::new());
            let mut document = bedrock_document(&stub.base);
            document
                .connection
                .0
                .as_object_mut()
                .unwrap()
                .insert("inferenceCredentialKind".to_string(), json!(kind));
            let (backend, _id) = backend();
            let response = dispatch(
                &backend,
                SetupCommand::TestConnection {
                    document: serde_json::to_value(document).unwrap(),
                    secrets: BTreeMap::new(),
                },
            );
            let SetupResponse::Error { error } = response else {
                panic!("expected an error response for {kind:?}");
            };
            assert_eq!(error.code, "CONFIG_VALIDATION", "{kind:?}");
            assert!(
                error.message.contains("cannot be probed"),
                "{kind:?}: {}",
                error.message
            );
            assert_eq!(stub.hits(), 0, "{kind:?}: nothing sent");
        }
    }

    /// The discovery fallback: an empty model list runs the control plane's
    /// model APIs first and probes against a policy-legal row.
    #[test]
    fn test_connection_bedrock_discovers_when_the_draft_has_no_models() {
        let listing = json!({
            "modelSummaries": [
                { "modelId": "cohere.command-r-v1:0", "modelName": "not ours" },
                { "modelId": "anthropic.claude-sonnet-4-5", "modelName": "Sonnet 4.5" }
            ]
        });
        let stub = StubGateway::serve(vec![
            canned(200, "application/json", &response_bytes(&listing)),
            canned(
                200,
                "application/json",
                &response_bytes(&bedrock_completion()),
            ),
        ]);
        let mut document = bedrock_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceModels");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok, "discovery picked the policy-legal model");
        assert_eq!(probe.model, "anthropic.claude-sonnet-4-5");
        assert_eq!(stub.hits(), 2, "one listing, one inference — nothing else");
    }

    /// The helper-script kind is offered for this family and runs once; its
    /// output is never persisted (`docs/09` §2.2; `docs/14` §9).
    #[test]
    fn test_connection_bedrock_runs_the_helper_kind() {
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&bedrock_completion()),
        )]);
        let mut document = bedrock_document(&stub.base);
        let connection = document.connection.0.as_object_mut().unwrap();
        connection.insert(
            "inferenceCredentialKind".to_string(),
            json!("helper-script"),
        );
        connection.insert("inferenceCredentialHelper".to_string(), json!("/bin/echo"));
        connection.insert(
            "inferenceCredentialHelperArgs".to_string(),
            json!("BEDROCK-HELPER-OUTPUT"),
        );
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(stub.hits(), 1, "one-token inference, no discovery");
    }

    // ---------------------------------------------------------------------------
    // The Vertex family's probe lane (`docs/09` §2.2 "Vertex AI"; Phase 3
    // slice 4)
    // ---------------------------------------------------------------------------

    /// A Vertex draft over a live stub, pointed at the family's optional
    /// base override (`inferenceVertexBaseUrl` replaces the region-derived
    /// host in full — `docs/14` §9 item 28; the tests never let the
    /// region-derived host reach the internet), with the required region,
    /// project, credential kind, and a `claude-` model listed (no
    /// discovery exists for this family).
    fn vertex_document(stub: &str) -> ProfileDocument {
        let (backend, id) = backend();
        let mut document = backend.library().read(&id).unwrap();
        document.provider = Some(ProviderKind::Vertex);
        document.connection = myharness_config::Section(json!({
            "inferenceVertexRegion": "us-east5",
            "inferenceVertexProjectId": "synthetic-project",
            "inferenceVertexBaseUrl": stub,
            "inferenceCredentialKind": "helper-script",
            "inferenceCredentialHelper": "/bin/echo",
            "inferenceCredentialHelperArgs": "VERTEX-HELPER-OUTPUT",
            "inferenceModels": ["claude-sonnet-4-5"],
        }));
        document
    }

    /// A Vertex raw-prediction answer: the native one-token probe response.
    /// The `model` field is optional on this family (`docs/14` §9 item 28's
    /// conditional echo), so the fixture omits it — the probe reports the
    /// requested id.
    fn vertex_completion() -> serde_json::Value {
        json!({
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        })
    }

    /// The live probe: one-token `rawPredict` against the drafted model.
    /// No discovery travels because none exists for this family; the
    /// helper-script credential is the only kind this lane resolves.
    #[test]
    fn test_connection_probes_the_vertex_family() {
        let stub = StubGateway::serve(vec![canned(
            200,
            "application/json",
            &response_bytes(&vertex_completion()),
        )]);
        let document = vertex_document(&stub.base);
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Probe { probe } = response else {
            panic!("expected a probe response");
        };
        assert!(probe.ok);
        assert_eq!(probe.model, "claude-sonnet-4-5");
        assert_eq!(probe.input_tokens, 7);
        assert_eq!(probe.output_tokens, 12);
        assert_eq!(probe.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(stub.hits(), 1, "one-token inference; no discovery exists");
    }

    /// The region forms the host: absent, no request travels.
    #[test]
    fn test_connection_vertex_requires_a_region() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = vertex_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceVertexRegion");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("region"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }

    /// The project forms the request path: absent, no request travels.
    #[test]
    fn test_connection_vertex_requires_a_project() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = vertex_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceVertexProjectId");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("project"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }

    /// The family's identity kinds — Google sign-in, Workforce, web/external
    /// identity, vendor profile — are refused before anything sends: their
    /// sign-in flows were never captured, so they are never emulated
    /// (`docs/14` §9 item 28). The helper script must never run for them
    /// either.
    #[test]
    fn test_connection_vertex_refuses_the_identity_kinds() {
        for kind in ["interactive", "workforce", "external-idp", "vendor-profile"] {
            let stub = StubGateway::serve(Vec::new());
            let mut document = vertex_document(&stub.base);
            document
                .connection
                .0
                .as_object_mut()
                .unwrap()
                .insert("inferenceCredentialKind".to_string(), json!(kind));
            let (backend, _id) = backend();
            let response = dispatch(
                &backend,
                SetupCommand::TestConnection {
                    document: serde_json::to_value(document).unwrap(),
                    secrets: BTreeMap::new(),
                },
            );
            let SetupResponse::Error { error } = response else {
                panic!("expected an error response for {kind:?}");
            };
            assert_eq!(error.code, "CONFIG_VALIDATION", "{kind:?}");
            assert!(
                error.message.contains("cannot be probed"),
                "{kind:?}: {}",
                error.message
            );
            assert_eq!(stub.hits(), 0, "{kind:?}: nothing sent");
        }
    }

    /// A draft that names no credential kind is refused: the lane never
    /// guesses a source (`docs/09` §2.2 "when set, only that source is
    /// used"; fail-closed discipline).
    #[test]
    fn test_connection_vertex_names_no_kind() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = vertex_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceCredentialKind");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "CONFIG_VALIDATION");
        assert!(error.message.contains("credential kind"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }

    /// No discovery exists for this family: a draft without a `claude-`
    /// model id is refused before the helper even runs and before anything
    /// sends (`docs/09` §2.2 "Vertex AI" disables discovery for every kind
    /// the family offers; `docs/14` §9 item 28).
    #[test]
    fn test_connection_vertex_needs_a_drafted_model() {
        let stub = StubGateway::serve(Vec::new());
        let mut document = vertex_document(&stub.base);
        document
            .connection
            .0
            .as_object_mut()
            .unwrap()
            .remove("inferenceModels");
        let (backend, _id) = backend();
        let response = dispatch(
            &backend,
            SetupCommand::TestConnection {
                document: serde_json::to_value(document).unwrap(),
                secrets: BTreeMap::new(),
            },
        );
        let SetupResponse::Error { error } = response else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, "PROVIDER_ERROR");
        assert!(error.message.contains("no claude- model id"));
        assert_eq!(stub.hits(), 0, "nothing sent");
    }
}
