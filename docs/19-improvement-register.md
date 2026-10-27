# 19 — Improvement Register

This register lists **proposed changes** to the rebuild. None are claims about shipped behavior. Each
item states the observed baseline, the proposal, compatibility impact, and tests. Items marked
**P0** must be closed before a public rebuild; **P1** before beta; **P2** can follow.

The preservation baseline is [03](03-feature-inventory.md) plus the feature/screen specs. If an item
changes observable behavior, it must be accepted explicitly and documented as an improvement.

## 1. Architecture and provider layer

### IMP-01 — Replace the local translation adapter with native adapters (P0)

**Baseline:** the inspected install contains a 1,001-line in-process Anthropic→OpenAI translator that
loses thinking/cache data, tool-result images, and model validation, and is described inaccurately as
“native.” It is negative evidence only.

**Proposal:** remove it entirely. Implement one adapter per provider/protocol using the official SDK or
native HTTP contract, emitting the internal `TurnEvent` stream. No fetch interception, no generic
router, no cross-protocol request rewriting.

**Compatibility:** behavior intentionally changes: unsupported providers become explicit adapters
or are unavailable; no silent translation. **Tests:** `TEST-AI-022/023/024`, `TEST-EXT-003/004`.
**Done when:** static checks find no global fetch patch and each shipped provider has a wire fixture.

### IMP-02 — Make model policy explicit and per-provider (P0)

**Status (2026-10-23): superseded in part by the open model layer** — the owner removed
family-level model-ID policy outright (`docs/14` §9 item 29; `docs/17` §4.14); ids travel
verbatim and the provider's own wire error is the refusal. What stands: the *never a
`return true` escape hatch* clause — no rebuild validator has ever had one, and the
remaining shape checks are honest shape checks. The capability/policy registry for
*capabilities* (vision, tools, context limits — IMP-03) remains a proposal.

**Baseline:** shipped policy accepts only Anthropic-family IDs; the live patch bypasses it with
`function So(e){return!0}`. A policy bypass is not an extension mechanism.

**Proposal:** a capability/policy registry keyed by provider + model. Claude families keep the
shipped Anthropic-only rules; new native providers get their own validators and capability metadata.
Blocked models are absent or disabled with a reason. No `return true` escape hatch.

**Compatibility:** non-Anthropic models require adding a documented provider adapter, not a patch.
**Tests:** `TEST-AI-007/008/024`, `TEST-CFG-004`, `TEST-EXT-008`.

### IMP-03 — Capability-first UI (P0)

**Baseline:** some controls are visible while their gates are unavailable, and capability data is
sparse; unknown capability tends to be treated optimistically in places.

**Proposal:** one typed `ModelCapabilities` object drives tools, vision, PDF, thinking, cache, effort,
1M, fast mode, structured output, and context limits. Unknown = unsupported. The UI shows why a
control is disabled.

**Compatibility:** fewer controls on unknown models; no silent feature failure. **Tests:** `TEST-AI-001..012`,
`TEST-UX-002/013`.

### IMP-04 — Signed, lazy extension packages (P0)

**Baseline:** extension gates and manifests exist, but the rebuild has no supply-chain contract.

**Proposal:** provider/SDK extensions are signed packages with a manifest, pinned SDK/integrity,
declared egress, credential schema, min version, and conformance suite. Lazy-load so one broken
package cannot prevent boot.

**Compatibility:** providers cannot register by mutating internals. **Tests:** `TEST-EXT-001..008`,
`TEST-ARCH-001`.

### IMP-05 — Add native protocol families without a universal translator (P1)

**Baseline:** six Anthropic-family providers; no native OpenAI/Gemini/MiniMax adapters.

**Proposal:** add OpenAI Responses, Gemini generateContent, MiniMax native (default), and a local
Ollama adapter as separate packages. Each has native request/stream/error mapping and its own model-ID
validation.

**Compatibility:** new providers require explicit selection; no model-name heuristics. **Tests:**
`TEST-EXT-003/005/006`, all applicable `TEST-AI-*`.

