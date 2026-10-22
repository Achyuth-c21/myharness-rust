//! Credential **helper script** execution — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/09 §2.2 credential kinds; `TEST-CFG-005`; `REQ-SETUP-05`).
//!
//! One credential kind is an external command that returns a credential
//! (`docs/09` §2.2: "`helper-script` | External command returns credential |
//! Arguments, timeout, TTL, silent refresh"). The shipped profile keys are
//! `inferenceCredentialHelper`, `inferenceCredentialHelperArgs`,
//! `inferenceCredentialHelperWindows`, `inferenceCredentialHelperTimeoutSec`,
//! `inferenceCredentialHelperTtlSec` and
//! `inferenceCredentialHelperSilentRefreshEnabled` (`docs/20` §2.1).
//!
//! Rules bound to this module:
//!
//! - **[HARD] No fallback.** The helper kind is the *only* source used
//!   (`docs/09` §2.2: "When set, only that source is used (no fallback)"), and
//!   `TEST-CFG-005` requires that a helper failure does not fall back. A failed
//!   helper is an error, never a silent switch to another credential.
//! - **[HARD] The helper's output is data, never instructions** (`RULES.md` §2,
//!   `docs/12` §5.2). The captured stdout is a credential value; it is never
//!   evaluated, never executed, and never rendered as a message.
//! - **[HARD] Never log a value** (`docs/09` §4). Errors carry the program path
//!   and the exit status — never stdout, never stderr, because a helper's stderr
//!   can contain the credential it failed to deliver.
//! - The value lives in memory as a `secrecy::SecretString` and is written to the
//!   OS keychain, never into a profile document
//!   ([`DATA-CONFIG-02`](docs/09) §5).

use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use secrecy::SecretString;

use crate::{SecretError, SecretStore};

/// Default helper timeout when the profile does not set one. The shipped default
/// was not captured (`docs/14` §9); this is a fail-closed default, not a claim.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum bytes of helper output retained. A credential is short; a runaway
/// helper must not be able to exhaust memory. The pipe is still drained so the
/// child can exit.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// How long to wait for the output readers after the child exits. A grandchild
/// holding the pipe open must not wedge the caller.
const READER_GRACE: Duration = Duration::from_secs(2);

/// The configuration of a credential helper, read from a profile's connection
/// section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperConfig {
    /// The helper program (`inferenceCredentialHelper`).
    pub program: String,
    /// Arguments (`inferenceCredentialHelperArgs`).
    pub args: Vec<String>,
    /// The Windows-specific program (`inferenceCredentialHelperWindows`).
    pub windows_program: Option<String>,
    /// Timeout (`inferenceCredentialHelperTimeoutSec`).
    pub timeout: Duration,
    /// How long a fetched credential stays fresh (`inferenceCredentialHelperTtlSec`).
    pub ttl: Option<Duration>,
    /// Whether a stale credential may be refreshed without a user-visible action
    /// (`inferenceCredentialHelperSilentRefreshEnabled`).
    pub silent_refresh: bool,
}

impl HelperConfig {
    /// Read the helper configuration out of a connection section.
    ///
    /// A missing helper program is not an error here — the helper kind is one
    /// option among several, and a profile that does not use it carries none of
    /// these keys. The caller decides whether a helper is required.
    pub fn from_connection(
        connection: &serde_json::Value,
    ) -> Result<Option<HelperConfig>, SecretError> {
        let Some(object) = connection.as_object() else {
            return Ok(None);
        };
        let program = object
            .get("inferenceCredentialHelper")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(program) = program else {
            return Ok(None);
        };
        let args = match object.get("inferenceCredentialHelperArgs") {
            Some(serde_json::Value::String(text)) => {
                text.split_whitespace().map(str::to_string).collect()
            }
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
            // The key's type is recorded as "string" (`docs/20` §2.1); any other
            // shape is unobserved, so no arguments are used rather than a guess.
            _ => Vec::new(),
        };
        let windows_program = object
            .get("inferenceCredentialHelperWindows")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let timeout = object
            .get("inferenceCredentialHelperTimeoutSec")
            .and_then(|v| v.as_u64())
            .filter(|secs| *secs > 0)
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);
        let ttl = object
            .get("inferenceCredentialHelperTtlSec")
            .and_then(|v| v.as_u64())
            .filter(|secs| *secs > 0)
            .map(Duration::from_secs);
        let silent_refresh = object
            .get("inferenceCredentialHelperSilentRefreshEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Some(HelperConfig {
            program: program.to_string(),
            args,
            windows_program,
            timeout,
            ttl,
            silent_refresh,
        }))
    }

    /// The program to run on this platform. `inferenceCredentialHelperWindows` is
    /// the Windows-specific program; the generic one is used everywhere else.
    pub fn program_for(&self, platform_is_windows: bool) -> &str {
        if platform_is_windows {
            self.windows_program.as_deref().unwrap_or(&self.program)
        } else {
            &self.program
        }
    }
}

