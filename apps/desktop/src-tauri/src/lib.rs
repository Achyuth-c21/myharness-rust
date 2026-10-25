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

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use myharness_app_shell as shell;
use myharness_bridge::catalog::CatalogRuntime;
use myharness_bridge::chat::{
    dispatch as chat_dispatch, ChatCommand, ChatResponse, ChatService, KeyringAuditKeys,
    ProfileTargets,
};
use myharness_bridge::mcp::{
    BridgeApprovalPrompt, BuiltMcp, McpConfig, ReqwestOAuthHttp, ToolRuntime,
};
use myharness_bridge::setup::{BridgeError, ConfigBackend, SetupCommand, SetupResponse};
use myharness_provider_sdk::TurnEvent;
use myharness_sessions::SessionService;
use myharness_store::Store;
use tauri::{Emitter as _, Manager as _, WebviewUrl, WebviewWindowBuilder};

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
    // Release builds print nothing, so the argument is unused there.
    #[cfg(not(debug_assertions))]
    let _ = message;
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
    let (raw, route) = args.iter().find_map(|arg| {
        shell::classify_deep_link(arg)
            .ok()
            .map(|route| (arg.clone(), route))
    })?;
    if route == shell::DeepLinkRoute::Setup3p {
        // The dedicated setup window (docs/09 §2.1; ARCH-NAV-03) is opened by the
        // caller; the main renderer is never navigated to it.
        dlog("deep-link setup-desktop-3p: opening the dedicated setup window");
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
    dlog(&format!("deep-link accepted {} -> {url}", safe_text(&raw)));
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
            request.uri().host().map(|uri_host| {
                uri_host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string()
            })
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

/// The profile-library root (`docs/09` §3.1: `~/.config/Claude-3p/configLibrary`).
///
/// `XDG_CONFIG_HOME` wins when it is an absolute path (the Linux convention);
/// otherwise `$HOME/.config`. When neither is available the configuration is
/// genuinely unreachable, and the setup window says so rather than writing
/// somewhere unexpected.
fn config_root() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".config"))
        })?;
    Some(base.join("Claude-3p").join("configLibrary"))
}

/// The setup backend, opened once. The library's own writes are atomic
/// (`crates/config`), and the mutex serializes the read-modify-write sequences
/// that span more than one file.
fn setup_backend() -> Result<&'static Mutex<ConfigBackend>, String> {
    static BACKEND: OnceLock<Mutex<ConfigBackend>> = OnceLock::new();
    let root = config_root().ok_or_else(|| {
        "The configuration directory is not available; set HOME or XDG_CONFIG_HOME.".to_string()
    })?;
    let backend = BACKEND.get_or_init(|| {
        // The OS keychain holds the credentials; the profile only carries handles
        // (`docs/09` §4; `DATA-CONFIG-02`). The keychain's runtime behavior is
        // compile-verified only (`docs/14` §9 item 8) - a failure here is reported
        // to the setup window rather than silently falling back to a plaintext
        // profile field.
        let store = myharness_secrets::KeyringStore::new(myharness_secrets::DEFAULT_SERVICE);
        Mutex::new(ConfigBackend::new(root, Box::new(store)))
    });
    Ok(backend)
}

/// The app's chat service: SQLite sessions under the app data directory, audit
/// keys in the OS keychain (`docs/11` §4.3), targets from the applied profile.
///
/// The store is opened once per process. A failure is reported to the caller
/// instead of falling back to an in-memory or renderer-owned store — losing
/// transcripts silently would be worse than refusing to send.
type Chat = ChatService<KeyringAuditKeys, ProfileTargets>;

fn chat_state(app: &tauri::AppHandle) -> Result<&'static Chat, String> {
    static CHAT: OnceLock<Result<Chat, String>> = OnceLock::new();
    match CHAT.get_or_init(|| init_chat(app)) {
        Ok(chat) => Ok(chat),
        Err(message) => Err(message.clone()),
    }
}