**Status (2026-10-23): partially realized by the open model layer** — the OpenAI Chat
Completions wire landed as one native wall serving the `openai` and `custom` families
(`crates/providers/openai`; `docs/17` §4.14). The "its own model-ID validation" clause is
superseded: family gates were removed by owner decision (`docs/14` §9 item 29). OpenAI
*Responses*, Gemini, MiniMax-native, and Ollama remain proposals.

### IMP-06 — MiniMax M3.1 Flash as the transparent reconstruction default (P1)

**Baseline:** current live UI selects MiniMax M3.1 Flash through an OpenRouter-style gateway and the
patch.

**Proposal:** keep MiniMax M3.1 Flash (free) as the preferred default for reconstruction, but label it
as MiniMax, use its native model ID, store the provider profile explicitly, and never present it as a
Claude model. No policy bypass.

**Compatibility:** visible default remains, but provider identity and request shape are correct.
**Tests:** `TEST-AI-024`, `TEST-UX-002`.

### IMP-18 — The open model layer: any model, any provider, custom OpenAI-compatible endpoints (P0, owner-approved 2026-10-23)

**Status (2026-10-23): approved and implemented** on the `phase-3` lane (commits
`6e7a738`, `8a3d1b8`; state record `MEMORY.md` §7d); `docs/17` §4.14, `docs/14` §9
items 29–32, `docs/13` Appendix C. What this entry records as decided then shipped:

- The harness takes **any model id any provider serves**: family-level model-ID gates
  were removed everywhere; `validate_open_model_id` checks shape only; a nonconforming
  id is the provider's own wire error to reject (verbatim message; no invented refusal).
- One native wall for the OpenAI Chat Completions wire
  (`crates/providers/openai`) serving two families: first-party `openai` and the
  user-defined `custom` endpoint (base required, credential OPTIONAL — a keyless send
  with no `authorization` header; helper-script kind allowed).
- Bridge: one shared probe lane for both families with the optional-credential
  resolver; discovery command additive (`setup_discover_openai_models`); renderer
  family picker on the Connection tab; probe/discover unlocked for the two families.
- `mantle`/`foundry` adapters remain Phase-3 road-map work; probe stays locked (never
  faked) until they land.

**Compatibility:** none broken — the three original families keep their lanes; the
superseded policy paragraphs are annotated in IMP-02/IMP-05. **Tests:** the
`crates/providers/openai` 21-fixture wire battery, the bridge lane tests, the
`TEST-AI-007` flips (§4.10–§4.12), `TEST-CFG-004/123/124` amendments.

## 2. Security and privacy

### IMP-07 — Keychain-only secrets with opaque renderer handles (P0)

**Baseline:** credentials can exist in profile JSON; the setup UI supports secret fields and helper
scripts. Some profile fields are readable on disk.

**Proposal:** store all secrets in the OS keychain; the renderer receives only handle IDs and presence
flags. Helper output is never logged; process env is scrubbed on exit. Export is redacted by default.

**Compatibility:** headless Linux needs a documented fallback (mode-0600 file + warning). **Tests:**
`TEST-CFG-006`, `TEST-AUTH-003/004`, `TEST-DATA-009`, `TEST-PRIV-004`.

### IMP-08 — Enforce egress at connection time, not only in the UI (P0)

**Baseline:** egress allowlist exists, active profile uses `*`, and MCP/browser paths have several
entry points.

**Proposal:** central egress broker with host allowlist, HTTPS-only by default, DNS/IP re-check after
resolution, block link-local/metadata/loopback unless explicitly allowed, no redirects to unapproved
hosts, per-server OAuth isolation. Deny before the socket is opened.

**Compatibility:** wildcard profiles must be visibly marked unsafe. **Tests:** `TEST-MCP-007`,
`TEST-SEC-003`, `TEST-CFG-008`, `TEST-EXT-006`.

### IMP-09 — Dedicated artifact/document origin with no bridge (P0)

**Baseline:** artifact viewer pane is unavailable, but library/capture and rendering libraries exist.
The main SPA and setup window share `app://localhost`.

**Proposal:** serve artifact/document previews from a dedicated origin/partition with a strict CSP,
sandboxed iframe, no `window.claude`, no top navigation, no same-origin storage, and a permission-free
network by default. PDFs/images use the same isolation.

