# AGENTS.md — Operating Manual for AI Contributors

**This file is the always-loaded entry point for any AI agent working in this repository.**
It is navigational, not duplicative: it tells you *where the authority is* and *what process
to follow*. The authority itself lives in `docs/`. If this file and a doc ever disagree, **the
doc wins** — fix this file.

Companion files, all in the repository root, all short on purpose:

| File | Purpose | Read it |
|---|---|---|
| `AGENTS.md` | Process: how to work here | always, first |
| `RULES.md` | Hard constraints: what is forbidden | always, second |
| `MEMORY.md` | Durable project facts and traps | always, third |
| `CHECKLIST.md` | Pre-commit / definition-of-done gate | before every commit |
| `CLAUDE.md` | Compact core card — auto-loaded by Claude Code | automatic |

---

## 1. What this repository is

`myharness-rust` is a **documentation-and-evidence blueprint** for rebuilding Claude Desktop 3P
as a Tauri 2 + Rust desktop application. It is **not** a working clone of the product and
**not** a place to invent behavior.

Three kinds of content live here and must never be blurred together:

1. **Preservation baseline** — what the shipped app actually does, as observed. Source:
   `docs/02`–`docs/09`, `docs/11`, `docs/16`, `docs/18`, `docs/20`, `docs/features/*`,
   `docs/screens/*`. Evidence strength is marked `Verified` / `Strong inference` / `Unknown`
   and spec columns `V` / `I` / `U` (see `README.md` conventions and `docs/01` §4).
2. **Rebuild decisions** — what the new implementation *should* do. Source: `docs/13`
   (roadmap) and `docs/19` (improvement register). These are explicitly **not** claims about
   shipped behavior.
3. **Negative evidence** — the locally patched `native-adapter.cjs` (an in-process
   Anthropic→OpenAI shim with a `So → true` policy bypass). This is documented as
   `AI-ADAPTER-01..14` in `docs/12` §7 and is a **specimen of what not to build**. It may be
   kept only as a negative test fixture. Never call it "native". See `MEMORY.md` §4.

## 2. The stack, on one screen

Source of truth: `STACK.md` and `docs/21-repo-layout.md`.

- **Shell:** Tauri 2 (`tao` + `wry`), custom `app://` protocol, signed update bundles.
- **Core:** Rust — `tokio`, `serde`, `thiserror`/`anyhow`; `axum` only where a local HTTP
  boundary is genuinely justified.
- **Renderer:** React + TypeScript + Vite.
- **Providers:** **native Rust provider adapters.** No protocol translation, no inference
  gateway, no router. Each provider ships its own adapter under `crates/providers/*`.
- **MCP:** Rust MCP SDK (`rmcp`).
- **Storage:** SQLite in WAL mode; `keyring` for secrets, `secrecy`/`zeroize` for handling.
- **Terminal:** `portable-pty`.
- **Workspace:** 26 members — `crates/{agent,app-shell,artifacts,bridge,config,mcp,plugins,
  projects,protocol-core,provider-sdk,scheduler,search,secrets,sessions,shared,skills,store,
  telemetry,updates}` plus `crates/providers/{anthropic,bedrock,bedrock-mantle,foundry,
  gateway,openai}` and `apps/desktop` (renderer + `src-tauri` shell). (`providers/vertex`
  was removed from rebuild scope at the owner's direction, 2026-10-23 — `docs/04` §3.1;
  `providers/openai` was added by the open-model-layer owner decision, 2026-10-23 —
  `docs/17` §4.14.)

> **Known doc conflict:** `docs/13` §2 still prescribes Electron/TypeScript, while `STACK.md`,
> `docs/21`, the Cargo workspace, and the code are Tauri/Rust. **Follow `STACK.md` and
> `docs/21`.** The `docs/13` §2 row is a stale leftover — see `MEMORY.md` §2.

## 3. Mandatory reading order

Do not skip this. Each step states what you may conclude afterward.

| # | Read | Before you may |
|---|---|---|
| 1 | `README.md` | cite anything — it defines the doc map and the `V`/`I`/`U` vocabulary |
| 2 | `RULES.md` (this repo) | write a single line of code |
| 3 | `MEMORY.md` (this repo) | trust any project fact from memory |
| 4 | `docs/01-methodology.md` | claim anything is `Verified` |
| 5 | `docs/12-ai-architecture.md` | touch `crates/providers/*`, `crates/provider-sdk`, or any adapter code |
| 6 | `docs/14-gaps-and-unknowns.md` | decide how to handle an unobserved behavior |
| 7 | `docs/10-architecture.md` §8 | move data across a trust boundary |
| 8 | `docs/17-traceability.md` | add a feature, a spec ID, or a test |
| 9 | `docs/22-branching.md` | create a branch or commit |
| 10 | `evidence/README.md` | touch anything under `evidence/` |
| 11 | `docs/13` + `docs/19` + `docs/04` | start a phase, propose an improvement, or judge current workstream status |
| 12 | The specific `docs/features/*` and `docs/screens/*` for the area you touch | change that area's behavior |

Steps 1–3 and 9–10 are unconditional. Steps 4–8 and 11–12 are conditional on what you touch.

## 4. The task workflow

### 4.1 Start of task

1. `git switch dev && git pull` — integrate before branching (`docs/22`).
2. `git switch -c phase-<n> dev` or `feature-<slug> dev` — a disposable lane cut from `dev`.
   Never edit on `main`; never `git checkout main`.
3. Locate the spec: search `docs/features/`, `docs/screens/`, and `docs/17-traceability.md`
   for the IDs that govern the area (`UI-*`, `FEAT-*`, `ARCH-*`, `API-*`, `DATA-*`, `AI-*`).
   If you cannot find a governing spec, **that is a finding** — record it as an unknown per
   `docs/14` §9; do not proceed on intuition alone.
