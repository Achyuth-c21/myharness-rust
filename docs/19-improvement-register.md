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

### IMP-06 — MiniMax M3.1 Flash as the transparent reconstruction default (P1)

**Baseline:** current live UI selects MiniMax M3.1 Flash through an OpenRouter-style gateway and the
patch.

**Proposal:** keep MiniMax M3.1 Flash (free) as the preferred default for reconstruction, but label it
as MiniMax, use its native model ID, store the provider profile explicitly, and never present it as a
Claude model. No policy bypass.

**Compatibility:** visible default remains, but provider identity and request shape are correct.
**Tests:** `TEST-AI-024`, `TEST-UX-002`.

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
