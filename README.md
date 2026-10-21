# Claude Desktop 3P — Reverse-Engineering and Rebuild Blueprint

**Status: documentation complete for the observed surface.** This repository is the evidence-backed
source of truth for rebuilding the installed Claude Desktop 3P application and extending it with
additional native AI providers, SDKs, models, protocols, skills, and MCP servers.

It is a **documentation/evidence repository**, not a working clone. Every material claim is labeled
Verified, Strong inference, or Unknown. Unreachable behavior is recorded as a limitation rather than
invented.

## Application under investigation

| Field | Value |
|---|---|
| Product | Claude Desktop (`@ant/desktop`) |
| Installed version | `2.42304.0` |
| Electron | `44.7.2` (Chromium `152.0.7977.149`, Node `24.21.0`) |
| Platform inspected | Fedora Linux 44, X11, 1920×1080, 1× scale |
| Install root | `~/.local/opt/claude-desktop-patched` |
| User data | `~/.config/Claude-3p` |
| Deployment mode | `3p` (third-party/custom gateway) |
| Renderer | local SPA at `app://localhost/new` |
| Current observed model | MiniMax M3.1 Flash (free) via a local Gateway |

The app is **not** the consumer Anthropic-hosted build. Runtime flags show
`deploymentMode: "3p"`, a custom gateway, a reduced feature-gate set, and a separate third-party
setup/control-plane window.

> **Important:** the inspected installation also contains an earlier local patch that intercepts
> inference and translates Anthropic requests to an OpenAI-compatible endpoint. That adapter is
> **noncompliant**, is negative evidence only, and is not the architecture described for the rebuild.
> The blueprint uses native provider adapters and rejects silent translation/routing.

## Read this first

| If you want to… | Read |
|---|---|
| Understand the evidence and access | [00-access-and-evidence](docs/00-access-and-evidence.md) |
| See how claims were verified | [01-methodology](docs/01-methodology.md) |
| Rebuild the shell/protocols | [02-lifecycle](docs/02-application-lifecycle.md), [10-architecture](docs/10-architecture.md) |
| Find a feature and its status | [03-feature-inventory](docs/03-feature-inventory.md) |
| Trace feature → doc → module → test | [17-traceability](docs/17-traceability.md) |
| Design the AI/provider layer | [12-ai-architecture](docs/12-ai-architecture.md) |
| Build the setup/control plane | [09-settings-and-configuration](docs/09-settings-and-configuration.md), [18-api-contracts](docs/18-api-contracts.md) |
| Build a screen | [05-screen-inventory](docs/05-screen-inventory.md), [07-design-system](docs/07-ui-design-system.md), [screens/](docs/screens/) |
| Build a feature | [features/](docs/features/) |
| Know what is unknown/blocked | [14-gaps-and-unknowns](docs/14-gaps-and-unknowns.md) |
| See proposed changes (separate from original) | [19-improvement-register](docs/19-improvement-register.md) |

## Documentation map

