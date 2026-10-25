//! MCP OAuth token storage and refresh — roadmap Phase 5 (`docs/12` §5.1;
//! `docs/features/mcp-skills-tools.md` §4).
//!
//! What is verified from the docs: tokens are **per server**, owned by the
//! main process/keychain, and `forgetMcpOAuth` clears one server's token. The
//! shipped authorization-code/refresh wire flow was not captured, so this
//! module implements only the pieces the rebuild can state honestly:
//!
//! - a typed token record and a per-server store;
//! - a refresh primitive behind an injected [`OAuthHttp`] (the production
//!   implementation lives at the bridge boundary);
//! - a fail-closed authorization header provider: no token and no refresh
//!   token means `NeedsAuth`, never an anonymous request.
//!
//! The refresh request shape (`grant_type=refresh_token`, form encoding) is a
//! rebuild-defined OAuth 2.0 choice, not a claim about the shipped client. The
//! authorization-code/PKCE browser flow remains unobserved and is not
//! emulated; a server that needs it surfaces `NeedsAuth`.
//!
//! Tokens are secrets: `Debug` redacts both token values, and errors never
//! include them.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

/// OAuth client configuration for one MCP server.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    /// Token endpoint. Must be `http` or `https`.
    pub token_url: String,
    /// Public client id.
    pub client_id: String,
    /// Optional requested scopes (sent only by a refresh implementation that
    /// needs them; the refresh grant usually inherits the original scopes).
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl fmt::Debug for OAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthConfig")
            .field("token_url", &self.token_url)
            .field("client_id", &self.client_id)
            .field("scopes", &self.scopes)
            .finish()
    }
}