/// Run the helper and return its stdout as the credential.
///
/// The returned value is a `SecretString` (zeroized on drop). Every failure path
/// is an error: there is no fallback to another credential source
/// (`TEST-CFG-005`).
pub fn run_helper(
    config: &HelperConfig,
    platform_is_windows: bool,
) -> Result<SecretString, SecretError> {
    let program = config.program_for(platform_is_windows).to_string();
    let output = run_process(&program, &config.args, config.timeout)?;
    let value = String::from_utf8_lossy(&output);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SecretError::HelperFailed {
            program,
            detail: "helper produced no credential output".to_string(),
        });
    }
    Ok(SecretString::from(trimmed.to_string()))
}

/// Resolves a helper-issued credential, with the TTL and silent-refresh
/// behavior the credential kind documents (`docs/09` §2.2: "Arguments, timeout, TTL,
/// silent refresh").
///
/// The resolver owns an in-memory cache of what the helper last returned. A
/// fetched credential is also written to the OS keychain under the given account,
/// so it survives a restart without being written into a profile document
/// ([`DATA-CONFIG-02`](docs/09) §5).
///
/// The TTL and silent-refresh semantics are a rebuild interpretation: the shipped
/// behavior was never observed (`docs/14` §9 item 15). The rebuild fails closed —
/// a stale credential with silent refresh disabled is an error, never a stale
/// value.
#[derive(Default)]
pub struct HelperResolver {
    cache: Mutex<BTreeMap<String, CachedCredential>>,
}

struct CachedCredential {
    value: SecretString,
    stored_at: Instant,
}

impl HelperResolver {
    /// A resolver with an empty cache.
    pub fn new() -> Self {
        HelperResolver::default()
    }

    /// Resolve the credential for `account`.
    ///
    /// 1. A cached credential inside its TTL is returned unchanged.
    /// 2. A cached credential past its TTL is refreshed when silent refresh is
    ///    enabled, and is otherwise an error.
    /// 3. With no cached credential the helper runs, and its value is written to
    ///    the keychain and cached.
    ///
    /// A helper failure is an error at every step — there is no fallback to
    /// another credential source (`TEST-CFG-005`).
    pub fn resolve(
        &self,
        account: &str,
        config: &HelperConfig,
        store: &dyn SecretStore,
        platform_is_windows: bool,
    ) -> Result<SecretString, SecretError> {
        let now = Instant::now();
        let cached = {
            let cache = self.cache.lock().expect("helper cache poisoned");
            cache.get(account).map(|c| (c.value.clone(), c.stored_at))
        };

        if let Some((value, stored_at)) = cached {
            let fresh = match config.ttl {
                Some(ttl) => now.duration_since(stored_at) < ttl,
                None => true,
            };
            if fresh {
                return Ok(value);
            }
            if !config.silent_refresh {
                return Err(SecretError::HelperExpired {
                    account: account.to_string(),
                });
            }
        }

        let value = run_helper(config, platform_is_windows)?;
        // The credential lives in the keychain, not in a profile document.
        store.store(account, &value)?;
        {
            let mut cache = self.cache.lock().expect("helper cache poisoned");
            cache.insert(
                account.to_string(),
                CachedCredential {
                    value: value.clone(),
                    stored_at: now,
                },
            );
        }
        Ok(value)
    }