| # | Document | Purpose |
|---|---|---|
| 00 | [Access and evidence](docs/00-access-and-evidence.md) | Environment, artifacts, constraints |
| 01 | [Methodology](docs/01-methodology.md) | Trust levels, static/runtime method, patch isolation |
| 02 | [Application lifecycle](docs/02-application-lifecycle.md) | Startup, windows, protocols, shutdown, updates |
| 03 | [Feature inventory](docs/03-feature-inventory.md) | 122 stable feature IDs and statuses |
| 04 | [Progress tracker](docs/04-progress-tracker.md) | What was inspected, what remains blocked |
| 05 | [Screen inventory](docs/05-screen-inventory.md) | Routes/surfaces, metrics, unobserved screens |
| 06 | [Navigation map](docs/06-navigation-map.md) | Routes, sidebar, menus, back/forward |
| 07 | [UI design system](docs/07-ui-design-system.md) | Tokens, typography, dimensions, themes |
| 08 | [Interaction catalog](docs/08-interaction-catalog.md) | Shortcuts, menus, dialogs, states |
| 09 | [Settings and configuration](docs/09-settings-and-configuration.md) | End-user settings + third-party setup |
| 10 | [Architecture](docs/10-architecture.md) | Main/preload/renderer/protocol boundaries |
| 11 | [Data and storage](docs/11-data-and-storage.md) | Sessions, audit, usage, memory, retention |
| 12 | [AI architecture](docs/12-ai-architecture.md) | Providers, adapters, capabilities, no substitution |
| 13 | [Roadmap and acceptance](docs/13-implementation-roadmap.md) | Phases, modules, core test suites |
| 14 | [Gaps and unknowns](docs/14-gaps-and-unknowns.md) | Explicit limitations |
| 15 | [Evidence index](docs/15-evidence-index.md) | Claim-to-evidence map + redaction rules |
| 16 | [Feature-flag matrix](docs/16-feature-flag-matrix.md) | All 131 runtime gates |
| 17 | [Traceability and tests](docs/17-traceability.md) | Feature/doc/module/test matrix + test catalog |
| 18 | [API contracts](docs/18-api-contracts.md) | IPC, HTTP, stream, MCP, config contracts |
| 19 | [Improvement register](docs/19-improvement-register.md) | Proposed changes, separate from baseline |
| 20 | [Configuration key index](docs/20-configuration-key-index.md) | All 186 profile keys + app/storage keys |

### Feature specifications

[Chat/composer](docs/features/chat-and-composer.md) ·
[Cowork](docs/features/cowork.md) ·
[Code](docs/features/code.md) ·
[Projects](docs/features/projects.md) ·
[Artifacts](docs/features/artifacts.md) ·
[Scheduled tasks](docs/features/scheduled-tasks.md) ·
[Search/filter](docs/features/search-and-filter.md) ·
[Customize](docs/features/customize.md) ·
[Settings](docs/features/settings.md) ·
[Third-party setup](docs/features/third-party-setup.md) ·
[Auth/accounts](docs/features/auth-and-accounts.md) ·
[Model selection](docs/features/model-selection.md) ·
[MCP/skills/tools](docs/features/mcp-skills-tools.md) ·
[Multimodal](docs/features/multimodal-attachments.md) ·
[Updates/telemetry/privacy](docs/features/updates-telemetry-privacy.md)

### Screen specifications

[New task](docs/screens/new-task.md) ·
[Task detail](docs/screens/task-detail.md) ·
[Code dashboard](docs/screens/code-dashboard.md) ·
[Projects](docs/screens/projects.md) ·
[Artifacts](docs/screens/artifacts.md) ·
[New artifact task](docs/screens/new-artifact.md) ·
[Scheduled](docs/screens/scheduled-tasks.md) ·
[Customize](docs/screens/customize.md) ·
[Settings](docs/screens/settings.md) ·
[Account/language](docs/screens/account-menu-and-language.md) ·
[3P setup](docs/screens/setup-desktop-3p.md) ·
[Search/filter](docs/screens/search-palette.md) ·
[Model picker](docs/screens/model-picker.md) ·
[Composer menus](docs/screens/composer-menus.md)

## Core findings

- The renderer is a **local Claude web SPA** served from `resources/ion-dist`, not a remote
  `claude.ai` frame.
- Shipped providers are **Gateway, Claude API, Bedrock, Bedrock Mantle, Vertex AI, Foundry**, with
  provider-native host/auth contracts.
- Shipped model policy is **Anthropic-only** for Claude provider families. It is not a general
  multi-vendor router. The live patch’s policy bypass is rejected.
- The **separate `/setup-desktop-3p` window** is the authoritative provider/control plane. It has nine
  tabs, six provider forms, draft/apply/discard, managed overlays, health, and export.
- The model picker captured here is a flat 522-row/432-flyout list with “needs API key” states. Search,
  grouping, and curated defaults are improvements, not shipped evidence.
- Feature gates remove several surfaces here: artifact pane, native Quick Entry, Cowork browser,
  computer use, iOS Simulator, WSL, self-hosted sessions, dedicated Chat tab, and `yukonSilver`.