**Compatibility:** some inline conveniences are lost; security boundary is explicit. **Tests:**
`TEST-ART-005`, `TEST-ARCH-006`, `TEST-SEC-001`.

### IMP-10 — Smaller, typed IPC surface (P0/P1)

**Baseline:** 17 settings + 60 web namespaces with ~1,160 methods, minified event payloads, and a
rotating channel token. Main-side sender validation is not exhaustively proven.

**Proposal:** versioned, typed contracts; allowlisted channels; generated client; main-side sender
checks; payload caps; no arbitrary paths/URLs/commands; error envelope without raw provider bodies.

**Compatibility:** internal only. **Tests:** `TEST-ARCH-001/003/004`, `TEST-SEC-001/003`.

### IMP-11 — Write-time redaction and privacy budgets (P0)

**Baseline:** logs may include paths/hosts/headers; telemetry has content-capture controls. Diagnostic
bundles are a separate flow.

**Proposal:** structured logging with field-level redaction at write time; deny-list for auth/cookies/
tokens; prompt/completion content never logged; bounded retention; support bundle enumerates files and
requires consent. Add tests that scan emitted logs/fixtures for secrets.

**Compatibility:** less debug detail by default; a local opt-in debug mode with explicit warning.
**Tests:** `TEST-PRIV-001..004`, `TEST-DATA-009`, `TEST-CFG-007`.

### IMP-12 — Artifact/session deletion and legal-hold correctness (P1)

**Baseline:** retention and `sessionRetentionHold` exist; artifacts/versions/previews create additional
copies.

**Proposal:** one retention engine covering metadata, transcript, uploads, outputs, artifact versions,
thumbnails, preview cache, and indexes; legal hold blocks GC and is auditable; tombstones for sync.
**Tests:** `TEST-DATA-004/005`, `TEST-ART-006`, `TEST-SESSION-004`.

## 3. Reliability and data

### IMP-13 — SQLite/WAL instead of many JSON files (P1)

**Baseline:** JSON/NDJSON stores for sessions, usage, schedules, config; writes are not shown to be
transactional across files.

**Proposal:** SQLite WAL for index/usage/schedules with migrations and atomic transactions;
content-addressed blobs for transcripts/artifacts. Preserve export/import semantics and redacted
backups.

**Compatibility:** migration from existing JSON with dry-run and backup; keep read-only export.
**Tests:** `TEST-DATA-001..003/008/010`, `TEST-SESSION-002/003`.

### IMP-14 — Recoverable sessions and idempotent cancellation (P1)

**Baseline:** stop/cancel and restore exist; crash behavior during streaming was not exhaustively
tested. Duplicate live processes are a concern after restart.

**Proposal:** explicit turn state machine, idempotency keys, one provider stream per turn, orphan
detection, partial-output preservation, and a recoverable “resume/continue” action that does not
replay a completed tool call.

**Compatibility:** stopped turns are visibly partial. **Tests:** `TEST-SESSION-001/002/003`,
`TEST-AI-015/016`.

### IMP-15 — Provider/model discovery resilience (P1)

**Baseline:** gateway discovery is 3 pages/10 s with partial-on-later-error; catalog has signature/
fallback rules. The picker in the patched build exposes 522 rows without search.

**Proposal:** retain the discovery contract, add ETag/cache, explicit first-page failure, per-provider
health, and a curated picker with search/grouping/recent models. Never retry discovery against a
different host.

**Compatibility:** search/grouping is an improvement, not observed behavior. **Tests:** `TEST-CFG-011/012`,
`TEST-AI-017/018`, `TEST-UX-013`.

### IMP-16 — Non-blocking setup and provider boot (P1)

**Baseline:** separate setup window survives provider boot failure; health/recheck/tainted states exist.

**Proposal:** always open the main shell with a clear degraded banner and a setup action; no modal
loop; no blank window; active profile remains unchanged until a valid apply.

**Compatibility:** main UI may show fewer models until setup is fixed. **Tests:** `TEST-CFG-001/010`,
`TEST-ARCH-007`, `TEST-UX-009`.