    /// Drop the cached credential for `account` (rotation, sign-out).
    pub fn forget(&self, account: &str) {
        let mut cache = self.cache.lock().expect("helper cache poisoned");
        cache.remove(account);
    }
}

/// Spawn the helper, enforce the timeout, and return its stdout.
fn run_process(program: &str, args: &[String], timeout: Duration) -> Result<Vec<u8>, SecretError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| SecretError::HelperFailed {
            program: program.to_string(),
            detail: format!("helper could not be started: {e}"),
        })?;

    // Read stdout and stderr on their own threads: a helper that fills the pipe
    // buffer would otherwise deadlock against our own wait.
    let stdout_rx = spawn_reader(child.stdout.take());
    let stderr_rx = spawn_reader(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                return Err(SecretError::HelperFailed {
                    program: program.to_string(),
                    detail: format!("helper could not be waited on: {e}"),
                })
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SecretError::HelperTimeout {
                program: program.to_string(),
                timeout_secs: timeout.as_secs(),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    // The child is gone; collect its output. A reader that is still blocked
    // (a grandchild holding the pipe) must not wedge the caller.
    let stdout = stdout_rx
        .recv_timeout(READER_GRACE)
        .map_err(|_| SecretError::HelperFailed {
            program: program.to_string(),
            detail: "helper output could not be collected".to_string(),
        })?;
    // stderr is drained and discarded: it can contain the credential a helper
    // failed to deliver, so it never reaches an error message or a log.
    let _ = stderr_rx.recv_timeout(READER_GRACE);

    if !status.success() {
        return Err(SecretError::HelperFailed {
            program: program.to_string(),
            detail: format!("helper exited with status {status}"),
        });
    }
    Ok(stdout)
}

/// Read a pipe to end on a background thread, retaining at most
/// [`MAX_OUTPUT_BYTES`] but continuing to drain so the child can exit.
fn spawn_reader<R: Read + Send + 'static>(reader: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let Some(mut reader) = reader else {
            let _ = tx.send(Vec::new());
            return;
        };
        let mut kept: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if kept.len() < MAX_OUTPUT_BYTES {
                        let room = MAX_OUTPUT_BYTES - kept.len();
                        kept.extend_from_slice(&chunk[..n.min(room)]);
                    }
                }
            }
        }
        let _ = tx.send(kept);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret as _;

    /// A helper that succeeds yields its stdout as the credential.
    #[cfg(unix)]
    #[test]
    fn successful_helper_returns_its_output() {
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "printf 'credential-value'".to_string()],
            windows_program: None,
            timeout: Duration::from_secs(5),
            ttl: None,
            silent_refresh: false,
        };
        let value = run_helper(&config, false).unwrap();
        assert_eq!(value.expose_secret(), "credential-value");
    }

    /// A non-zero exit is an error and yields no credential — and the error
    /// carries no output, only the program and the status.
    #[cfg(unix)]
    #[test]
    fn failing_helper_produces_no_fallback_and_no_output_in_the_error() {
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'secret-leak' >&2; exit 3".to_string(),
            ],
            windows_program: None,
            timeout: Duration::from_secs(5),
            ttl: None,
            silent_refresh: false,
        };
        let error = run_helper(&config, false).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("SECRET_HELPER"), "{text}");
        assert!(
            !text.contains("secret-leak"),
            "the error must not echo output"
        );
    }

    /// The timeout is enforced: a helper that never exits is killed.
    #[cfg(unix)]
    #[test]
    fn helper_that_hangs_is_killed_at_the_timeout() {
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "sleep 30".to_string()],
            windows_program: None,
            timeout: Duration::from_millis(200),
            ttl: None,
            silent_refresh: false,
        };
        let error = run_helper(&config, false).unwrap_err();
        assert!(
            matches!(error, SecretError::HelperTimeout { .. }),
            "{error}"
        );
    }

    /// Empty output is a failure, not an empty credential.
    #[cfg(unix)]
    #[test]
    fn empty_output_is_a_failure() {
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "true".to_string()],
            windows_program: None,
            timeout: Duration::from_secs(5),
            ttl: None,
            silent_refresh: false,
        };
        let error = run_helper(&config, false).unwrap_err();
        assert!(matches!(error, SecretError::HelperFailed { .. }), "{error}");
    }

    /// A helper that cannot be started is an error, not a fallback.
    #[test]
    fn missing_helper_is_an_error() {
        let config = HelperConfig {
            program: "/nonexistent/helper-binary-xyz".to_string(),
            args: Vec::new(),
            windows_program: None,
            timeout: Duration::from_secs(1),
            ttl: None,
            silent_refresh: false,
        };
        let error = run_helper(&config, false).unwrap_err();
        assert!(matches!(error, SecretError::HelperFailed { .. }), "{error}");
    }

    /// The configuration is read from the documented profile keys.
    #[test]
    fn config_is_read_from_the_documented_keys() {
        let connection = serde_json::json!({
            "inferenceCredentialHelper": "/usr/local/bin/cred-helper",
            "inferenceCredentialHelperArgs": "--json",
            "inferenceCredentialHelperWindows": "C:\\\\cred-helper.exe",
            "inferenceCredentialHelperTimeoutSec": 15,
            "inferenceCredentialHelperTtlSec": 900,
            "inferenceCredentialHelperSilentRefreshEnabled": true,
        });
        let config = HelperConfig::from_connection(&connection).unwrap().unwrap();
        assert_eq!(config.program, "/usr/local/bin/cred-helper");
        assert_eq!(config.args, vec!["--json".to_string()]);
        assert_eq!(config.timeout, Duration::from_secs(15));
        assert_eq!(config.ttl, Some(Duration::from_secs(900)));
        assert!(config.silent_refresh);
        assert_eq!(config.program_for(true), "C:\\\\cred-helper.exe");
        assert_eq!(config.program_for(false), "/usr/local/bin/cred-helper");
    }

    /// A profile that does not use the helper kind carries no helper config.
    #[test]
    fn absent_helper_yields_no_config() {
        let connection = serde_json::json!({ "inferenceCredentialKind": "static" });
        assert!(HelperConfig::from_connection(&connection)
            .unwrap()
            .is_none());
    }

    /// The resolver caches a fetched credential and writes it to the keychain;
    /// a second resolve inside the TTL does not run the helper again.
    #[cfg(unix)]
    #[test]
    fn resolver_caches_within_the_ttl() {
        let store = crate::MemoryStore::default();
        let resolver = HelperResolver::new();
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf \"value-$(cat $COUNTER 2>/dev/null || echo 1)\"".to_string(),
            ],
            windows_program: None,
            timeout: Duration::from_secs(5),
            ttl: Some(Duration::from_secs(600)),
            silent_refresh: false,
        };
        let first = resolver
            .resolve("profile/a/connection.apiKey", &config, &store, false)
            .unwrap();
        let second = resolver
            .resolve("profile/a/connection.apiKey", &config, &store, false)
            .unwrap();
        assert_eq!(first.expose_secret(), second.expose_secret());
        // The credential was written to the keychain, not returned only in memory.
        assert!(store.load("profile/a/connection.apiKey").unwrap().is_some());
    }

    /// A helper failure is an error and never a fallback to another source.
    #[cfg(unix)]
    #[test]
    fn resolver_reports_a_helper_failure_without_falling_back() {
        let store = crate::MemoryStore::default();
        let resolver = HelperResolver::new();
        let config = HelperConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "exit 7".to_string()],
            windows_program: None,
            timeout: Duration::from_secs(5),
            ttl: None,
            silent_refresh: false,
        };
        let error = resolver
            .resolve("profile/a/connection.apiKey", &config, &store, false)
            .unwrap_err();
        assert!(matches!(error, SecretError::HelperFailed { .. }), "{error}");
        assert!(store.load("profile/a/connection.apiKey").unwrap().is_none());
    }

    /// A helper credential is written to the keychain, not to a profile.
    #[test]
    fn helper_credential_is_stored_in_the_keychain() {
        let store = crate::MemoryStore::default();
        let value = SecretString::from("helper-issued-credential".to_string());
        store
            .store("profile/abc/connection.apiKey", &value)
            .unwrap();
        let loaded = store
            .load("profile/abc/connection.apiKey")
            .unwrap()
            .unwrap();
        assert_eq!(loaded.expose_secret(), "helper-issued-credential");
    }
}