impl OAuthConfig {
    /// Validate the shape before use. The URL is checked here, not in the
    /// HTTP client, so a bad managed config fails closed before any request.
    pub fn validate(&self) -> Result<(), OAuthError> {
        let parsed = Url::parse(&self.token_url)
            .map_err(|error| OAuthError::InvalidConfig(format!("invalid token URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(OAuthError::InvalidConfig(
                "the token URL scheme must be http or https".to_string(),
            ));
        }
        if parsed.host_str().is_none() {
            return Err(OAuthError::InvalidConfig(
                "the token URL has no host".to_string(),
            ));
        }
        if self.client_id.trim().is_empty() {
            return Err(OAuthError::InvalidConfig(
                "the OAuth client id is empty".to_string(),
            ));
        }
        for scope in &self.scopes {
            if scope.contains('\0') {
                return Err(OAuthError::InvalidConfig(
                    "an OAuth scope contains a NUL byte".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// A stored token. `expires_at` is seconds since the Unix epoch; `None` means
/// the server did not state an expiry (the token is used until it fails).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthToken {
    /// Bearer access token.
    pub access_token: String,
    /// Optional refresh token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Token type; defaults to `Bearer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// Expiry, in seconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl fmt::Debug for OAuthToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthToken")
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[redacted]"),
            )
            .field("token_type", &self.token_type)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl OAuthToken {
    /// A token with no stated expiry.
    pub fn bearer(access_token: impl Into<String>) -> OAuthToken {
        OAuthToken {
            access_token: access_token.into(),
            refresh_token: None,
            token_type: None,
            expires_at: None,
        }
    }

    fn is_expired(&self, now: u64, skew_seconds: u64) -> bool {
        self.expires_at
            .is_some_and(|expires| expires <= now.saturating_add(skew_seconds))
    }

    fn header_value(&self) -> Result<String, OAuthError> {
        if self.access_token.is_empty() {
            return Err(OAuthError::InvalidToken(
                "the stored access token is empty".to_string(),
            ));
        }
        if self.access_token.contains(['\r', '\n', '\0']) {
            return Err(OAuthError::InvalidToken(
                "the stored access token contains an unsafe character".to_string(),
            ));
        }
        let kind = self.token_type.as_deref().unwrap_or("Bearer");
        if kind.contains(['\r', '\n', '\0']) || kind.is_empty() {
            return Err(OAuthError::InvalidToken(
                "the stored token type is unsafe".to_string(),
            ));
        }
        Ok(format!("{kind} {}", self.access_token))
    }
}

/// Per-server token storage. The production implementation is keychain-backed
/// at the bridge; tests use [`MemoryOAuthStore`].
pub trait OAuthStore: Send + Sync {
    fn get(&self, server: &str) -> Result<Option<OAuthToken>, OAuthError>;
    fn set(&self, server: &str, token: &OAuthToken) -> Result<(), OAuthError>;
    fn forget(&self, server: &str) -> Result<(), OAuthError>;
}

/// In-memory store for tests and headless runs.
#[derive(Debug, Default)]
pub struct MemoryOAuthStore {
    tokens: std::sync::Mutex<BTreeMap<String, OAuthToken>>,
}

impl OAuthStore for MemoryOAuthStore {
    fn get(&self, server: &str) -> Result<Option<OAuthToken>, OAuthError> {
        Ok(self
            .tokens
            .lock()
            .map_err(|_| OAuthError::Store("the OAuth store lock was poisoned".to_string()))?
            .get(server)
            .cloned())
    }

    fn set(&self, server: &str, token: &OAuthToken) -> Result<(), OAuthError> {
        self.tokens
            .lock()
            .map_err(|_| OAuthError::Store("the OAuth store lock was poisoned".to_string()))?
            .insert(server.to_string(), token.clone());
        Ok(())
    }

    fn forget(&self, server: &str) -> Result<(), OAuthError> {
        self.tokens
            .lock()
            .map_err(|_| OAuthError::Store("the OAuth store lock was poisoned".to_string()))?
            .remove(server);
        Ok(())
    }
}

/// The HTTP operation the OAuth manager needs. Kept as a trait so the refresh
/// wire is testable without a live authorization server and so the production
/// client stays at the bridge boundary.
#[async_trait]
pub trait OAuthHttp: Send + Sync {
    /// Exchange a refresh token for a new token.
    async fn refresh(
        &self,
        config: &OAuthConfig,
        refresh_token: &str,
    ) -> Result<OAuthToken, OAuthError>;
}

/// OAuth failures. Messages never carry token material.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OAuthError {
    #[error("the OAuth configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("the OAuth store failed: {0}")]
    Store(String),
    #[error("the OAuth token is invalid: {0}")]
    InvalidToken(String),
    #[error("OAuth authorization is required for {server}")]
    NeedsAuth { server: String },
    #[error("OAuth refresh failed for {server}: {message}")]
    Refresh { server: String, message: String },
}

/// The narrower error the MCP manager consumes. `NeedsAuth` is a distinct
/// server state; everything else is a normal startup failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthError {
    #[error("OAuth authorization is required for {server}")]
    NeedsAuth { server: String, message: String },
    #[error("OAuth failed for {server}: {message}")]
    Failed { server: String, message: String },
}

/// Supplies an authorization header for an HTTP/SSE server.
#[async_trait]
pub trait McpAuthProvider: Send + Sync {
    async fn authorization_header(
        &self,
        server: &str,
    ) -> Result<Option<(String, String)>, AuthError>;
}

/// Per-server OAuth manager with a single-flight refresh lock.
pub struct OAuthManager {
    configs: BTreeMap<String, OAuthConfig>,
    store: Arc<dyn OAuthStore>,
    http: Arc<dyn OAuthHttp>,
    refresh_lock: tokio::sync::Mutex<()>,
    skew_seconds: u64,
}

impl fmt::Debug for OAuthManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthManager")
            .field("servers", &self.configs.keys().collect::<Vec<_>>())
            .field("skew_seconds", &self.skew_seconds)
            .finish()
    }
}

impl OAuthManager {
    pub fn new(store: Arc<dyn OAuthStore>, http: Arc<dyn OAuthHttp>) -> OAuthManager {
        OAuthManager {
            configs: BTreeMap::new(),
            store,
            http,
            refresh_lock: tokio::sync::Mutex::new(()),
            skew_seconds: 60,
        }
    }

