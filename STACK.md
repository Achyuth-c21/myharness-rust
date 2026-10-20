# myharness — Target Stack

**Variant:** Rust stack

## Intended stack

- **Shell:** Tauri 2 (`tao`/`wry`) with a custom `app://` protocol
- **Core:** Rust (`tokio`, `serde`, `axum` only where a local boundary is justified)
- **UI:** React + TypeScript + Vite
- **Providers:** native Rust provider adapters; no protocol translation or inference gateway
- **MCP:** Rust MCP SDK (`rmcp` or equivalent) with stdio/HTTP transports
- **Storage:** SQLite/WAL
- **Secrets:** OS keychain via `keyring`, with `secrecy`/`zeroize`
- **Processes/PTY:** `portable-pty`, `tokio::process`
- **Packaging:** Tauri bundler with signed updates

## Trade-off

Tauri uses the system webview (WebKitGTK on Linux), not Chromium. That reduces binary size and keeps
the backend in Rust, but exact pixel/behavior parity with the Electron build must be tested.

**Status:** Rust workspace scaffold exists (`apps/desktop` + `crates/`, laid out in
[docs/21-repo-layout.md](docs/21-repo-layout.md)). Structure only — no application code yet.

The shared product behavior is defined in `README.md` and `docs/`.
