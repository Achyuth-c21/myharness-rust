# 14 — Gaps, Unknowns, and Limitations

This document is intentionally conservative. “Unknown” means the investigation did not establish
the behavior; it is not a placeholder for a guess. A rebuild may choose a behavior, but that choice
must be labeled as a rebuild decision and tested.

## 1. Platform and feature gates

The runtime exposes 131 feature gates: 64 supported, 51 unavailable, 16 unsupported. Full results are
in [16-feature-flag-matrix.md](16-feature-flag-matrix.md).

| Surface | Observed gate/result | Status | Impact |
|---|---|---|---|
| Artifacts side pane | `artifactsPane` unavailable | Unavailable | Artifact library exists, but the side pane/viewer flow was not reached |
| Native Quick Entry | `nativeQuickEntry` unavailable; Quick Entry shortcut exists | Unavailable | Global panel, dictation-in-panel, and native entry behavior unknown |
| Cowork browser | `coworkBrowser` unsupported (`cowork_browser_disabled`) | Unsupported | Built-in browser settings visible, but the in-app browser surface is unavailable |
| Computer use | computer-use gates unavailable/consent-gated | Unavailable | Teach/preview/stop UX unobserved |
| iOS Simulator | simulator surfaces unsupported on Linux | Unsupported | Camera/device panes and simulator tools unknown |
| WSL sessions | `disableWslSessions` / Linux host | Disabled | WSL-specific Code behavior unknown |
| Self-hosted sessions | `selfHosted` gates unavailable | Unavailable | Self-hosted token/URL, remote runner behavior unknown |
| Cloud sessions | not configured | Unknown | Cloud placement, sharing, sync behavior unknown |
| Dedicated Chat tab | `chatTab` unavailable | Unavailable | Unified Cowork view is shown; Chat-specific navigation/layout unknown |
| VM virtualization (`yukonSilver`) | unavailable | Unavailable | Full VM sandbox behavior on this host unknown |
| Tray icon | supported gate but desktop reports no tray | Partial | Close-to-tray behavior unknown on this Linux desktop |
| Quick Entry dictation | `quickEntryDictation` unavailable | Unavailable | Dictation flow unknown |

A gate can be unavailable for build, platform, deployment, or policy reasons. Do not conclude the
feature is absent from all Claude Desktop builds.

## 2. Backend and account behavior

The deployment is third-party mode. `/api/bootstrap` and related endpoints are synthesized locally;
unavailable API paths return a typed 503. Therefore:

- Anthropic account/org/billing/entitlement behavior is not observable.
- Remote conversation sync, sharing, and web/mobile parity are unknown.
- Hosted model routing/fallbacks are unknown and must not be inferred from the desktop UI.
- Organization administration, SCIM, audit export, and billing screens are not present here.
- Usage numbers are device-local and do not represent provider billing.
- Sign-in behavior for a real Anthropic account could not be exercised.
- Gateway OAuth/device-code flows were not completed.

**Do not** document Anthropic’s server implementation from this evidence.

## 3. Unobserved UI flows

