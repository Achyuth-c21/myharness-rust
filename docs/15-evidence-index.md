# 15 — Evidence Index

This index maps claims to artifacts. Raw evidence is **private and kept outside this repository**.
Several screenshots and text dumps contain personal task titles, memory snippets, usage totals,
account identifiers, or private hostnames. Only `evidence/redacted/` is approved for publication.

## 1. Pristine and current application trees

| Path | Description | Use |
|---|---|---|
| `evidence/app-asar-orig/` | Extracted pristine `resources/app.asar.orig` | Shipped behavior |
| `<private-archive>/evidence/app-asar-current/` | Extracted live `resources/app.asar` | Patch diff/current state only; not in this repository |
| `evidence/app-asar-orig/package.json` | Product/version/main/dependencies | Verified metadata |
| `evidence/app-asar-orig/.vite/build/index.pre.js` | Main bootstrap | Lifecycle |
| `evidence/app-asar-orig/.vite/build/index.chunk-Z7WxBjyE.js` | Main bundle | Protocols, config, providers, sessions |
| `evidence/app-asar-orig/.vite/build/mainView.js` | Preload/contextBridge | API surface, origin checks |
| `evidence/app-asar-orig/.vite/build/*.js` | Window/worker/preview modules | Architecture |
| `evidence/app-asar-orig/resources/bundled-skills/` | Six bundled skills + manifest | Skills |
| `evidence/app-asar-orig/resources/artifact-libs/` | Mermaid, Chart.js, Grid.js | Artifacts |
| `~/.local/opt/claude-desktop-patched/resources/ion-dist/` | Shipped renderer SPA (4,224 files) | UI/routes/locales |
| `~/.local/opt/claude-desktop-patched/resources/ion-dist/i18n/en-US.json` | 35,441 UI strings | Labels/copy |
| `~/.local/opt/claude-desktop-patched/resources/ion-dist/assets/v1/index-D3kGySMO.js` | Renderer entry/chunk map | UI behavior |

The current asar differs from pristine in only two files. The private current tree was compared with
the pristine tree; the approved sanitized summary and hashes are in
`evidence/redacted/live-patch-summary.md`. See [01-methodology.md](01-methodology.md) and
[12-ai-architecture.md](12-ai-architecture.md).

## 2. Runtime inventories

| File | Contents | Sensitivity |
|---|---|---|
| `evidence/runtime-api-surface.json` | Settings/web namespaces, methods, 131 feature gates | Low; method names only |
| `evidence/ui-tokens.json` | Resolved light theme tokens/layout metrics | Low |
| `evidence/theme-rules.json` | CSS theme selectors/rules | Low |
| `evidence/component-styles.json` | Computed component dimensions/colors/radius | Low |
| `evidence/style-aggregate.json` | Font sizes/weights/radii/color frequency on task screen | Low |
| `evidence/localstorage-key-types.json` | 267 keys normalized to 104 patterns; no values | Low |
| `evidence/indexeddb-schema.json` | `keyval` store keys; no records | Low |
| `evidence/i18n-en-US-values.txt` | Extracted English strings | Low; may contain product copy only |
| `<private-archive>/evidence/logs/` | Launch/runtime logs | Private; do not publish |

The repository files above are the preferred citations for architecture/UI claims because they do not
contain personal content. Logs remain private.

## 3. Screen text captures

The files below are in `<private-archive>/evidence/screens/`, not in this repository. The table is a
traceability index; do not add these files to a publication package.

| File | Surface | Redaction needed |
|---|---|---|
| `screens/settings-general.txt` | Settings → General | None (generic) |
| `screens/settings-privacy.txt` | Settings → Privacy | Provider/deployment copy; verify no host |
| `screens/settings-usage.txt` | Settings → Usage | **Yes** — personal totals |
| `screens/settings-claude-code.txt` | Settings → Claude Code | Possibly local paths |
| `screens/settings-import-export.txt` | Settings → Import & export | Low |
| `screens/settings-system.txt` | Settings → System | None |
| `screens/settings-developer.txt` | Settings → Developer | **Yes** — server names/paths |
| `screens/setup-3p-connection.txt` | Setup → Connection | Low; gateway URL may be private |
| `screens/setup-3p-capabilities.txt` | Setup → Capabilities | Low; folder/egress may be private |
| `screens/setup-3p-connectors.txt` | Setup → Connectors | **Yes** — MCP hosts/tokens |
| `screens/setup-3p-telemetry.txt` | Setup → Telemetry & updates | Low |
| `screens/setup-3p-limits.txt` | Setup → Limits | Low |
| `screens/setup-3p-appearance.txt` | Setup → Appearance | Low |
| `screens/setup-3p-plugins.txt` | Setup → Plugins | Low; marketplace names |
| `screens/setup-3p-egress.txt` | Setup → Egress | **Yes** — private MCP/Tailscale hosts |
| `screens/setup-3p-source.txt` | Setup → Source | Low |
| `screens/setup-3p-provider-*.txt` | Provider forms | Credentials not captured; verify fields |

## 4. Screenshots

