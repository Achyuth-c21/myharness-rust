# Final Sprint — Completion Checklist (working state, on disk only)

Statuses: **complete** · **incomplete** · **blocked** · **verified**
("complete" = implemented + tested at the recorded level; "verified" = additionally exercised
end-to-end in this environment. A stub or a UI element is not complete.)

Baseline (2026-10-26, dev @ `706f2ec`): Phases 0–7 complete and merged/pushed with owner
approval; Phase 8 slices 1–2 (Responses wall + setup surface) complete and pushed. Workspace
548 cargo tests / 0 failed; renderer gates green; E2E T01–T49 + setup 16/16.

**Final sprint state (2026-10-26, lane `feature-final-sprint`, commits `8965209` → `57de540`):
the full workspace suite is 596 cargo tests / 0 failed; clippy `--workspace --all-targets
-D warnings` clean; `cargo metadata` green; both E2E suites ALL PASS (shell T01–T49, setup
16/16); the release desktop binary builds under `--locked` (37.7 MB). Nothing has been
merged to `dev` or pushed — owner authorization pending, per the sprint mandate.

## Phase 8 — extensions and provider coverage

| Item | Status |
|---|---|
| Provider adapters: gateway, anthropic (Claude API), bedrock, openai (Chat Completions: first-party + custom), openai-responses | complete |
| Provider adapters: bedrock-mantle, foundry | incomplete — deliberately stubbed/locked (roadmap order overtaken by the open model layer; deferral rationale recorded this sprint) |
| Provider adapters: gemini, minimax-native | incomplete — deliberately deferred (not required by governing criteria; rationale recorded this sprint) |
| Local provider extension (Ollama / llama.cpp surface) | **complete** (wire battery 20 + family surface + 3 bridge lane tests; verified through E2E) |
| Extension registration framework (TEST-EXT-001/002/004/006/007) | **partial, honestly split**: manifest schema/integrity/lazy-isolated loader complete with adversarial tests; TEST-EXT-007's UI half complete (the family catalog crosses the bridge); the production trust anchor is **blocked** (no credential — fail-closed refusal by construction); TEST-EXT-004's static tripwire and the agent-side registration are the recorded residuals |
| Signed-package trust anchor | blocked — no production signing credential or captured trust anchor exists; fail-closed refusal by construction (same posture as the catalog signature, docs/14 §9 item 20) |
| Wire batteries (gateway 23, anthropic 21+10u, bedrock 34, openai 21, responses 17) | verified |

## Phase 9 — packaging and hardening

### Measured results (2026-10-26, this machine)

- **Performance budgets** (`performance_budgets_over_a_local_workload`,
  crates/store tests — real run): create 500 sessions **32.5 ms** (budget 5 s),
  2,000 ledger rows **79.8 ms** (budget 5 s), list 500 sessions **2.0 ms**
  (budget 300 ms), backup of the populated store **3.2 ms** (budget 2 s). The
  test pins the ceilings so a catastrophic regression fails the suite.