fn init_chat(app: &tauri::AppHandle) -> Result<Chat, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("the app data directory is not available: {error}"))?;
    let root = data.join("chat");
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("the session directory could not be created: {error}"))?;
    let store = Store::open(&root.join("sessions.db"), &root.join("files"))
        .map_err(|error| format!("the session store could not be opened: {error}"))?;
    let keys = KeyringAuditKeys::new(Box::new(myharness_secrets::KeyringStore::new(
        myharness_secrets::DEFAULT_SERVICE,
    )));
    let targets = ProfileTargets::new(setup_backend()?);

    // MCP is optional and must never block chat. A malformed MCP file becomes
    // a warning on an empty manager; a failed start is reported per server.
    let mcp_root = data.join("mcp");
    let mut mcp_warnings = Vec::new();
    if let Err(error) = std::fs::create_dir_all(&mcp_root) {
        // MCP is optional: a broken directory disables the MCP surface but
        // must not take chat down with it.
        mcp_warnings.push(format!(
            "the MCP directory could not be created; MCP is disabled: {error}"
        ));
    }
    let catalog = Arc::new(CatalogRuntime::load(data.join("catalog")));
    let mut loaded = McpConfig::load(&mcp_root);
    loaded.warnings.extend(mcp_warnings);
    // Plugin-contributed servers join the normal manager/policy path. A
    // plugin enable/disable change takes effect on the next app start; the
    // current session snapshot is never mutated underneath a turn.
    catalog.apply_plugin_servers(&mut loaded.config);
    let oauth_store = myharness_bridge::mcp::keyring_oauth_store(Arc::new(
        myharness_secrets::KeyringStore::new(myharness_secrets::DEFAULT_SERVICE),
    ));
    let oauth_http = match ReqwestOAuthHttp::new() {
        Ok(client) => Arc::new(client) as Arc<dyn myharness_mcp::OAuthHttp>,
        Err(error) => {
            let mut warnings = loaded.warnings.clone();
            warnings.push(format!("OAuth is unavailable: {error}"));
            let built = BuiltMcp {
                manager: Arc::new(tokio::sync::Mutex::new(myharness_mcp::McpManager::new())),
                config: myharness_bridge::mcp::McpConfig::default(),
                warnings,
            };
            return Ok(chat_with_tools(
                app,
                SessionService::new(store, keys),
                targets,
                built,
                oauth_store,
                mcp_root,
                catalog,
            ));
        }
    };
    let built = match loaded.config.build(
        &mcp_root.join("schemas.json"),
        oauth_store.clone(),
        oauth_http,
    ) {
        Ok(mut built) => {
            built.warnings.extend(loaded.warnings);
            built
        }
        Err(error) => {
            let mut warnings = loaded.warnings;
            warnings.push(format!("MCP is disabled: {error}"));
            BuiltMcp {
                manager: Arc::new(tokio::sync::Mutex::new(myharness_mcp::McpManager::new())),
                config: myharness_bridge::mcp::McpConfig::default(),
                warnings,
            }
        }
    };
    Ok(chat_with_tools(
        app,
        SessionService::new(store, keys),
        targets,
        built,
        oauth_store,
        mcp_root,
        catalog,
    ))
}

fn chat_with_tools(
    app: &tauri::AppHandle,
    sessions: SessionService<KeyringAuditKeys>,
    targets: ProfileTargets,
    built: BuiltMcp,
    oauth_store: Arc<dyn myharness_mcp::OAuthStore>,
    mcp_root: std::path::PathBuf,
    catalog: Arc<CatalogRuntime>,
) -> Chat {
    let handle = app.clone();
    let prompt = Arc::new(BridgeApprovalPrompt::new(Arc::new(move |event| {
        let _ = handle.emit("chat://tool-approval", event);
    })));
    let runtime = Arc::new(ToolRuntime::new(
        built,
        prompt,
        Some(mcp_root.join("approvals.json")),
        Some(oauth_store),
    ));
    let boot = Arc::clone(&runtime);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let summary = boot.start_all().await;
        let (servers, warnings) = boot.statuses().await;
        let _ = handle.emit(
            "chat://mcp-status",
            serde_json::json!({
                "summary": summary,
                "servers": servers,
                "warnings": warnings,
            }),
        );
    });
    ChatService::new(sessions, targets)
        .with_tools(runtime)
        .with_catalog(catalog)
}

/// The chat subset of `web.LocalAgentModeSessions` (docs/18 §3.4/§4). Content
/// deltas are forwarded as `chat://turn-event` with the session id; the
/// command's response is the final state (usage, stop reason, errors).
#[tauri::command]
async fn chat_command(app: tauri::AppHandle, command: ChatCommand) -> ChatResponse {
    let service = match chat_state(&app) {
        Ok(service) => service,
        Err(message) => {
            return ChatResponse::Error {
                error: BridgeError::new("STORE_UNAVAILABLE", message),
                user_turn_recorded: false,
            }
        }
    };
    let session_id = match &command {
        ChatCommand::SendTurn { session_id, .. } => Some(session_id.clone()),
        _ => None,
    };
    let handle = app.clone();
    let observer = move |event: &TurnEvent| {
        if let Some(session_id) = &session_id {
            let payload = serde_json::json!({
                "sessionId": session_id,
                "event": event,
            });
            let _ = handle.emit("chat://turn-event", payload);
        }
    };
    chat_dispatch(service, command, Some(&observer)).await
}

