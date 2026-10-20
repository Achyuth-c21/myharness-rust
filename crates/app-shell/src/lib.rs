//! Desktop shell: lifecycle, windows, menus, `app://` protocol serving, CSP, theme.
//!
//! Phase 1 of docs/13-implementation-roadmap.md. This crate is deliberately
//! `std`-only so the security-critical logic (protocol guard, CSP assembly,
//! deep-link allowlisting) is unit-testable without a webview; the Tauri binary
//! in `apps/desktop/src-tauri` adapts these primitives into runtime handlers.

pub mod csp;
pub mod lifecycle;
pub mod protocol;

pub use csp::{build as build_csp, CspConfig, default_policy};
pub use lifecycle::{classify_deep_link, DeepLinkError, DeepLinkRoute};
pub use protocol::{
    serve, AssetRoot, HttpRequest, HttpResponse, RequestKind, ResolveError, StatusCode,
};
