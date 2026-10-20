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

fn adapt(
    request: tauri::http::Request<Vec<u8>>,
) -> Result<tauri::http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| shell::protocol::HOST.to_string());
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
    builder
        .body(response.body)
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
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
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }));

    builder
        .register_uri_scheme_protocol("app", |_context, request| adapt(request))
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running myharness");
}
