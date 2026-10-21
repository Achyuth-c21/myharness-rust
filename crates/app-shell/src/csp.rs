//! CSP assembly (docs/10 §5.1).
//!
//! Rules enforced here:
//! - policy is generated from validated config, never assembled ad hoc;
//! - `unsafe-eval` is never granted; `unsafe-inline` never for scripts;
//! - origins must be https (or the app origin) and silently incomplete config does
//!   not widen the policy — the strictest interpretation wins (docs/01 methodology);
//! - inline bootstrap injection, when Phase 2 adds public config, materializes as a
//!   pinned hash — never `unsafe-inline`.

#[derive(Debug, Clone, Default)]
pub struct CspConfig {
    /// https origins the renderer may call (connect-src).
    pub api_origins: Vec<String>,
    /// Dedicated artifact/content iframe origin (frame-src); stays disjoint from app.
    pub artifact_iframe_origin: Option<String>,
    /// Configured extra connect sources (docs/10 §5.1).
    pub connect_extra_origins: Vec<String>,
    /// Pinned hash of the documented inline bootstrap script ('sha256-…'), or none.
    pub allowed_script_hash: Option<String>,
}

/// Origin forms accepted into a directive (report-bug-hunt B2): the exact app
/// origin, or `https://<host>[:<port>]` with only authority characters. Values are
/// interleaved verbatim into the joined directive string, so anything looser is
/// directive injection (`https://x; script-src 'unsafe-inline'` must never pass —
/// no separators, slashes beyond the scheme, quotes, or spaces).
fn valid_origin(origin: &str) -> bool {
    if origin.eq_ignore_ascii_case(crate::protocol::ALLOWED_ORIGIN) {
        return true;
    }
    let lower = origin.to_ascii_lowercase();
    let Some(remainder) = lower.strip_prefix("https://") else {
        return false;
    };
    if remainder.is_empty() {
        return false;
    }
    // to_ascii_lowercase preserves len, so slice the original by the remainder.
    let tail = &origin[origin.len() - remainder.len()..];
    tail.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b':'))
}

/// Integrity-source form accepted into script-src (report-bug-hunt B2): a fully
/// closed `'sha256-<digest>'` literal with only base64 characters — no internal
/// quotes, spaces, or separators that could smuggle a second source or directive.
fn valid_script_hash(hash: &str) -> bool {
    let Some(inner) = hash.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) else {
        return false;
    };
    let Some(digest) = inner.strip_prefix("sha256-") else {
        return false;
    };
    !digest.is_empty()
        && digest.len() <= 44
        && digest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_'))
}

pub fn build(config: &CspConfig) -> String {
    let mut script = String::from("'self'");
    if let Some(hash) = config.allowed_script_hash.as_deref() {
        if valid_script_hash(hash) {
            script.push(' ');
            script.push_str(hash);
        }
    }
    let mut connect = String::from("'self'");
    for origin in config.api_origins.iter().chain(config.connect_extra_origins.iter()) {
        if valid_origin(origin) {
            connect.push(' ');
            connect.push_str(origin);
        }
    }
    let mut frame = String::from("'self'");
    if let Some(artifact) = config.artifact_iframe_origin.as_deref() {
        if valid_origin(artifact) {
            frame.push(' ');
            frame.push_str(artifact);
        }
    }
    format!(
        "default-src 'self'; script-src {script}; style-src 'self'; img-src 'self' data:; \
         media-src 'self'; font-src 'self' data:; connect-src {connect}; frame-src {frame}; \
         object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'self'"
    )
}

/// Phase 1 static policy: matches what the renderer actually loads today.
pub fn default_policy() -> String {
    build(&CspConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_tight() {
        let policy = default_policy();
        assert!(policy.contains("object-src 'none'"));
        assert!(policy.contains("script-src 'self'"));
        assert!(!policy.contains("unsafe-eval"));
        assert!(!policy.contains("unsafe-inline"));
        assert!(policy.contains("frame-ancestors 'self'"));
    }

    #[test]
    fn origins_only_when_valid() {
        let config = CspConfig {
            api_origins: vec!["https://api.example.com".to_string(), "javascript:alert(1)".to_string()],
            artifact_iframe_origin: Some("https://artifacts.example.com".to_string()),
            connect_extra_origins: vec![],
            allowed_script_hash: None,
        };
        let policy = build(&config);
        assert!(policy.contains("https://api.example.com"));
        assert!(!policy.contains("javascript:"));
        assert!(policy.contains("https://artifacts.example.com"));
    }

    #[test]
    fn script_hash_never_unsafe_inline() {
        let config = CspConfig {
            allowed_script_hash: Some("'unsafe-inline'".to_string()),
            ..CspConfig::default()
        };
        assert!(!build(&config).contains("unsafe-inline"));
        let config = CspConfig {
            allowed_script_hash: Some("'sha256-WODSNEREpAmvPTgjp9z9DA43dClAW OncejRwHJ0XNiw='".replace(' ', "")),
            ..CspConfig::default()
        };
        assert!(build(&config).contains("'sha256-WODSNEREpAmvPTgjp9z9DA43dClAWOncejRwHJ0XNiw='"));
    }

    /// report-bug-hunt B2: CSP values must never inject a second directive.
    #[test]
    fn csp_values_cannot_inject_directives() {
        let config = CspConfig {
            api_origins: vec!["https://evil.example; script-src 'unsafe-inline'".to_string()],
            connect_extra_origins: vec!["https://x'.base-uri 'self'; object-src *".to_string()],
            artifact_iframe_origin: Some("https://frame.example/..'unsafe-eval'".to_string()),
            ..CspConfig::default()
        };
        let policy = build(&config);
        assert_eq!(policy.matches("script-src").count(), 1, "single script-src");
        assert_eq!(policy.matches("object-src").count(), 1, "single object-src");
        assert_eq!(policy.matches("base-uri").count(), 1, "single base-uri");
        assert_eq!(policy.matches("unsafe-eval").count(), 0, "no smuggled unsafe-eval");
        assert_eq!(policy.matches("unsafe-inline").count(), 0, "no smuggled unsafe-inline");
    }

    /// report-bug-hunt B2: script hashes must be closed integrity literals.
    #[test]
    fn script_hash_must_be_closed_integrity_literal() {
        for bad in [
            "'sha256-abc'unsafe-inline'", // trailing source injection
            "sha256-abc",                 // no quotes
            "'sha256-a abc==",            // unclosed quote
            "'sha256-a b'",               // space (source separator)
            "''",                         // empty
            "'sha256-'",                  // empty digest
        ] {
            let config = CspConfig {
                allowed_script_hash: Some(bad.to_string()),
                ..CspConfig::default()
            };
            let policy = build(&config);
            assert_eq!(policy.matches("script-src").count(), 1, "single script-src for {bad}");
            assert!(!policy.contains("unsafe-inline"), "no inline for {bad}");
            assert!(!policy.contains("sha256-a b"), "no split source for {bad}");
            if bad != "'sha256-abc'unsafe-inline'" {
                assert!(!policy.contains("sha256-ab"), "rejects {bad}");
            }
        }
    }
}