| Flow | Why unobserved | What is known |
|---|---|---|
| Existing project detail | Would require creating a project with personal content | List/empty/create flows known |
| Project files/folders | Same | Project creation has folder + instructions + memory |
| Artifact viewer/editor | `artifactsPane` unavailable | Library + new-artifact task known; API has versions/share/star/hide |
| Artifact version history/share | Not reached | API methods exist; layout unknown |
| Scheduled task editor | Not exercised | Page/templates/keep-awake known |
| Scheduled run history/failure | No runs | Task schema and skips known |
| Permission prompt | No tool call required approval in captured session | Events/APIs and policy known; layout unknown |
| Trusted folders manager | Button not opened | Settings summary and API known |
| Allowed-sites manager | Button not opened | Settings summary and API known |
| Memory manager | Memory list seen; edit/reset not exercised | Toggle/list known; content intentionally not retained |
| Skill editor/upload | Not exercised | List/filter/add and bundled manifest known |
| Connector detail/OAuth | Not exercised | List/status/Connect known; flow unknown |
| Plugin install/detail | No plugins installed | Empty state, marketplaces, org plugin schema known |
| Update download/install | Update intentionally not applied | Banner/config keys known |
| About/changelog | Not opened | Menu entry known |
| Support bundle | Not run | API/copy known |
| Language apply/restart | Dialog captured; not changed | Locale list and persistence known |
| Account sign-out | Destructive; not exercised | Menu entry known |
| Quick Entry/tray/popouts | Platform-gated | Shortcut/window code known |
| Code session detail | Not created | Dashboard/route/APIs known |
| Terminal/PTY UI | Not exercised | Process lifecycle/settings known |
| Browser/Cowork browser | Gate unsupported | Settings/policy known |
| Computer use/simulator | Platform-gated | Flags only |
| Error overlay | Not triggered | Source strings only |
| Windows/macOS variants | Not available | Linux only |
| Title-bar glyph↔role pairing (panel toggle vs sidebar toggle) | The 2026-10-21 reference capture is not in the repo (`evidence/` holds no image files), so the pairing cannot be re-inspected | Roles and order come from the shipped app's own chrome: its top-left three-line control is a *Menu* button and the sidebar is toggled by `Cmd/Ctrl+B` (app menu "Show/Hide Sidebar"), so the three lines are not the sidebar collapse. The rebuild follows that pairing (panel toggle = three lines, sidebar toggle = framed pane, sidebar toggle second). Exact codepoints remain unpinned (`docs/07` §1.5) |

## 4. Architecture unknowns

| Topic | Unknown | Current evidence |
|---|---|---|
| Main IPC sender validation | Whether every generated handler checks sender, not just preload | Preload top-frame/origin check verified; generated channel pattern known |
| Exact config precedence | Local vs bootstrap vs org/MDM vs defaults | “Bootstrap overrides local and becomes read-only” stated in UI; merge code not fully traced |
| Apply/relaunch semantics | Which changes require relaunch; session guard details | `relaunchApp`, `relaunchAppWithSessionGuard`, relaunch window config |
| Setup draft persistence | Whether drafts survive window close/crash | Apply/Discard observed; internal draft store unknown |
| App startup redirect | `/` → `/new` rule | Runtime landed on `/new` |
| Session route restoration | Whether the last open task is restored after restart | Session restore verified; last-route behavior unknown |
| Window/popout topology | All popout types and layouts | Build files/strings; not exercised |
| CSP generation | Full directive set per config | Injection verified; exact directives configuration-dependent |
| Update implementation | Download/signature/rollback specifics in this package | Feed/config/UI known; not applied |
| Crash recovery | Database corruption/half-migration behavior | Error overlay/tolerant restore strings |
| Deep-link grammar | Allowed routes/params and auth checks | Schemes/registration known |
| Native menu behavior | OS-rendered menu structure | Accelerators extracted from source |
| Protocol stubs | Complete endpoint table and forwarded paths | Key stubs/503/gate verified |
| Service worker/cache | Role in offline UI | Chromium partition/cache present; behavior unknown |
| Multi-account/multi-org | Disabled here; switching semantics | API present; `disableMultiAccount` |

## 5. Provider/inference unknowns

