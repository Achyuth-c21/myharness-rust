//! Tauri 2 shell wiring — Phase 1 of docs/13-implementation-roadmap.md.
//!
//! Behavior traced to verified evidence:
//! - launches sequence per docs/02 §2: single-instance lock first (step 2), privileged
//!   scheme registration (step 7 in the original; here `app://`), then window creation;
//! - protocol gates are implemented in `myharness_app_shell::protocol` and unit-tested
//!   there; this binary only adapts request/response shapes (docs/10 §5.1);
//! - the renderer has no Node access; privileged work crosses the typed bridge
//!   (`crates/bridge`), which ships only `ping` in Phase 1 (docs/18);
//! - deep links validate via `myharness_app_shell::lifecycle` (docs/06 route grammar).

use myharness_app_shell as shell;

#[cfg(feature = "single-instance")]
use tauri::Manager as _;

/// Resolve the production static root (`frontendDist`). Development uses `devUrl`
/// instead, so a missing root must never hard-crash: the protocol still answers with
/// the typed envelope (see `shell::protocol::serve`).
fn asset_root() -> shell::AssetRoot {
    if let Some(value) = std::env::var_os("MYHARNESS_DIST") {
        let path = std::path::PathBuf::from(value);
        if path.is_dir() {
            return shell::AssetRoot::new(path);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for rel in ["apps/desktop/dist", "dist"] {
            let candidate = cwd.join(rel);
            if candidate.is_dir() {
                return shell::AssetRoot::new(candidate);
            }
        }
    }
    shell::AssetRoot::new(std::path::PathBuf::from("/nonexistent-myharness-dist"))
}

/// Uniform app:// fallback for adapter-level failures only (e.g. malformed output
/// construction in `adapt`). All gate/typed-envelope behavior lives in
/// `shell::protocol::serve`, so this should be unreachable in practice.
fn fallback_response() -> tauri::http::Response<Vec<u8>> {
    let mut response = tauri::http::Response::new(Vec::new());
    *response.status_mut() = tauri::http::StatusCode::INTERNAL_SERVER_ERROR;
    response
}

/// Debug-only runtime trace. The native fixture (`tests/native/shell-runtime.sh`)
/// asserts on these `[myharness] …` lines; release builds print nothing.
fn dlog(message: &str) {
    #[cfg(debug_assertions)]
    eprintln!("[myharness] {message}");
}

/// Log-safe rendering of roll-in text: control characters (log-line injection via a
/// crafted argv) collapse to `?`.
fn safe_text(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Phase-1 deep-link plumbing (docs/02 §2 step 2, docs/06 §6): the launch URL rides
/// argv on a cold start and forwarded argv on a second launch (single-instance
/// callback routes both through the primary). Classification is fail-closed — only
/// the documented grammar (`shell::lifecycle::classify_deep_link`, docs/06 §1 route
/// table, TEST-SEC-003) navigates; anything else is rejected with the window
/// unaffected. `Setup3p` is recognized but deferred: the dedicated setup window is a
/// Phase-2 surface (docs/13 §4, ARCH-NAV-03 "separate and independently reachable"),
/// so the main renderer must never navigate to it. Returns the app URL to navigate.
fn deep_link_target(args: &[String]) -> Option<tauri::Url> {
    let (raw, route) = args
        .iter()
        .find_map(|arg| {
            shell::classify_deep_link(arg)
                .ok()
                .map(|route| (arg.clone(), route))
        })?;
    if route == shell::DeepLinkRoute::Setup3p {
        dlog("deep-link setup-desktop-3p: dedicated setup window lands in Phase 2 (ARCH-NAV-03); renderer not navigated");
        return None;
    }
    let rest = raw.split_once("://")?.1;
    let rest = if rest.is_empty() {
        "new".to_string()
    } else {
        rest.trim_start_matches('/').to_string()
    };
    // Malformed paths cannot parse as a URL -> None (fail closed, no navigation).
    let url = tauri::Url::parse(&format!("app://localhost/{rest}")).ok()?;
    dlog(&format!(
        "deep-link accepted {} -> {url}",
        safe_text(&raw)
    ));
    Some(url)
}

/// Log the rejections (fixture observability); navigation decisions unaffected.
fn deep_link_rejections(args: &[String]) {
    for arg in args {
        if arg.contains("://") && shell::classify_deep_link(arg).is_err() {
            dlog(&format!(
                "deep-link rejected {} (fail closed, window unaffected)",
                safe_text(arg)
            ));
        }
    }
}

fn adapt(request: tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    // docs/02 §4 gate 1: hostname must be `localhost`; other hosts return 404.
    // report-bug-hunt B1: a request with NO Host header must not default to the
    // allowed value — consult the request URI's authority when present, and
    // otherwise fail CLOSED with an empty host (the gate exists to reject
    // exactly what cannot be verified; `protocol::serve` compares the value).
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or_else(|| {
            request
                .uri()
                .host()
                .map(|uri_host| uri_host.trim_start_matches('[').trim_end_matches(']').to_string())
        })
        .unwrap_or_default();
    let origin = request
        .headers()
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let kind = if request.headers().contains_key("origin") {
        shell::RequestKind::Fetch
    } else {
        shell::RequestKind::Navigation
    };
    let path = request.uri().path().to_string();
    let csp = shell::default_policy();
    let root = asset_root();
    let response = shell::serve(
        &root,
        &csp,
        &shell::HttpRequest {
            path,
            host,
            origin,
            kind,
        },
    );

    let mut builder = tauri::http::Response::builder();
    builder = builder.status(response.status.as_u16());
    for (key, value) in &response.headers {
        builder = builder.header(*key, value);
    }
    // Adapter-level construction failures (never observed in practice) answer with
    // the uniform fallback rather than a panic — the window keeps its shell.
    builder
        .body(response.body)
        .unwrap_or_else(|_| fallback_response())
}

/// First, deliberately minimal typed command (docs/18): liveness only.
#[tauri::command]
fn ping() -> myharness_bridge::Response {
    myharness_bridge::dispatch(myharness_bridge::Command::Ping)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // docs/02 §2 (TEST-ARCH-002): the primary instance takes the lock; a second launch
    // is routed through it. Gated so machines without system dbus can still build
    // (`--no-default-features`); default-on keeps documented behavior.
    #[cfg(feature = "single-instance")]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        deep_link_rejections(&args);
        let url = deep_link_target(&args);
        if let Some(window) = app.get_webview_window("main") {
            if let Some(url) = url {
                if let Err(error) = window.navigate(url) {
                    dlog(&format!("deep-link navigation failed: {error}"));
                }
            }
            let _ = window.set_focus();
        }
    }));

    // Cold start (docs/02 §2 the same launch sequence): a deep link may arrive as argv
    // of the FIRST process. Config windows exist by the time `setup` runs; navigate
    // them once boot reaches a window. Rejections logged, boot unaffected.
    let startup_args: Vec<String> = std::env::args().skip(1).collect();

    builder
        .register_uri_scheme_protocol("app", |_context, request| adapt(request))
        .invoke_handler(tauri::generate_handler![ping])
        .setup(move |app| {
            deep_link_rejections(&startup_args);
            if let Some(url) = deep_link_target(&startup_args) {
                match app.get_webview_window("main") {
                    Some(window) => {
                        if let Err(error) = window.navigate(url) {
                            dlog(&format!("cold-start deep-link navigation failed: {error}"));
                        }
                    }
                    None => dlog("cold-start deep-link dropped: main window not present at setup"),
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running myharness");
}