4. Read the governing spec *in full*. Specs carry per-row confidence; honor it.
5. State the plan, in the chat, in terms of: the spec IDs you are implementing, the modules
   you will touch, the test IDs you will satisfy, and the unknowns you are accepting.

### 4.2 During task

- **Cite the spec in code.** Every non-trivial unit in this repo carries doc references in its
  doc comments — see `crates/bridge/src/lib.rs` for the reference style (it cites `docs/18`,
  `docs/10` §8, `ARCH-PROC-03`, `TEST-ARCH-001`, `ARCH-PROC-07`). Follow it.
- **Crate stubs carry a specific scaffold comment**:
  `//! Scaffolded structure only - implementation arrives in a later roadmap phase.` plus
  `//! TODO(phase): implement this module per its roadmap phase; wire deps via the root
  Cargo.toml workspace.dependencies.` (`crates/provider-sdk/src/lib.rs`). Keep that shape.
- **Keep traceability IDs in the code**, not only in the docs (`docs/17`).
- **Match the current spec status vocabulary** when reporting work: `Available`,
  `Available, unexercised`, `Partial`, `Policy-disabled`, `Unavailable`, `Unsupported`,
  `Unknown`, `Noncompliant` (`docs/17`).
- **Never widen scope to "make it nicer".** Proposals go to `docs/19` as `IMP-*` with a
  P0/P1/P2 priority, and are labeled as proposals, not as shipped behavior.
- If you discover the spec is wrong or ambiguous, **stop and write it down** (see §6).

### 4.3 End of task

1. Run `CHECKLIST.md` in full.
2. Run the tests for what you touched: `cargo test -p <crate>`, plus
   `node tests/e2e/shell.spec.mjs` for renderer changes, plus `tsc --noEmit` and
   `vite build` for renderer changes. Fixture rules are in `tests/README.md`.
3. Update the traceability row in `docs/17-traceability.md` if a status changed.
4. Update `docs/04-progress-tracker.md` if a workstream advanced.
5. `git switch dev && git merge --no-ff <lane>`, then `git push origin dev`, then `git pull`.
   `main` is maintainer-only and never advances by agent automation.

## 5. The branch loop (verbatim from `docs/22`)

```text
1. git switch dev && git pull          # integrate latest
2. git switch -c phase-1 dev           # new lane from dev
3. code + commit locally on the lane
4. git switch dev && git merge --no-ff phase-1   # land the work
5. git push origin dev                 # publish dev
6. git pull                            # confirm clean state
7. repeat from (2) for the next slice
```

Lanes are **disposable** — if a lane rots, cut a fresh one from `dev`.

## 6. When you are wrong, or the doc is wrong

`docs/14` §9 defines the five rules for handling unknowns. In short:

| Situation | Required handling |
|---|---|
| A screen was never observed | Write an **approved decision**, label it "unobserved" |
| A provider behavior was never observed | Isolate it behind a **native adapter** and add a **wire test** |
| A capability cannot be detected | Treat it as **disabled**, never as assumed-present |
| A policy cannot be enforced | **Do not ship the toggle** |
| A platform feature is unavailable | State it as **explicitly unavailable**; do not emulate it |

A wrong guess written as fact is the single worst outcome in this repository. `Unknown` is a
legitimate, respected answer here.

## 7. Definition of done

A task is done when:

- [ ] Every changed behavior traces to a spec ID in `docs/17-traceability.md`.
- [ ] Every unknown is still labeled `Unknown` / `Strong inference` — none silently became `Verified`.
- [ ] The relevant `TEST-*` IDs are implemented and pass.
- [ ] `cargo metadata` succeeds (minimum build gate for a lane commit).
- [ ] The renderer gates pass if the renderer was touched (`tsc --noEmit`, `vite build`, E2E).
- [ ] No secret, credential, private hostname, or personal content appears in the diff
      (`evidence/README.md` rules 3–8).
- [ ] `RULES.md` has no violations.
- [ ] `docs/04` / `docs/17` are updated if status changed.
- [ ] The lane is merged into `dev` with `--no-ff`; `main` untouched.

## 8. Where to look for things

| Question | File |
|---|---|
| What does the app do? | `docs/features/*`, `docs/screens/*`, `docs/03`, `docs/05` |
| How is it structured? | `docs/10`, `docs/21`, `Cargo.toml` |
| What are the data shapes? | `docs/11`, `docs/18`, `docs/20` |
| What are the AI/provider rules? | `docs/12` (authoritative) |
| What is the build order? | `docs/13` phases 0–3 |
| What is unknown or contradictory? | `docs/14` §7 (traps), §9 (rules) |
| What is on the roadmap as a proposal? | `docs/19` |
| What must I not do? | `RULES.md` |
| What did we already learn? | `MEMORY.md` |
| What evidence backs a claim? | `docs/15`, `evidence/` |
| How do branches work? | `docs/22` |

## 9. Quick reference — ID prefixes

`UI-*` (interface) · `FEAT-*` (feature) · `ARCH-*` (architecture) · `API-*` (contract) ·
`DATA-*` (storage) · `AI-*` (provider/agent behavior) · `TEST-<AREA>-<NNN>` (tests, areas:
ARCH, UX, SESSION, AI, CFG, MCP, SEC, ART, CODE, SCHED, DATA, PRIV, UPDATE, EXT, AUTH, I18N) ·
`REQ-*` (requirement) · `IMP-*` (improvement proposal) · `AI-ADAPTER-NN` (the 14 recorded
violations of the local adapter).

---

**Rule of the repository, in one line:** *the docs are the spec; evidence is the citation;
`Unknown` is an answer; a green test that contradicts the docs is not a pass.*