    /// Add or replace one server's OAuth configuration.
    pub fn with_config(
        mut self,
        server: impl Into<String>,
        config: OAuthConfig,
    ) -> Result<OAuthManager, OAuthError> {
        config.validate()?;
        self.configs.insert(server.into(), config);
        Ok(self)
    }

    /// Refresh this many seconds before expiry (default 60).
    pub fn with_skew_seconds(mut self, skew_seconds: u64) -> OAuthManager {
        self.skew_seconds = skew_seconds;
        self
    }

    /// True when the server has an OAuth configuration.
    pub fn has_config(&self, server: &str) -> bool {
        self.configs.contains_key(server)
    }

    /// Forget one server's token. Other servers are untouched.
    pub fn forget(&self, server: &str) -> Result<(), OAuthError> {
        self.store.forget(server)
    }

    /// Current token, without refreshing.
    pub fn stored_token(&self, server: &str) -> Result<Option<OAuthToken>, OAuthError> {
        self.store.get(server)
    }

    /// Resolve the authorization header at a caller-supplied time.
    pub async fn authorization_header_at(
        &self,
        server: &str,
        now: u64,
    ) -> Result<Option<(String, String)>, OAuthError> {
        let Some(config) = self.configs.get(server) else {
            return Ok(None);
        };
        let _guard = self.refresh_lock.lock().await;
        let mut token = self.store.get(server)?;
        if let Some(current) = token.as_ref() {
            if !current.is_expired(now, self.skew_seconds) {
                return Ok(Some(("Authorization".to_string(), current.header_value()?)));
            }
        }
        let refresh_token = token
            .as_ref()
            .and_then(|current| current.refresh_token.clone())
            .ok_or_else(|| OAuthError::NeedsAuth {
                server: server.to_string(),
            })?;
        let refreshed = self
            .http
            .refresh(config, &refresh_token)
            .await
            .map_err(|error| {
                // Do not wrap a `NeedsAuth` from the client into a refresh failure:
                // the manager's caller needs the distinct state.
                match error {
                    OAuthError::NeedsAuth { .. } => OAuthError::NeedsAuth {
                        server: server.to_string(),
                    },
                    other => OAuthError::Refresh {
                        server: server.to_string(),
                        message: other.to_string(),
                    },
                }
            })?;
        let refreshed = if refreshed.refresh_token.is_none() {
            OAuthToken {
                refresh_token: Some(refresh_token),
                ..refreshed
            }
        } else {
            refreshed
        };
        self.store.set(server, &refreshed)?;
        token = Some(refreshed);
        let token = token.expect("the refreshed token was just stored");
        Ok(Some(("Authorization".to_string(), token.header_value()?)))
    }

    /// Resolve the authorization header using the system clock.
    pub async fn authorization_header(
        &self,
        server: &str,
    ) -> Result<Option<(String, String)>, OAuthError> {
        self.authorization_header_at(server, unix_now()).await
    }
}

#[async_trait]
impl McpAuthProvider for OAuthManager {
    async fn authorization_header(
        &self,
        server: &str,
    ) -> Result<Option<(String, String)>, AuthError> {
        OAuthManager::authorization_header(self, server)
            .await
            .map_err(|error| match error {
                OAuthError::NeedsAuth { .. } => AuthError::NeedsAuth {
                    server: server.to_string(),
                    message: "OAuth authorization is required".to_string(),
                },
                other => AuthError::Failed {
                    server: server.to_string(),
                    message: other.to_string(),
                },
            })
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct MockHttp {
        calls: AtomicUsize,
        token: std::sync::Mutex<Option<OAuthToken>>,
        error: std::sync::Mutex<Option<OAuthError>>,
    }

    #[async_trait]
    impl OAuthHttp for MockHttp {
        async fn refresh(
            &self,
            _config: &OAuthConfig,
            _refresh_token: &str,
        ) -> Result<OAuthToken, OAuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = self.error.lock().unwrap().clone() {
                return Err(error);
            }
            Ok(self
                .token
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| OAuthToken::bearer("new-access")))
        }
    }

