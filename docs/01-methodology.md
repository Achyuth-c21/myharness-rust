# 01 — Methodology and Evidence Model

## 1. Purpose

The investigation must support a rebuild without inventing missing behavior. Every material claim is
therefore tied to evidence and assigned a confidence level. The repository distinguishes:

- **Verified** — directly observed in the running app, an extracted shipped file, a reproducible
  runtime response, or a documented schema.
- **Strong inference** — supported by several independent observations, but not directly proven.
  The reasoning and a competing explanation are stated.
- **Unknown** — not observed. It is listed in [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md),
  not filled with a plausible-sounding answer.

A statement without an evidence reference is either a rebuild decision (explicitly labeled) or a
definition in this documentation set.

## 2. Source hierarchy

When sources conflict, the order is:

1. **Pristine shipped asar and `resources/ion-dist`.** This is the behavior the product ships.
2. **Live runtime behavior**, if it does not depend on the local inference patch.
3. **Locale strings, resource manifests, and static protocol tables** in the pristine app.
4. **Current patched runtime**, only for the patch diff and for observing the current state.
5. **Logs, configuration, and storage**, used for lifecycle and schema evidence, never for claims
   about Anthropic server behavior.
6. **UI copy alone**, used to confirm labels and disclosed behavior, not to infer hidden mechanics.

The live app is running with a non-shipped inference adapter. Therefore:

- Its model list, model IDs, “needs API key” rows, and translator behavior are **current-state
  evidence**, not shipped 3P behavior.
- Its `Gateway` provider UI is shipped.
- Its rendered screens, menus, settings, and protocol behavior are shipped unless a diff says
  otherwise.

The exact asar diff is limited to the bootstrap injection, the fetch interception in `vWt`, and a
model-policy bypass (`function So(e){return!0}`). No other shipped file differs.

## 3. Static inspection method

Static work used:

- `rg`/Python over minified JS, with context windows around a symbol or string.
- Extraction of `flatKey` configuration descriptors, provider labels, credential enums, endpoint host
  maps, and locale strings.
- Regex extraction of menu accelerators and their adjacent labels.
- Inspection of `package.json`, bundled-skill manifests, artifact libraries, and protocol schemes.
- Byte comparison of pristine and current asars, followed by context extraction at the changed
  offsets.
- JSON schema/key inspection of configuration, model-catalog sidecars, session metadata, and
  Chromium storage.

Minified symbol names are not semantic. A name such as `v2r` is reported only as an identifier; its
behavior is described from the code and runtime effects.

## 4. Runtime inspection method

Runtime work used the Chromium DevTools Protocol against the real renderer:

- Visible text and accessibility nodes for labels, roles, and menu structure.
- DOM selectors for interaction and geometry.
- Computed styles for tokens, fonts, and component states.
- Screenshots for visual evidence, not as the sole source of behavior.
- Storage key/schema enumeration without copying values.
- Controlled clicks and form edits, followed by **Discard Changes** for setup forms.

Interaction rules during investigation:

- Never apply a provider configuration, test a credential, install an update, sign out, delete an
  account, or export data without an explicit need.
- Stop any generation started accidentally.
- Prefer closing menus by toggling their trigger; React menus can stay mounted after Escape.
- Record the target URL when a result depends on which renderer was inspected.

## 5. Handling the local inference patch

The current install is **not** a pristine product. The patch consists of:

| Location | Change | Consequence |
|---|---|---|
| `index.pre.js` | `require` of `~/.config/Claude-3p/inference-bridge/bridge.cjs` before app bootstrap | Loads `native-adapter.cjs` and installs `globalThis.__claudeNative` |
| `index.chunk-Z7WxBjyE.js`, `vWt` | `globalThis.__claudeNative.shouldTranslate(...)` branch | Sends matching `/v1/messages` and `/v1/models` calls to the local adapter |
| `index.chunk-Z7WxBjyE.js`, `So` | Returns `true` for every model ID | Bypasses the shipped Anthropic model allowlist |

The adapter calls itself “native” in comments. That word is **not** used in this repository to
describe it. It is an in-process translation shim: it rewrites Anthropic Messages to OpenAI Chat
Completions, monkey-patches the fetch boundary, drops thinking/cache/tool-result image data, and
bypasses the shipped model policy. It is documented as a rejected experiment in
[12-ai-architecture.md](12-ai-architecture.md), with the deficiencies that a compliant design must
avoid.

## 6. Evidence identifiers and traceability

Documents use stable prefixes so that a requirement can be traced to a module and a test:

| Prefix | Meaning | Example |
|---|---|---|
| `UI-*` | screen or component requirement | `UI-COMPOSER-04` |
| `FEAT-*` | user-facing feature | `FEAT-SCHED-02` |
| `ARCH-*` | process, IPC, or protocol requirement | `ARCH-PROTO-03` |
| `API-*` | API/IPC contract | `API-CUSTOM3P-07` |
| `DATA-*` | persistence requirement | `DATA-SESSION-02` |
| `AI-*` | provider/model capability requirement | `AI-PROVIDER-05` |
| `TEST-*` | acceptance test | `TEST-CFG-003` |

[17-traceability.md](17-traceability.md) maps feature → documentation → rebuild module → test.
[13-implementation-roadmap.md](13-implementation-roadmap.md) defines the test cases.

## 7. Confidence conventions in feature/screen specs

Feature and screen specifications use tables with a `Confidence` column:

- `V` — verified in shipped source or live shipped UI.
- `I` — strong inference; reasoning is in the paragraph or footnote.
- `U` — unknown, intentionally unspecified.

A screen that could not be reached is not described visually. Its existence may be recorded from a
route string, flag, or menu entry, but its layout is marked unknown.

## 8. Known sources of ambiguity

1. **Feature flags can hide shipped code.** A supported gate does not prove every sub-surface is
   reachable in this deployment.
2. **The setup window is separate.** Its renderer URL is `app://localhost/setup-desktop-3p`; it can be
   selected accidentally when the main target is omitted.
3. **Server responses are synthesized.** In 3P mode, `/api/bootstrap` and related endpoints are local
   stubs or overlays, not Anthropic server behavior.
4. **The model catalog is signed and hosted.** It is a download artifact, not application source.
5. **The current provider profile is local.** Its gateway points at a local adapter and its model IDs
   are prefixed; neither is part of the shipped contract.
6. **Screenshots contain personal data.** They are evidence, not publishable assets.
7. **MCP failure states are environment-specific.** A failed `mcp-remote` fetch is recorded as an
   observed error state, not a universal product behavior.
8. **No source maps.** The rebuild cannot preserve internal function boundaries; it preserves
   externally observable behavior and contracts.

## 9. Definition of “complete” for this pass

A feature is documented when:

- Its entry point, primary states, and exit paths are known.
- Its data ownership and persistence location are known or explicitly unknown.
- Its provider/MCP/tool dependencies are known or explicitly unknown.
- Its error, empty, loading, and permission states are described where observed.
- Its rebuild module and at least one acceptance test are assigned.

It is not required to reproduce Anthropic’s private backend. The rebuild must instead expose a clean
provider boundary and reproduce the app-side contracts documented in
[12-ai-architecture.md](12-ai-architecture.md) and [18-api-contracts.md](18-api-contracts.md).