/// The `settings.Custom3pSetup` namespace (docs/18 §3.4): profiles, config
/// read/write/apply, the merged view, health, the export bundle, and the setup
/// form schemas. Arguments are validated in the bridge; secrets cross this
/// boundary as handles, never as values (docs/18 §10).
#[tauri::command]
fn setup_command(command: SetupCommand) -> SetupResponse {
    let backend = match setup_backend() {
        Ok(backend) => backend,
        Err(message) => {
            return SetupResponse::Error {
                error: myharness_bridge::setup::BridgeError {
                    code: "CONFIG_IO".to_string(),
                    message,
                    retryable: false,
                },
            }
        }
    };
    let guard = match backend.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return SetupResponse::Error {
                error: myharness_bridge::setup::BridgeError {
                    code: "CONFIG_IO".to_string(),
                    message: "The configuration is locked by another operation.".to_string(),
                    retryable: true,
                },
            }
        }
    };
    myharness_bridge::setup::dispatch(&*guard, command)
}

/// Model discovery against an Anthropic-compatible gateway
/// (`docs/12` §3.1). This is the `settings.Custom3pHelperRun` namespace
/// (`docs/18` §3.4): a setup-time probe of the gateway's own `/v1/models`
/// contract, not a translation layer.
#[tauri::command]
async fn setup_discover_models(
    request: myharness_bridge::helper::DiscoveryRequest,
) -> Result<myharness_bridge::helper::DiscoveryOutcome, myharness_bridge::setup::BridgeError> {
    let client = reqwest::Client::new();
    myharness_bridge::helper::discover_models(&request, &client).await
}

/// Model discovery against an OpenAI-compatible endpoint — the open model
/// layer's families (`docs/13` §C; owner decision recorded 2026-10-23,
/// `docs/14` §9): first-party `openai` and a user-defined `custom` base
/// URL, key optional. A setup-time probe of the endpoint's own `/models`
/// contract, the same namespace posture as the gateway discovery — not a
/// translation layer.
#[tauri::command]
async fn setup_discover_openai_models(
    request: myharness_bridge::helper::OpenAiDiscoveryRequest,
) -> Result<myharness_bridge::helper::OpenAiDiscoveryOutcome, myharness_bridge::setup::BridgeError>
{
    let client = reqwest::Client::new();
    myharness_bridge::helper::discover_openai_models(&request, &client).await
}

/// Open (or focus) the separate setup window (`docs/09` §2.1: "This is a
/// separate Electron window, not a modal in the main renderer";
/// `docs/screens/setup-desktop-3p.md`: `app://localhost/setup-desktop-3p`).
///
/// The window is created on demand so the app boots with the main window only.
#[tauri::command]
fn open_setup_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("setup-desktop-3p") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(
        &app,
        "setup-desktop-3p",
        WebviewUrl::App("app://localhost/setup-desktop-3p".into()),
    )
    .title("Configure third-party inference")
    .inner_size(900.0, 720.0)
    .build()
    .map_err(|error| error.to_string())?;
    // The fixture asserts on this line: it is the proof that the window was
    // actually created, not merely that the deep link was classified.
    dlog("setup window opened");
    Ok(())
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
        let setup_requested = args.iter().any(|arg| {
            shell::classify_deep_link(arg)
                .map(|route| route == shell::DeepLinkRoute::Setup3p)
                .unwrap_or(false)
        });
        if setup_requested {
            // The setup window is separate and independently reachable (ARCH-NAV-03).
            if let Err(error) = open_setup_window(app.clone()) {
                dlog(&format!("setup window could not be opened: {error}"));
            }
            return;
        }
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
        .invoke_handler(tauri::generate_handler![
            ping,
            chat_command,
            setup_command,
            setup_discover_models,
            setup_discover_openai_models,
            open_setup_window
        ])
        .setup(move |app| {
            deep_link_rejections(&startup_args);
            let setup_requested = startup_args.iter().any(|arg| {
                shell::classify_deep_link(arg)
                    .map(|route| route == shell::DeepLinkRoute::Setup3p)
                    .unwrap_or(false)
            });
            if setup_requested {
                if let Err(error) = open_setup_window(app.handle().clone()) {
                    dlog(&format!("setup window could not be opened: {error}"));
                }
            }
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