- No source maps or original TypeScript exist. The UI is minified; behavior is supported by runtime
  DOM/accessibility evidence and static string/schema extraction.

## Evidence and redaction

- `evidence/app-asar-orig/` — pristine extracted app (shipped behavior). Source redistribution is
  subject to the app’s license/notice terms.
- `evidence/runtime-api-surface.json` — namespaces, methods, 131 gates.
- `evidence/ui-tokens.json`, `component-styles.json`, `theme-rules.json` — UI measurements.
- `evidence/redacted/` — **the approved publish-safe subset**, including a manifest with SHA-256
  hashes. If a file is not there, it is not approved for publication.

Raw screenshots, screen-text captures, launch logs, and the live patched source tree are in a private
archive **outside this repository** and must not be packaged with it. Secrets and personal content are
never reproduced in the docs. See [15](docs/15-evidence-index.md) and
[evidence/README.md](evidence/README.md) for the full handling rules.

## Reproducing the inspection

```bash
# Launch with CDP enabled (inspection only; not needed for normal use):
<install-root>/claude-desktop --remote-debugging-port=9222

# List renderer targets:
node evidence/tools/cdp.mjs list --target app://

# Read URL/text (choose the main renderer, not setup):
node evidence/tools/cdp.mjs eval --target '/new' \
  "({url:location.href,title:document.title,text:document.body.innerText.slice(0,2000)})"

# Capture a screenshot into private evidence, never into this repository:
export PRIVATE_EVIDENCE="${PRIVATE_EVIDENCE:?set to a directory outside the repository}"
node evidence/tools/cdp.mjs screenshot "$PRIVATE_EVIDENCE/screen.png"
```

CDP target selection is ambiguous because the main window and setup window share an origin. Use a
URL substring such as `/new`, `epitaxy`, or `cowork/`; omitting it often selects setup.

## Conventions

- **Verified** — directly observed/reproducible with a cited artifact.
- **Strong inference** — multiple independent supports, not directly proven.
- **Unknown** — not verified; never filled with invented behavior.
- **Improvement** — proposed change, recorded in [19](docs/19-improvement-register.md).
- Feature IDs are stable and defined in [03](docs/03-feature-inventory.md).
- Requirement IDs in feature specs use `REQ-*`; they are not inventory IDs.
- Tests use `TEST-<AREA>-<NNN>`; core suites are indexed in [17](docs/17-traceability.md).
- No provider adapter may monkey-patch global fetch, translate arbitrary protocols, or silently
  substitute a model.
- MiniMax M3.1 Flash (free) is the preferred transparent reconstruction default, explicitly labeled
  as MiniMax and using its native ID.

## Limitations

Anthropic’s server-side behavior, original TypeScript, source maps, macOS/Windows variants, native file
pickers, some permission dialogs, populated project detail, populated artifact viewer, scheduled-task
editor/history, and several platform-gated features were not accessible. They are tracked in
[14](docs/14-gaps-and-unknowns.md) and are not presented as implemented behavior.

Last updated: 2026-10-20.

## Agent instructions

If you are an AI agent (or driving one) working in this repository, read these first, in this order:

| File | Purpose |
|---|---|
| [AGENTS.md](AGENTS.md) | Operating manual: reading order, task workflow, branch loop, definition of done |
| [RULES.md](RULES.md) | Hard constraints: what is forbidden, each rule citing its source doc |
| [MEMORY.md](MEMORY.md) | Durable project facts, known traps, doc conflicts, where the answer lives |
| [CHECKLIST.md](CHECKLIST.md) | Pre-commit gate to run before every commit |
| [CLAUDE.md](CLAUDE.md) | Compact core card, auto-loaded by Claude Code |

These files are navigational — they point at the authority in `docs/`. If an agent file and a doc
disagree, the doc wins and the agent file should be corrected. They are maintained in append-only
fashion: extend at the end, never insert mid-file or renumber existing entries.

