# 21 — Repository Layout (Scaffold)

Created 2026-10-20. Implements the target module layout from [13-implementation-roadmap.md](13-implementation-roadmap.md)
section 3 and the module map in [10-architecture.md](10-architecture.md) section 9, on the chosen Rust stack
([STACK.md](../STACK.md)): Tauri 2 shell, Rust core, React + TypeScript + Vite renderer.

## Tree

```text
apps/desktop/            the desktop app
  src/                   React + TS renderer (the "main renderer" trust domain)
    routes/              client-side routing table (new, cowork, epitaxy, projects, artifacts, scheduled)
    screens/             one folder per screen from docs/05-screen-inventory.md
    components/          design system components (docs/07-ui-design-system.md)
    composer/            composer and popovers
    settings/            settings modal and language dialog
    setup/               setup-desktop-3pwindow content
  src-tauri/             Tauri 2 shell crate (bin): lifecycle wiring, windows, app:// protocol
crates/                  Rust workspace libraries (one module per roadmap stage)
  bridge/                typed IPC envelope replacing the Electron preload namespaces (docs/18)
  config/                profiles, managed overlay, validation, credentials assembly
  secrets/               OS keychain only (keyring); secrecy/zeroize
  protocol-core/         TurnEvent / ModelDescriptor / usage types
  provider-sdk/          adapter trait + conformance kit (no protocol translation, ever)
  providers/             six native adapters (gateway, anthropic, bedrock, bedrock-mantle, vertex, foundry)
  agent/                 turn loop: streaming, tools, approvals, cancellation
  sessions/              session metadata, transcripts, retention
  store/                 SQLite WAL: migrations, repos, content-addressed files
  mcp/                   rmcp runtime: lifecycle, OAuth, tool registry, egress/approval policy
  skills/ plugins/ projects/ artifacts/ scheduler/ search/
  app-shell/             shell logic shared with the Tauri shell crate
  telemetry/             usage/crash/OTel with write-time redaction
  updates/               signed feed, verify, enforce, rollback
  shared/                schemas, redaction helpers, policy types, error taxonomy
tests/
  unit/ integration/ e2e/ wire/ fixtures/       layers from roadmap section 6.1
```

## Mapping from the roadmap's Electron layout

The roadmap sketched `src/main` / `src/preload` / `src/renderer`. In this Tauri build there is no
Node main process and no preload script; the equivalent trust domains are Rust crates plus the
Tauri shell:

| Roadmap (Electron)      | This scaffold                                   |
|-------------------------|-------------------------------------------------|
| `src/main/lifecycle`    | `apps/desktop/src-tauri` + `crates/app-shell`   |
| `src/main/config`       | `crates/config`, `crates/secrets`               |
| `src/main/providers`    | `crates/providers/*`, `crates/provider-sdk`     |
| `src/main/agent`        | `crates/agent`                                  |
| `src/main/mcp`          | `crates/mcp`                                    |
| `src/main/sessions`     | `crates/sessions`, `crates/store`               |
| `src/main/telemetry`    | `crates/telemetry`                              |
| `src/main/updates`      | `crates/updates`                                |
| `src/preload` (bridge)  | `crates/bridge` (typed Tauri commands/events)   |
| `src/renderer`          | `apps/desktop/src`                              |
| `packages/*`            | `crates/protocol-core`, `crates/shared`         |
| `tests/*`               | `tests/*` (unchanged)                           |

Trust boundaries from docs/10 section 8 are preserved by structure: the renderer crate never links
`crates/secrets` or `crates/providers/*`; all privileged operations cross `crates/bridge` (ARCH-PROC-02/03/05).

## State

- `cargo metadata` parses the 26-member workspace; every crate is a documented stub (`lib.rs` header names
  its roadmap phase and spec docs).
- Frontend is a minimal Vite shell; `npm install` not yet run.
- Icons not yet generated: run `cargo tauri icon <source>` (see `apps/desktop/src-tauri/icons`).
- Branch model: `main` is the reviewed baseline, `dev` is the working copy per the repo workflow.
