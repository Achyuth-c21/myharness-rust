//! Tauri 2 shell for the myharness desktop app.
//!
//! Phase 1 of docs/13-implementation-roadmap.md wires lifecycle, windows, menus, the
//! `app://` protocol with CSP, single-instance and deep links here, delegating to
//! `myharness-app-shell` and exposing the typed surface owned by `crates/bridge`.

/// Entry used by main.rs. Runtime wiring lands in Phase 1; do not ship this stub to users.
pub fn run() {
    // Phase 1:
    //   tauri::Builder::default()
    //     .plugin(myharness_app_shell::lifecycle())
    //     .run(tauri::generate_context!())
    //     .expect("error while running myharness");
}