| Topic | Unknown | Evidence |
|---|---|---|
| Exact provider request paths | Bedrock/Vertex/Foundry payload details | Provider families/host maps/validation; minified implementation |
| Anthropic backend fallback/routing | Hosted behavior | Not accessible in 3P |
| Full capability matrix | Per-model tools/vision/thinking/cache | Catalog fields exist; values not enumerated |
| Provider retry policy | Retries/backoff/idempotency | Error strings/timeouts; exact policy unknown |
| Stream event coverage | All native event types | Text/tool/thinking paths observed; full schema unknown |
| Token counting | Native methods per provider | Adapter estimate exposed as noncompliant; shipped method unknown |
| Model catalog merge precedence | Static vs discovery vs catalog | Sources known; exact tie-breaking unknown |
| Gateway model metadata schema | All optional fields | `anthropic_family_tier`, `supports_1m`, `prefer_1m`, `max_effort_level` seen |
| Gateway error schema | Full error taxonomy | Typed 503 and provider errors seen |
| Credential helper protocol | Exact stdout/JSON contract | Args/timeout/TTL fields known |
| OAuth token storage | Keychain vs encrypted file per platform | Token cache exists; exact backend unknown |
| Proxy/PAC behavior | Host matching and auth | Config fields known |
| Bedrock Mantle semantics | Auth/signing and discovery | Host/credential rules known |
| Foundry auth | Azure identity/interactive details | Fields/validation known |
| 1M context behavior | Request flag and model identity | `[1m]`, `supports_1m`, `prefer_1m` |
| Fast mode | Provider/model support and wire flag | UI/catalog fields known |
| Effort clamping | What happens when a model lacks an effort level | Options known; behavior unknown |
| Usage limit enforcement | Exact window algorithm/cached-token exclusion | UI copy + ledger fields |

## 6. Data and security unknowns

| Topic | Unknown |
|---|---|
| Encryption at rest | Which stores are encrypted beyond OS/keychain |
| Log retention | Rotation/size/age policy |
| Memory indexing | Vector/embedding vs file-only |
| Artifact storage format | Content store, versions, dedupe |
| Artifact preview isolation | Exact origin/partition/sandbox flags |
| Plugin signature verification | Required signatures, trust roots, failure modes |
| Extension install source | Which marketplaces/URLs are trusted |
| Session ID/hash derivation | Account/org pseudonymization and salts |
| Audit key lifecycle | Generation/rotation/deletion |
| Backup/restore | Whether user-data backup is supported |
| Migration | Schema versions and downgrade behavior |
| Telemetry payloads | Full field list and sampling |
| Diagnostic bundle contents | Exact files/redaction |
| Egress enforcement | DNS/IP/redirect handling and proxy interaction |
| Symlink handling | Canonicalization and TOCTOU defenses |
| Custom header secret detection | Heuristics and bypass cases |
| Sender validation | Coverage of generated handlers |
| CSP exceptions | Any `unsafe-*`, `data:`, `blob:` directives per mode |
| Permission persistence | Session vs project vs global scope details |

## 7. Known inconsistencies and traps

1. **Current state ≠ shipped state.** The live install has a local adapter and `So → true`.
2. **Model count ≠ selectable models.** Discovery can return hundreds; `inferenceModels` can contain
   two. They are different sources.
3. **Feature visible ≠ feature usable.** Browser settings are visible while `coworkBrowser` is
   unsupported.
4. **Setup window shares origin.** CDP target selection can silently inspect the setup window.
5. **Menus remain mounted.** Toggle the trigger; do not assume Escape unmounted the DOM.
6. **“New artifact” is destructive.** It starts generation immediately.
7. **Screenshot contents are personal.** Task titles, memory snippets, usage totals, and local paths
   must not be published.
8. **Logs include private infrastructure.** MCP/egress logs can contain tailnet/private hosts.
9. **Session JSON is huge.** It embeds system prompts/tool snapshots; lazy loading is required.
10. **Catalog model names are not entitlements.** Credentials and provider policy still apply.
11. **The advisory update is not applied.** `2.42304.2` behavior may differ.
12. **No source maps.** Symbol names are not stable API.
13. **Dev-server-port squatters.** A stale process bound to the dev port answers E2E suites with
    a different app (observed: unrelated SPA with title `Claude` on `:5173`). A whole-grid red
    under port conflict is a harness failure, not a product regression; verify the served
    document title before interpreting results (`tests/report-phase1.md`, Run 3).

## 8. Research questions for a follow-up pass

These would require a different environment or explicit permission:

1. Existing project detail and file management.
2. Artifact viewer/versions/share/export with a non-personal artifact.
3. Permission approval modal and persistent approval lifetime.
4. Trusted folders/allowed-sites/memory managers (using non-personal fixtures).
5. Scheduled task editor and run history with a harmless test task.
6. Connector OAuth and plugin installation using a test account.
7. Bedrock, Vertex, Foundry, and Mantle against test credentials (never real secrets).
8. macOS/Windows window, menu, tray, and platform integrations.
9. Anthropic-hosted account/org behavior in a normal deployment.
10. Update install/rollback on a disposable VM.
11. Computer use, simulator, Cowork browser, Quick Entry, artifacts pane on a supported platform.
12. Full CSP/egress/proxy matrix with network capture.

## 9. How unknowns are handled in the rebuild

- A screen that was not observed is implemented only after a design decision is approved; it is
  labeled “unobserved” in [17-traceability.md](17-traceability.md).
- A provider behavior that was not observed is isolated behind a native adapter and a wire test; it
  is not hard-coded into the UI.
- A capability that cannot be detected is disabled, not assumed.
- A policy that cannot be enforced is not offered as a toggle.
- A platform feature that is unavailable is explicitly unavailable, not emulated with a misleading
  placeholder.

This preserves the product’s honesty: the UI must not claim a capability the runtime cannot deliver.

### §9 supplement — bug-hunt residues (2026-10-21)

From the phase-1 bug hunt (`tests/report-bug-hunt-phase1.md`), fixed in §F and closed with these
recorded unknowns:

1. **Real filesystem faults under `app://` answer the typed 503 envelope** (`custom_3p_not_available`,
   docs/18). It is the only contractually defined protocol error body, so a host filesystem failure
   (EACCES etc.) still reports as "not available in third-party mode" — mislabeled but contractual.
   Directory reads (the main real-world trigger, `/.`) are fixed and take the SPA fallback instead.
   Status: **Unknown** — a faithful error kind would need a documented envelope first.
2. **The Tauri adapter's host gate (B1 fix) is compile-verified (2026-10-21):** with the system
   libraries installed (`webkit2gtk-4.1`, `gtk3`, `libsoup3`, `dbus`, `librsvg`, `pkgconf` — user
   action), `cargo check` in `apps/desktop/src-tauri` finishes clean. First compilation also
   surfaced two latent build blockers unrelated to B1, fixed in the same train: (a) the
   `register_uri_scheme_protocol` sync callback must return `Response` directly (adapter now maps
   `adapt()` construction failures to a uniform fallback), and (b) the four placeholder icons were
   1×1 gray+alpha, rejected by `generate_context!` — replaced with RGBA PNGs drawing the documented
   orange starburst (docs/07 §1.5). Remaining unknown: the **runtime** behavior of real webview
   requests (whether a given webview sends a `Host` header) is only positively observable through a
   running window; the fail-closed rejection path stays logic-verified. Status: **compile-verified
   (cargo check); runtime pass-through exercised by the launched shell, rejection path logic-only**.
3. **One T30 reload-timeout flake recurred** during the fix verification (same signature as the
   report's recorded one-off: `a.nav-item` absent after `page.reload()`, not reproducible in
   isolation; two subsequent full runs green). Status: **Unknown** — flake of unknown cause; the
   suite is otherwise deterministic over consecutive runs.
4. **Deep-link plumbing is runtime-exercised (2026-10-21)** — cold-start argv and second-launch
   forwarding both navigate the primary (fail-closed classification, `Setup3p` deferred to the
   Phase-2 setup window per ARCH-NAV-03). Fixture: `tests/native/shell-runtime.sh` (17/17).
   Still unknown/deferred: OS-level scheme registration (the `myharness://` x-scheme-handler /
   `.desktop` entry) belongs to packaging (docs/13 §4 Phase 9) — clicking such a link in a browser
   does nothing on this machine today. Status: **runtime-verified via argv channel; OS registration
   deferred, not implemented**.
5. **Native menu accelerators: recorded deferral (IMP-30)** — the shipped app's accelerators
   (docs/06 §7, `Verified`) are honored only insofar as Phase-1 features exist: shell accelerators
   in-app, text-edit accelerators WebView-native, the rest deferred to the feature phases or the
   menu surface (a GTK menubar would visibly break the frameless geometry captured in docs/07).
   Status: **decided and documented (deferred)**, not lost.