All are 1920×1080 PNG captures from the live renderer. Most include the sidebar and therefore personal
task titles. They are stored in `<private-archive>/evidence/screenshots/` and are **not in this
repository**. No screenshot is approved for publication.

| Group | Files | Claims supported |
|---|---|---|
| Early window/CDP | `00`–`02` | Boot state, initial UI |
| Projects | `10`, `11`, `12`, `66`–`68` | Empty state, create dialog, form |
| Artifacts | `20`, `69`, `70` | Empty library, new-artifact task |
| Scheduled | `21` | Empty state/templates/keep-awake |
| Customize | `22`–`24` | Skills, connectors, plugins |
| Account/language | `25`, `34` | Account menu, locale list |
| Settings | `26`–`33`, `71`–`73` | Sections, privacy, usage, dark theme |
| Setup | `35`–`43` | All nine tabs |
| New task/composer | `50`–`59` | Composer, model picker/browser, menus |
| Main screens | `60`–`65` | Home, Code, Cowork, search, filter, task |
| Task detail | `65`, `70` | Transcript, streaming, artifact task |

Screenshots are evidence for layout/tokens, not proof of every interaction. The accessibility tree,
DOM text, and source are preferred for behavior.

## 5. Storage and configuration evidence

These paths are outside the repository and contain live data. Extract only schemas/keys.

| Path | Use | Do not publish |
|---|---|---|
| `~/.config/Claude-3p/config.json` | preference schema | tokens/UUID/locale identity |
| `~/.config/Claude-3p/configLibrary/` | profile schema | API keys, private hosts |
| `~/.config/Claude-3p/local-agent-mode-sessions/` | session/usage/memory/audit schema | prompts, memory, files, email |
| `~/.config/Claude-3p/claude-code-sessions/` | Code metadata schema | paths, branch/worktree names |
| `~/.config/Claude-3p/model-catalog/` | signed catalog/sidecar | Catalog metadata is product data, not personal |
| `~/.config/Claude-3p/logs/` | lifecycle/error evidence | paths, hosts, prompt fragments |
| `~/.local/share/claude-native/native-adapter.cjs` | noncompliant adapter evidence | provider config paths; no credentials |
| `~/.config/Claude-3p/inference-bridge/bridge.cjs` | patch bootstrap | local paths |

## 6. What supports each major claim

| Claim | Evidence |
|---|---|
| Local SPA, not remote claude.ai | `app://` handler, `ion-dist` entry, runtime URL |
| 3P mode / synthetic bootstrap | config, protocol stubs, UI copy, server placements |
| Six providers / host mappings | main bundle provider tables/functions |
| Claude-only model policy | `So`, provider validators |
| Noncompliant adapter | asar diff + adapter source + logs |
| 131 feature gates | `runtime-api-surface.json` |
| Separate setup window | runtime URL/screenshot |
| Settings sections | screenshots + text captures |
| Observed skills / connector list | composer menu capture + bundled manifest |
| Session/usage schemas | file inspection (keys only) |
| Signed model catalog | `published.json` sidecar + decoded schema |
| UI tokens | `ui-tokens.json`, `component-styles.json`, screenshots |

## 7. Redaction procedure

Before publishing any evidence:

1. Remove task titles, transcript text, memory entries, file names, and usage totals.
2. Blur/crop the sidebar when it contains personal history.
3. Replace account/org/device UUIDs with stable placeholders.
4. Remove private hostnames (Tailscale, internal MCP, VPC endpoints) and local absolute paths.
5. Redact authorization headers, cookies, query strings, and API keys, including screenshots.
6. Verify screenshots for reflections of personal data in scrollbars, tooltips, or previews.
7. Re-run a secret scanner over all text/JSON/Markdown.
8. Keep raw evidence outside the publishable tree or in a clearly marked private directory.

## 8. Approved publish-safe subset

`evidence/redacted/` now contains structured, non-personal derivatives only:

- `runtime-api-surface.json` — namespace/method/feature-flag names.
- `localstorage-key-types.json` — normalized key names and value classes.
- `indexeddb-schema.json` — database/store names.
- `ui-tokens.json`, `component-styles.json`, `theme-rules.json`, `style-aggregate.json` — tokens and
  measurements.
- `i18n-en-US-values.txt` — shipped English UI strings.
- `live-patch-summary.md` — sanitized two-file patch evidence, file hashes, and external adapter note.
- `manifest.json` — sizes, SHA-256 hashes, app version, and redaction statement.

**No screenshot, text capture, transcript, log, config value, task title, memory, usage figure,
account/org identifier, credential, or private host is approved.** Raw captures remain in the private
archive outside the repository. The handling rules are in `evidence/README.md`.

## 9. Evidence maintenance

- Keep `app-asar-orig` immutable.
- Re-extract the current asar into the private archive after any app update; never overwrite orig.
- Keep raw screenshots, screen text, and logs in the private archive, never in the repository.
- Record the app version and capture date with every new evidence file.
- Prefer JSON/DOM/text over screenshots for behavior.
- Do not add live credentials or personal captures to fixtures/tests.
- Update [17-traceability.md](17-traceability.md) when adding evidence for a previously unknown row.