- **Reproducible builds — honest result: NOT bit-for-bit reproducible.**
  Two independent release builds of `myharness-provider-local` under
  `--locked` (toolchain cargo/rustc 1.99.0 Fedora) produced **different**
  rlib hashes (`052640b9…` vs `2ff75098…`) — cargo metadata embeds
  target-path-dependent material. Recorded, not claimed away. What the
  build DOES pin: locked dependencies (`--locked`), the recorded toolchain
  version, workspace-pinned dependency versions. Precise residual: making
  artifacts bit-identical needs `-C metadata`/path canonicalization work
  (e.g. a fixed target dir or cargo's future reproducibility switches).
- **Packaging**: the tauri bundler CLI is **not installed** in this
  environment; no installer was built and none is claimed. The buildable
  artifact evidence is the release binary build (`cargo build --release -p
  myharness-desktop --locked`) — result recorded below. **Correction
  (2026-10-27, `docs/17` §4.20 `WA-01`):** building was not running — the
  binary still loaded `devUrl` because `custom-protocol` was never enabled.
  The feature is now on by default; with no dev server the release binary
  renders the embedded `app://` bundle.
- **Signing**: production signing credentials do not exist. The fail-closed
  postures hold by construction (the update verifier refuses every bundle;
  the extension anchor refuses every signature); dev-vs-production trust
  separation is recorded (`docs/14` §9 item 74). **Status: implementation
  complete at the verification-gate level; release signing is
  authorization-pending.**

### Accessibility audit (2026-10-26)

Checked: every icon-only control sampled carries an `aria-label` ("Close",
"Send", "Toggle sidebar", "Toggle panel", "Stop", "Search tasks",
"Filter tasks"…); decorative SVGs are `aria-hidden`; the Cowork/Code mode
switch has `tablist`/`tab`/`aria-selected` semantics; keyboard flows are
E2E-exercised (Ctrl+K palette, Ctrl+N draft clear — `shell.spec.mjs`).
Remaining manual verification (not performed here, recorded): a screen-reader
walkthrough, contrast measurement, and focus-trap verification on the tool
approval modal.

### Threat-model review (summary; full record in `docs/14` §9 item 74)

Boundaries walked: local-provider endpoints, extension packages, credentials,
tool execution, workspace access, memory/privacy, diagnostics, updates,
retention, IPC. Findings fixed this sprint, each with regression tests: the
local wall's destination gate (loopback pinning, DNS-rebinding refusal,
redirect refusal — the wall's battery) and the extension manifest's
structural gates (path traversal, oversized manifests, digest-shape
enforcement — `crates/extensions` tests). Accepted residuals: stdio MCP
egress is not inspected (OS boundary, recorded since Phase 5), DNS/IP egress
beyond the loopback gate is not checked for non-local walls, and the
trust-anchor provisioning decision remains the owner's.

## Phase 9 — packaging and hardening

| Item | Status |
|---|---|
| Reproducible builds | **evidence recorded: NOT bit-for-bit reproducible** (two independent builds hash differently); locked deps + recorded toolchain are the pins |
| Signed packages/installers | **blocked for production** (no credential/bundler CLI); the fail-closed verification gates hold by construction; the release binary builds |
| Crash recovery / migrations / corrupt-state handling | complete (store quarantine, forward-only migrations, recovery — Phase 4/7) |
| Backup / restore | **complete at the store layer** (6 tests); the in-app restore flow is deferred with the recorded reason |
| Performance budgets | **complete** — measured real numbers pinned by a test (create-500 32.5 ms, ledger-2000 79.8 ms, list-500 2.0 ms, backup 3.2 ms) |
| Accessibility audit | **complete as recorded** — named-control/keyboard findings verified; screen-reader/contrast/focus-trap remain manual items |
| Threat-model review | **complete as recorded** (`docs/14` §9 item 74 + this file) — two findings fixed this sprint with regression tests |
| Documentation freeze | **complete** — the stale docs/13 §2 stack table is fixed to the verified architecture; §4.19 + the Phase 8/9 status notes written |

## Small outstanding items

| Item | Status |
|---|---|
| Stale Electron/TypeScript row, docs/13 §2 | **complete** — the table now states the verified Tauri/Rust stack with the shipped Electron stack as a labeled preservation note |
| `apps/desktop/src/components/Untitled-1` | inspected + reported: 347-byte C scratch exercise (XOR-0 string copy; a stray `3` breaks compilation), referenced by nothing — left untouched, owner's call; safe to delete |
| Native-runtime / live setup-window exercise | incomplete — recorded gaps; environment-bounded |
| Untracked docs/evidence push-policy residue on origin | owner-handled (recorded 2026-10-21, MEMORY §11) |

## Merge/push authorization

Per the sprint mandate: commits land in reviewable increments on the
`feature-final-sprint` lane. **Nothing merges to `dev` and nothing pushes without the
owner's explicit authorization** (which, this session, covered only the two Responses
slices already at `706f2ec`).