### IMP-17 — Scheduler truthfulness and recovery (P1)

**Baseline:** local-only schedules, keep-awake, skip records and approval lifetime exist; run history/
editor were not exercised.

**Proposal:** make “runs only while awake and online” prominent; record skip reasons; retry failed
runs idempotently; never claim cloud execution; preserve history on disable; timezone/DST tests.

**Compatibility:** jobs missed while asleep remain skipped unless a catch-up policy is chosen and
documented. **Tests:** `TEST-SCHED-001..006`, `TEST-UX-006`.

## 4. UX and accessibility

### IMP-18 — Searchable, grouped model picker (P1)

**Baseline:** captured picker is a flat 522-item menu plus a 432-item flyout with no search; model
identity is truncated.

**Proposal:** searchable by display/provider/native ID; group by provider/family; pin current/recent;
show credential and capability badges; full ID on focus; no automatic selection. Keep a “More models”
path for the long tail.

**Compatibility:** changes the observed flat menu; justified by usability. **Tests:** `TEST-UX-002/013`,
`TEST-AI-001..010`.

### IMP-19 — One-popover-at-a-time composer discipline (P1)

**Baseline:** sequential automation left the model, add, and skill/connector menus mounted together;
the product may allow stacking, but the evidence is contaminated by the capture method.

**Proposal:** close the previous composer popover when a new trigger opens; one Escape closes the
topmost; outside click closes all; focus restoration. **Compatibility:** clearer layering; no data loss.
**Tests:** `TEST-UX-001/013`.

### IMP-20 — Accessible menus, dialogs, and streaming (P1)

**Baseline:** DOM shows `role=menu`/`menuitemradio`, but tooltips may be hover-only, focus restoration
is not proven, and streaming announcements risk verbosity.

**Proposal:** complete combobox/menu/tab/dialog semantics, focus traps/restores, `aria-activedescendant`,
radio/switch states, reduced-motion support, one polite streaming status per phase, accessible
heatmap/table alternatives, IME-safe Enter. **Compatibility:** visual behavior unchanged.
**Tests:** `TEST-UX-012/013/014`, `TEST-I18N-004`.

### IMP-21 — Model identity and session-change warnings (P0/P1)

**Baseline:** changing the global default or model can be ambiguous; transcripts retain the model per
turn but the UI does not always emphasize a mid-session switch.

**Proposal:** show provider + model + effort at every send point; on mid-session change, show an inline
marker and require confirmation if the new model lacks capabilities used earlier. No silent fallback.

**Compatibility:** adds friction; prevents wrong-model output. **Tests:** `TEST-AI-008/024`,
`TEST-UX-002/012`.

### IMP-22 — Empty/error/policy states are distinct (P2)

**Baseline:** several screens have good empty states, but unavailable/error/empty can look alike.

**Proposal:** a shared state taxonomy: loading, empty, no-results, unavailable, unsupported,
policy-disabled, error, offline. Every state has copy, an action where safe, and no fake data.
**Compatibility:** copy changes only. **Tests:** `TEST-UX-004..008/013`, `TEST-MCP-009`.

### IMP-23 — Localization completeness and locale metadata (P2)

**Baseline:** 35,441 English strings; 11 locales offered; some product copy may remain untranslated.

**Proposal:** catalog keys with ICU plurals, locale-aware dates/numbers, autonyms with `lang`, fallback
to English, no raw IDs. Do not machine-translate at runtime. **Compatibility:** none.
**Tests:** `TEST-I18N-001..004`.

## 5. Operations and lifecycle

### IMP-24 — Update safety and rollback (P1)

**Baseline:** update offered but not applied; enforcement window and ancient journal exist.

**Proposal:** signed feed, signature/version floor, staged download, atomic install, rollback to the
previous version, session guard, no update while a provider stream/PTY is active without consent.
**Compatibility:** may delay updates. **Tests:** `TEST-UPDATE-001/002`, `TEST-ARCH-008`.

### IMP-25 — Support bundle that is safe by construction (P2)

**Baseline:** support bundle action exists; privacy copy says it may be sent explicitly.

