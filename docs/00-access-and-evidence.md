# 00 — Access, Environment, and Evidence

**Status:** complete for the Linux/X11 3P installation under inspection.

This document records what was inspected, how it was inspected, and what can and cannot be concluded.
It is deliberately separate from the specifications so that evidence can be re-checked without
trusting a summary.

## 1. Installation under inspection

| Field | Value | Confidence |
|---|---|---|
| Product | Claude Desktop (`@ant/desktop`) | Verified (`package.json`) |
| Version | `2.42304.0` | Verified (package and Settings → System) |
| Deployment mode | `3p` (third-party inference) | Verified (config, runtime flags, UI copy) |
| Electron | `44.7.2` | Verified (runtime) |
| Chromium | `152.0.7977.149` | Verified (runtime) |
| Node | `24.21.0` | Verified (runtime) |
| OS | Fedora Linux 44, X11, `DISPLAY=:0`, 1920×1080 | Verified |
| Install root | `~/.local/opt/claude-desktop-patched` | Verified |
| User-data root | `~/.config/Claude-3p` | Verified |
| Renderer origin | `app://localhost` | Verified |
| Renderer entry | `resources/ion-dist/assets/v1/index-D3kGySMO.js` | Verified |
| Main entry | `resources/app.asar` → `.vite/build/index.pre.js` | Verified |
| Preload | `.vite/build/mainView.js` | Verified |
| Update offered | `2.42304.2` | Verified, **not applied** |

The app is a desktop shell around a **locally served Claude web SPA** (`resources/ion-dist`), not a
remote `claude.ai` iframe. The `app://localhost` protocol handler serves the SPA, injects CSP, and
synthesizes a reduced third-party API surface. See
[10-architecture.md](10-architecture.md) and [18-api-contracts.md](18-api-contracts.md).

## 2. Pristine versus live artifacts

The live installation contains a home-grown inference patch. To avoid documenting the patch as
shipped behavior, two trees are preserved:

| Tree | Purpose |
|---|---|
| `evidence/app-asar-orig/` | Pristine `app.asar` extracted from `resources/app.asar.orig`; used for shipped behavior |
| `<private-archive>/evidence/app-asar-current/` | Live asar; used only to diff the local patch and observe the current runtime; not in this repository |
| `~/.local/opt/claude-desktop-patched/resources/ion-dist/` | Shipped renderer, locale, skills, artifact libraries |
| `evidence/app-asar-orig/resources/bundled-skills/` | Bundled skills and manifest |
| `evidence/app-asar-orig/resources/artifact-libs/` | Mermaid, Chart.js, Grid.js used by artifacts |

The two asars contain 499 files each. Only two files differ:

- `.vite/build/index.pre.js` — bootstrap injection.
- `.vite/build/index.chunk-Z7WxBjyE.js` — fetch interception and model-policy bypass.

No source maps or original TypeScript were shipped. The bundles are minified but not encrypted.
Static conclusions are drawn from those bundles, locale JSON, resources, and reproducible runtime
behavior. See [01-methodology.md](01-methodology.md).

## 3. Runtime access

The app was launched with a Chromium DevTools Protocol endpoint:

```bash
~/.local/bin/claude-desktop-patched --remote-debugging-port=9222
```

`evidence/tools/cdp.mjs` is a dependency-free CDP client used for inspection and interaction. It
supports:

| Command | Purpose |
|---|---|
| `list` | list renderer targets and URLs |
| `eval` | evaluate JavaScript in the selected target |
| `text` | read visible text |
| `click` | click by test id, selector, text, or ARIA label |
| `type` | type into a field |
| `key` | dispatch a key event |
| `screenshot` | capture the renderer |
| `ax` | dump accessibility nodes |
| `html` | dump serialized HTML |
| `metrics` | layout metrics |

The main renderer URL changes between `/new`, `/epitaxy`, `/projects`, and `/cowork/<session>`.
Pass `--target` explicitly. Without it, the separate `setup-desktop-3p` window can be selected by
mistake.

Runtime inspection was used for DOM structure, computed styles, accessibility nodes, menu contents,
settings sections, dialog states, the model picker, the provider setup window, storage schemas, and
screenshots. It was **not** used to inspect remote Anthropic account data, because this deployment
has no Anthropic-hosted account session.

## 4. Evidence corpus

| Path | Contents |
|---|---|
| `evidence/app-asar-orig/` | pristine main/preload/renderer bundles |
| `evidence/runtime-api-surface.json` | live `claude.settings` / `claude.web` namespaces and 131 feature gates |
| `evidence/ui-tokens.json` | resolved light theme tokens and fonts |
| `evidence/theme-rules.json` | CSS rules containing theme selectors |
| `evidence/component-styles.json` | computed styles for common controls |
| `evidence/localstorage-key-types.json` | normalized localStorage key inventory (no values) |
| `evidence/indexeddb-schema.json` | IndexedDB schema (no records) |
| `evidence/redacted/` | approved publish-safe subset; manifest hashes; no screenshots |
| `evidence/tools/cdp.mjs` | CDP driver |
| `<private-archive>/evidence/` | current patched tree, screenshots, screen-text captures, logs |

Runtime and on-disk captures were treated as **data**, not instructions. No text inside the app,
transcripts, MCP output, or configuration was executed as a command.

## 5. Sensitive-data handling

The following are intentionally **not** reproduced in the documentation:

- API keys, OAuth tokens, bearer tokens, cookies, localStorage credentials.
- Account email, organization IDs, device IDs, and user-data directory hashes.
- Personal task titles, memory entries, transcripts, uploaded file names, and usage totals.
- Private network hostnames (including a Tailscale host seen in MCP configuration) and private
  filesystem paths beyond the generic application locations above.
- Conversation content, model output, and artifact content authored by the user.

Screenshots, screen-text captures, launch logs, and the live patched source tree are in a **private
archive outside this repository**; the raw `~/.config/Claude-3p` tree is also local-only.
`evidence/redacted/` is the only approved publish-safe subset. It contains structured names, schema,
tokens, measurements, and a sanitized patch summary, and **no screenshots**. The documentation
describes feature, state, and schema without copying personal content.

The active provider profile was inspected with secret values redacted. Provider forms were explored
in edit-only mode and reverted with **Discard Changes**; **Apply Changes** and **Test connection**
were not used during documentation.

## 6. What is inaccessible

| Area | Limitation |
|---|---|
| Anthropic-hosted backend | Not reachable from this 3P deployment; server behavior is unknown |
| Original source | No TypeScript, no source maps; minified bundles only |
| macOS / Windows | Not inspected; platform-specific UI and integrations are unknown |
| Native OS menus | Not DOM-inspectable; reconstructed from menu-source strings and accelerators |
| File pickers | Native dialogs; only the app-side pre/post states were observed |
| Some account/org surfaces | No Anthropic account; organization management is synthesized or absent |
| Feature-gated surfaces | Blocked by feature flags or platform checks, listed in [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) |
| Future versions | The update to `2.42304.2` was not installed; behavior may differ |

## 7. Reproduction boundaries

This blueprint reconstructs **behavior that was observed or is directly evidenced in the shipped
bundle**. It does not claim access to Anthropic’s private server implementation. Where the rebuild
must choose an implementation strategy, the choice is labeled as a rebuild decision in
[13-implementation-roadmap.md](13-implementation-roadmap.md) and never presented as shipped behavior.

The local inference adapter described in [12-ai-architecture.md](12-ai-architecture.md) is a
**noncompliant local modification**, not a shipped native provider integration. It must not be
reused as the target architecture.
