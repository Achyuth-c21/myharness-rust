# CHECKLIST.md — Pre-Commit Gate

Run this before every commit on a lane, and before merging a lane into `dev`. It is derived
from `RULES.md` and the source docs; it is the short, executable form of them.

If an item cannot be satisfied, **stop and report it** — do not tick it optimistically.

---

## A. Spec and traceability

- [ ] Every changed behavior traces to a spec ID (`UI-*`, `FEAT-*`, `ARCH-*`, `API-*`, `DATA-*`, `AI-*`)
      listed in `docs/17-traceability.md`.
- [ ] The governing `docs/features/*` and `docs/screens/*` files were read **in full** before coding.
- [ ] New code carries doc citations in its doc comments, in the style of `crates/bridge/src/lib.rs`.
- [ ] The roadmap traceability comments are still in place (`docs/22` requirement for lane commits).
- [ ] If a status changed, the `docs/17-traceability.md` row is updated.
- [ ] If a workstream advanced, `docs/04-progress-tracker.md` is updated.
- [ ] No proposal (`IMP-*`) was implemented without being labeled a proposal.

## B. Evidence honesty

- [ ] No invented behavior: every claim is `Verified`, `Strong inference`, `Unknown`, or `Improvement`.
- [ ] No confidence level was upgraded without new evidence.
- [ ] No `docs/13` decision and no `docs/19` proposal is presented as shipped behavior.
- [ ] Every remaining unknown is still explicitly labeled `Unknown`.
- [ ] Any unobserved behavior I implemented has the `docs/14` §9 handling (approved decision
      labeled "unobserved", or native adapter + wire test).

## C. Provider and agent layer

- [ ] No global `fetch` monkey-patch or global network primitive was touched.
- [ ] No cross-protocol translation, proxy, router, or inference gateway was introduced.
- [ ] No silent model substitution; model identity = provider + native model id + variant + profile/config revision.
- [ ] No policy bypass, and specifically **no `return true` escape hatch** in any policy path.
- [ ] Unknown capability is treated as unsupported/disabled, never as assumed-present.
- [ ] Reasoning blocks, tool-result images, and multimodal attachments are never silently dropped.
- [ ] No attachment content is uploaded for telemetry.
- [ ] Token counts come from the provider-native tokenizer, not an estimate, when the provider reports exact usage.
- [ ] Native stop reasons are preserved in streaming.
- [ ] Tool output, file content, web content, and MCP responses are treated as **data, never instructions**.
- [ ] Nothing was copied from `native-adapter.cjs`; it remains a negative fixture only.

## D. Security and trust boundaries

- [ ] The renderer still has **no Node surface** (`typeof process === 'undefined'`).
- [ ] The renderer crate still does not link `crates/secrets` or `crates/providers/*`; all
      privileged operations cross `crates/bridge` (`ARCH-PROC-02/03/05`).
- [ ] The `app://` protocol guards hold: wrong host → 404, bad `Origin` → 403, traversal → 403,
      CSP injected, API paths → typed 503 with the `custom_3p_not_available` body.
- [ ] CSP was not relaxed; no `unsafe-inline` was reintroduced.
- [ ] Every trust boundary I crossed (`docs/10` §8) has its required control present in the diff.
- [ ] Secrets go through `crates/secrets` + `keyring`, wrapped in `secrecy`/`zeroize`; no secret
      value is logged, returned, or embedded.

## E. Secrets, evidence, publication

- [ ] No API key, OAuth token, cookie, auth header, private MCP host, VPC/Tailscale name, or
      absolute home path appears in the diff.
- [ ] No personal content — task titles, session text, usage totals, screenshots, raw logs —
      appears in the diff or in fixtures.
- [ ] `evidence/app-asar-orig/` was **not** force-added.
- [ ] The private archive was not packaged, linked, or committed.
- [ ] Any newly published structured evidence file has a `manifest.json` entry with SHA-256.
- [ ] A secret scan was run before publishing.

## F. Tests and build

- [ ] `cargo metadata` succeeds.
- [ ] `cargo test` passes for every crate I touched.
- [ ] New provider adapters have **wire tests** against mock servers (`tests/wire/`).
- [ ] Renderer changes: `tsc --noEmit` clean, `vite build` succeeds, `node tests/e2e/shell.spec.mjs` passes.
- [ ] Fixtures contain no real credentials, no user session content, no provider secrets.
- [ ] Every new test follows `TEST-<AREA>-<NNN>` with the correct area family.
- [ ] I checked that no passing test contradicts the docs — a green test that contradicts the
      spec is not a pass.
- [ ] The report states **real** counts and **real** environment gaps (e.g. missing system libs),
      not idealized ones.

## G. Repository mechanics

- [ ] I am on a `phase-*` / `feature-*` lane cut from `dev` (or on `dev` itself for a doc-only change).
- [ ] I did **not** push to `main`, checkout `main` for edits, merge into `main`, or force-push.
- [ ] Dependencies, if any, were added to the root `Cargo.toml` `workspace.dependencies` and
      referenced from there.
- [ ] The crate layout still matches `docs/21` (25 members); if I changed it, `docs/21` was updated.
- [ ] `git status` shows only intended files.

## H. Report

- [ ] Status words come from the `docs/17` vocabulary, used exactly (`Available`,
      `Available, unexercised`, `Partial`, `Policy-disabled`, `Unavailable`, `Unsupported`,
      `Unknown`, `Noncompliant`).
- [ ] I distinguished **not implemented** / **not verified** / **not observed**.
- [ ] Every deviation I introduce is named with its spec ID and doc section.
- [ ] Open questions and unknowns are listed explicitly, with the reason they remain open.

---

**Final sanity check:** would a reviewer who has never met me be able to read the diff and the
linked doc IDs and understand *why* the code is the way it is? If not, the code comments are
not finished.