**Proposal:** local preview before upload, per-file inclusion, secret scan, redacted logs/config,
size cap, explicit destination. Never auto-upload. **Compatibility:** more steps. **Tests:**
`TEST-DATA-009`, `TEST-PRIV-003/004`.

### IMP-26 — Platform capability matrix instead of dead controls (P2)

**Baseline:** gates such as `artifactsPane`, `nativeQuickEntry`, `coworkBrowser`, `computerUse`,
iOS Simulator, WSL, self-hosted sessions, `chatTab`, and `yukonSilver` are unavailable here while some
settings remain visible.

**Proposal:** ship a per-platform capability manifest; hide/disable with a reason; never show a toggle
that cannot work. Keep the evidence-backed status visible in About/Diagnostics. **Compatibility:** fewer
controls on unsupported platforms. **Tests:** `TEST-ARCH-007`, `TEST-UX-013`, `TEST-SEC-004`.

### IMP-27 — Conformance fixtures as release gates (P0)

**Baseline:** documentation is evidence-rich but the inspected app has no source maps/tests.

**Proposal:** every provider/MCP/storage contract gets a fixture; CI blocks release on missing native
wire tests, secret scans, accessibility checks, and traceability rows. Use synthetic data only.

**Compatibility:** none. **Tests:** all suites; definition of done in [13](13-implementation-roadmap.md).

### IMP-28 — One role, one glyph, one label per title-bar control (P2)

**Baseline:** the title-bar left cluster holds four controls, but only one was live: the panel toggle
was inert and the three-line control carried the sidebar-collapse function, so the glyph a user reads
as "menu" performed a different action than the glyph beside it. `docs/07` §1.5 leaves exact codepoints
unpinned, so the pairing was decided by reading, not by spec.

**Proposal:** publish a title-bar control inventory — each control has exactly one role, one distinct
glyph, and an accessible label that names the role; a control with no live target is disabled with a
discoverable reason (a tooltip on a native `disabled` button never renders, so use an
`aria-describedby` note or a non-disabled wrapper). Reconcile the rebuild against the shipped app's own
chrome before any further reference capture is read. **Compatibility:** none; the two left controls
keep their positions and glyphs and swap only their roles. **Tests:** `TEST-UX-013`.

### IMP-29 — Global shortcuts stay scoped while typing (P2)

**Baseline:** `docs/08` UI-INT-001/020/023 name the shortcuts but pin no focus-target rule, so the
rebuild fired `Ctrl+B` / `Ctrl+K` / `Ctrl+,` even while the user typed in the composer
(`report-bug-hunt-phase1.md` C9, `Improvement`).

**Proposal (adopted for the composer, phase-1):** while the event target is editable (text field /
textarea / contenteditable) and no overlay is open, every global shortcut stays inert **except**
`Ctrl+N`, which `docs/08` UI-INT-005 defines as an explicit, unconditioned New invocation. Overlays
(settings, palette) keep the unscoped behavior their flows rely on. Spec lines in `docs/08` are
unchanged — this is a renderer-scope rule, not a spec rewrite. **Tests:** T05, T13, T19, T27 still
green; verified in `tests/report-bug-hunt-phase1.md` §F.

### IMP-30 — Native menu surface for accelerator parity (P2)

**Baseline:** `docs/06` §7 extracted the shipped app's native menu accelerators (`Verified` on the
shipped bundle). The rebuild's shell is frameless (`docs/05`/`docs/07` geometry), so attaching a
GTK/muda menubar to carry accelerators would render a visible menu strip and break the captured
home geometry. `docs/13` §4 lists "native menu accelerators" under Phase 1 — carried here as the
deferred decision instead.

**Current state (Phase 1):** shell accelerators are implemented in-app (Ctrl+N, Ctrl+,, Ctrl+K,
Ctrl+B; battery T05/T13/T19/T27); text-edit accelerators (Z/X/C/V/A in editable surfaces) are
WebView-native in WebKitGTK; the remainder (Ctrl+/, Ctrl+\, Ctrl+;, Ctrl+Shift+T, open file/folder,
F1, find/zoom roles, Ctrl+W/Q window roles) arrive with the features or menu surface they trigger.