    fn config() -> OAuthConfig {
        OAuthConfig {
            token_url: "https://auth.example.test/token".into(),
            client_id: "client".into(),
            scopes: Vec::new(),
        }
    }

    fn store_with(server: &str, token: OAuthToken) -> Arc<MemoryOAuthStore> {
        let store = Arc::new(MemoryOAuthStore::default());
        store.set(server, &token).unwrap();
        store
    }

    #[tokio::test]
    async fn a_valid_token_is_used_without_refreshing() {
        let store = store_with(
            "github",
            OAuthToken {
                access_token: "access".into(),
                refresh_token: Some("refresh".into()),
                token_type: None,
                expires_at: Some(10_000),
            },
        );
        let http = Arc::new(MockHttp::default());
        let manager = OAuthManager::new(store, http.clone())
            .with_config("github", config())
            .unwrap();
        let header = manager
            .authorization_header_at("github", 1_000)
            .await
            .unwrap();
        assert_eq!(
            header,
            Some(("Authorization".into(), "Bearer access".into()))
        );
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_expired_token_is_refreshed_and_stored() {
        let store = store_with(
            "github",
            OAuthToken {
                access_token: "old".into(),
                refresh_token: Some("refresh".into()),
                token_type: None,
                expires_at: Some(100),
            },
        );
        let http = Arc::new(MockHttp::default());
        let manager = OAuthManager::new(store.clone(), http.clone())
            .with_config("github", config())
            .unwrap();
        let header = manager
            .authorization_header_at("github", 1_000)
            .await
            .unwrap();
        assert_eq!(
            header,
            Some(("Authorization".into(), "Bearer new-access".into()))
        );
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            store.get("github").unwrap().unwrap().access_token,
            "new-access"
        );
    }

    #[tokio::test]
    async fn refresh_is_isolated_per_server() {
        let store = Arc::new(MemoryOAuthStore::default());
        store
            .set(
                "one",
                &OAuthToken {
                    access_token: "one".into(),
                    refresh_token: Some("one-refresh".into()),
                    token_type: None,
                    expires_at: Some(100),
                },
            )
            .unwrap();
        store
            .set(
                "two",
                &OAuthToken {
                    access_token: "two".into(),
                    refresh_token: Some("two-refresh".into()),
                    token_type: None,
                    expires_at: Some(10_000),
                },
            )
            .unwrap();
        let http = Arc::new(MockHttp::default());
        let manager = OAuthManager::new(store.clone(), http.clone())
            .with_config("one", config())
            .unwrap()
            .with_config("two", config())
            .unwrap();
        manager.authorization_header_at("one", 1_000).await.unwrap();
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        assert_eq!(store.get("two").unwrap().unwrap().access_token, "two");
    }

    #[tokio::test]
    async fn no_refresh_token_means_needs_auth() {
        let store = store_with(
            "github",
            OAuthToken {
                access_token: "old".into(),
                refresh_token: None,
                token_type: None,
                expires_at: Some(100),
            },
        );
        let manager = OAuthManager::new(store, Arc::new(MockHttp::default()))
            .with_config("github", config())
            .unwrap();
        assert!(matches!(
            manager.authorization_header_at("github", 1_000).await,
            Err(OAuthError::NeedsAuth { .. })
        ));
    }

    #[tokio::test]
    async fn forgetting_one_server_leaves_the_other_token() {
        let store = Arc::new(MemoryOAuthStore::default());
        store.set("one", &OAuthToken::bearer("one")).unwrap();
        store.set("two", &OAuthToken::bearer("two")).unwrap();
        let manager = OAuthManager::new(store.clone(), Arc::new(MockHttp::default()));
        manager.forget("one").unwrap();
        assert!(store.get("one").unwrap().is_none());
        assert_eq!(store.get("two").unwrap().unwrap().access_token, "two");
    }

    #[test]
    fn debug_never_prints_token_values() {
        let token = OAuthToken {
            access_token: "super-secret".into(),
            refresh_token: Some("refresh-secret".into()),
            token_type: None,
            expires_at: None,
        };
        let debug = format!("{token:?}");
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("refresh-secret"));
    }
}