**Proposal:** when a menu surface exists in the product (or at Phase 9 packaging), attach a hidden
native menu carrying the remaining documented accelerators without touching frameless geometry;
turn the deferral off. Native window roles (Close/Quit) land with it as bridge commands with
`docs/18` rows. **Tests:** extend the native runtime fixture with per-accelerator assertions.

## 6. Phase 5 follow-ups

These are proposals, not claims about shipped behavior. Phase 5's local/stdio path is implemented
(`docs/17` §4.16); the items below close its conformance and UX gaps.

### IMP-31 — Live HTTP MCP and OAuth conformance fixtures (P1)

**Baseline:** the manager is tested with a scripted connector, a real stdio fixture, and an
injected OAuth HTTP transport. No live Streamable HTTP server, redirect, or keychain-backed OAuth
flow is exercised end-to-end.

**Proposal:** add a local HTTP MCP fixture (including a redirect case) and a synthetic OAuth token
server, then run the real transport and keychain store in an integration lane. Keep credentials
synthetic and per-test. **Compatibility:** none. **Tests:** `TEST-MCP-001/006/007`.

### IMP-32 — Model-driven tool discovery (P1)

**Baseline:** tool search is a deterministic UI/developer surface. A model receives only the
session's frozen tool list; it cannot request a schema or a tool that was not pre-selected.

**Proposal:** add an explicit, policy-gated discovery step (or a bounded meta-tool) that returns
schemas for live tools on demand, with the same namespacing, approval, and audit rules as normal
calls. Never auto-execute a discovered tool and never mutate a frozen snapshot mid-turn.
**Compatibility:** more tool declarations may appear on demand. **Tests:** `TEST-MCP-002/004/008`.

### IMP-33 — Persistent, actionable MCP failure surface (P1)

**Baseline:** per-server failures are recorded, status events and a Developer retry exist, but no
persistent toast/action survives a screen change. `startupFailureToast` is config only.

**Proposal:** one durable failure record per server with a retry action and a dismiss that does not
hide the underlying state; never block boot. **Compatibility:** adds a visible notice.
**Tests:** `TEST-MCP-009`, `TEST-UX-013`.

### IMP-34 — Plugin signature verification and marketplace (P0/P1)

**Baseline:** plugins are local-only; `Required` refuses every install because verification is not
implemented, and `Optional` reports `Unverified`. There is no marketplace or `.dxt`/`.mcpb`
installer.

**Proposal:** define a signed manifest/integrity format, verify before install, pin publisher
identity, and only then offer a marketplace. Until then, keep the current honest refusal and
labels. **Compatibility:** required-signature installs become possible only after verification
lands. **Tests:** `TEST-EXT-001/002/006/008`.

### IMP-35 — Approval prompt durability and per-session tool turns (P1)

**Baseline:** a pending approval is an in-memory oneshot tied to the current renderer. Navigating
away can lose it; one global approval mutex serializes tool turns across sessions.

**Proposal:** persist pending prompts with an expiry, re-emit them when the session reopens, and
scope the approval lock per session (or per approval key) so unrelated sessions can run in
parallel. **Compatibility:** approvals may survive navigation; tool turns are less serialized.
**Tests:** `TEST-MCP-003`, `TEST-AI-019`.

### IMP-36 — DNS/IP egress checks and custom-transport redirects (P0)

**Baseline:** the egress allowlist checks the configured host before connect. The pinned `rmcp`
Streamable HTTP client follows only same-origin 307/308 redirects and refuses cross-origin hops;
the production OAuth refresh client applies the allowlist and disables redirects. DNS/IP
resolution and link-local/metadata targets are not checked, and a custom transport client would
bypass the redirect guarantee.

**Proposal:** add DNS/IP checks for link-local/metadata/loopback unless explicitly allowed; if a
custom transport client is ever supplied, re-run the allowlist on every hop. Keep stdio's
non-inspection explicit. **Compatibility:** some local/metadata hosts stop working unless allowed.
**Tests:** `TEST-MCP-007`, `TEST-AI-020`.

### IMP-37 — Complete tool schema validation and audio results (P2)

**Baseline:** the validator covers a bounded JSON Schema subset; `pattern`, `format`, and external
`$ref` are not enforced. MCP audio results are explicit errors because the provider SDK has no
audio part.

**Proposal:** add the missing keywords behind the same bounded, fail-closed validator, and carry
audio through the provider SDK only when a provider wire can actually accept it. Until then, keep
the explicit errors. **Compatibility:** stricter argument rejection; audio remains unsupported on
wires without a slot. **Tests:** `TEST-MCP-008/010`, `TEST-AI-019`.

### IMP-38 — Code terminal, worktrees, remote placement, and Fast mode (P1/P2)

**Baseline:** Phase 6 Slice 1 implements the Code dashboard and local folder-backed sessions,
reusing the Cowork transcript/tool/approval surface. There is no PTY/terminal, worktree/GitHub
metadata, SSH/WSL/remote placement, or capability-gated Fast mode; the composer shows Fast as
visibly disabled and local is the only placement.

**Proposal:** add each surface only behind its own policy/capability gate and native adapter:
a bounded PTY lifecycle that drains on stop/quit, worktree metadata that never deletes
uncommitted work, an explicit remote placement chooser with no local fallback, and Fast mode
that is enabled only when the selected model declares support and never changes model identity.
Until then, keep the controls disabled/explicit and keep the deferred test IDs unimplemented.
**Compatibility:** adds native/terminal permissions and remote transport. **Tests:**
`TEST-CODE-003/005/006/007/008`, `TEST-SESSION-010`.

### IMP-39 — Accessible tertiary text without erasing the observed token (P1)

**Baseline:** the whole-application campaign's phase 1 (`docs/17` §4.20, finding `WA-02`) ran
axe-core (WCAG 2.0/2.1/2.2 A+AA) over nine routes in light and dark: 10 serious `color-contrast`
violations across 33 nodes. Every light-mode finding is the preserved token
`--mh-text-3: #898781` — documented in `docs/07` as the observed tertiary text/icon value — on
`#fcfcfb`/`#f3f3f3` surfaces at 3.23–3.59:1 for 12 px text (`.tasks-title`, `.profile-provider`,
`.settings-group-label`, `.error-code`); `.code-placement` is 4.1:1 on the dark `#282827` surface.
AA requires 4.5:1 for text below the large-text threshold; the 3:1 non-text threshold does not
apply to these labels.

**Proposal:** treat this as a preservation-vs-accessibility conflict, not a rebuild defect, and
let the owner choose explicitly:

- **(a) preserve** the observed token and record an AA exception for tertiary chrome text;
- **(b) darken** the light-mode value (and lift the dark surface value) until every text use is
  ≥4.5:1, registered as a deliberate deviation from `docs/07`; or
- **(c) split the token**: keep `#898781` only for icons/non-text (3:1 requirement) and introduce a
  text-safe tertiary token for labels.

Option (c) preserves the most observed evidence. Whichever is chosen, the token decision is
recorded in `docs/07` and `tests/e2e/a11y.spec.mjs` is the gate. **Compatibility:** visual only;
no behavior or data change. **Tests:** `TEST-UX-012/014`, `tests/e2e/a11y.spec.mjs`.

**Decision and status (2026-10-27): implemented — option (c).** `--mh-text-3` now carries the
computed AA derivatives (`#6f6e68` light, worst case 4.61:1 on `#f3f3f3`; `#94928c` dark,
worst case 4.74:1 on `#282827`). The observed sample `#898781` is retained as `--mh-icon-3` and
applied to icon-only selectors (`.profile-chevron`, `.palette-glyph`, `.project-chip
svg:last-child`, `.task-icon`, `.active-chevron`, `.session-title svg`, `.code-session-chevron`,
`.filter-row-chevron`), where the WCAG 3:1 non-text threshold applies (3.24:1 light, 4.11:1
dark). `docs/07` §1.2 records the extension as a rebuild decision, not an observed value. The
gate re-ran clean: **18/18 route/mode scans, 0 serious/critical**. `WA-02` is closed.
