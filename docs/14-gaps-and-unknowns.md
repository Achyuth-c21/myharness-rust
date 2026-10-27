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

6. **Config wire-format spelling is only partially verified (2026-10-22)** — `docs/11` §3 pins
   `appliedId`/`entries` and `docs/18` §10 pins `schemaVersion` (both now enforced by
   `crates/config` wire-format tests). `ProviderKind` values follow the documented provider
   IDs of `docs/12` §1 (`anthropic`, `bedrock`, `mantle`, `vertex`, `foundry`, `gateway`).
   `inferenceCredentialKind` follows the `docs/09` §2.2 kind table in kebab-case
   (`static`, `helper-script`, `interactive`, `vendor-profile`, `workforce`, `external-idp`)
   — but the *shipped renderer's* exact strings were never captured (`docs/20` records value
   classes without values), so the spelling is the rebuild's choice, not an observation. The
   legacy-import mapping (`claudeAiImport`) must translate freely when that slice lands.
   Unknown-key preservation means a wrong guess here is never destructive — round-trips keep
   whatever the shipped side wrote. Status: **Unknown (values), mitigated structurally**.
7. **`delete` of the applied profile** — the shipped behavior was never observed. The rebuild
   refuses it (`CONFIG_IN_USE`; the applied selection is replaced document-first via
   apply-then-delete), which is a recorded fail-safe decision, not an evidence claim.
   Status: **Unobserved → approved decision (fail-safe)**.
8. **Keychain runtime behavior is unverified (2026-10-21)** — `crates/secrets` is
   compile-verified only: `KeyringStore` (keyring 3.6 with `apple-native`,
   `windows-native`, `sync-secret-service` → pure-Rust zbus) has no runtime claim on
   any platform until the bridge wiring slice exercises it on the target desktop
   (`docs/17` §4.8 owned rows). `MemoryStore` is tests/headless tooling only; the
   production no-keychain fallback is the observed mode-`0600` inline profile field
   (`docs/09` §4), never a process-local store. Status: **Unknown (runtime), mitigated
   by fail-closed resolution**.
9. **Some provider field key names were never captured (2026-10-22)** — `docs/09` §2.2 documents
   the *fields* per provider, but `docs/20` §2.1 (the key index, 186 captured `flatKey`s) has
   no entry for five of them: the Claude API optional base URL, the Foundry deployment name, and
   the Bedrock Mantle region / base URL / API key. The rebuild writes these keys under the
   spellings listed in `crates/config::forms::UNVERIFIED_KEYS`; the fields themselves are
   documented, only the on-wire spelling is a guess. Status: **Unknown (key names), mitigated
   structurally** — a wrong key name is not destructive (unknown-key preservation keeps whatever
   the shipped side wrote), and `docs/20` §2.1 is corrected when the shipped spelling is
   observed.
10. **The Bedrock Mantle key family is absent from the key index entirely (2026-10-22)** — no
   `inference*Mantle*` key was captured, while `docs/12` §1 lists Mantle as a first-class
   provider family with its own endpoint, credential and model rules. The rebuild treats
   Mantle's keys as a sibling of the Bedrock family (`inferenceBedrockMantle*`), which is an
   inference from naming, not an observation. Status: **Unknown (key names), mitigated
   structurally**.
11. **Per-credential-kind requiredness in the shipped form is unobserved (2026-10-22)** — the
   shipped UI marks required fields with `*` (`docs/screens/setup-desktop-3p.md`), but which
   fields are required under *which* credential kind was never captured (a static API key is
   required for the static kind and meaningless for the helper kind). The rebuild encodes the
   conditional as `FieldDef::required_when`, which is a rebuild contract derived from
   "when set, only that source is used (no fallback)" (`docs/09` §2.2), not an observed
   per-kind marker set. Status: **Strong inference**.
12. **The Foundry model-discovery disabled-combination list is unobserved (2026-10-22)** —
   `docs/09` §2.2 says discovery "is disabled for several credential kinds" for Foundry but
   does not say which, unlike the explicit lists for Mantle and Vertex. The rebuild does not
   enforce any Foundry discovery combination. Status: **Unknown**.
13. **The managed-overlay document shape and its trust policy are unobserved (2026-10-22)** —
   `docs/09` §2.10 describes an HTTPS endpoint returning a per-user JSON overlay whose
   "values override local settings and become read-only", but the overlay's own grammar was
   never captured, and the trust/signature policy is explicitly unproven
   (`docs/features/third-party-setup.md` Source tab: "exact trust/signature policy is
   unknown"). The rebuild's `crates/config::overlay` accepts a document that mirrors the
   profile document (a JSON object of sections, each a flat map of keys) and a flat
   `section/key` map, and the merge precedence is defined explicitly rather than inferred.
   The **fetch** is deliberately not implemented: bootstrap delivery must be authenticated
   and `trustBootstrapDelivery` is a security-sensitive opt-in, so a transport built on an
   unverified trust policy would be a defect, not a convenience. Status: **Unknown
   (shape), Unknown (trust policy), mitigated structurally**.
14. **The precedence between the local profile, the bootstrap overlay, organization MDM
   policy and runtime defaults was never proven (2026-10-22)** — `docs/09` §3.5 says
   "exact precedence is unknown" and asks the rebuild to "treat precedence as a contract to
   define explicitly". The rebuild defines `managed overlay > active local profile >
   runtime defaults` and records it as a decision. The organization-MDM tier is declared in
   the `ConfigSource` enum but has no producer in this rebuild. Status: **Unobserved →
   approved decision (documented precedence)**.
15. **The helper-script TTL and silent-refresh semantics were never observed (2026-10-22)** —
   `docs/09` §2.2 lists the credential kind's knobs ("Arguments, timeout, TTL, silent
   refresh") and `docs/20` §2.1 gives the key names, but the shipped behavior — what
   happens when a cached credential passes its TTL, and what "silent refresh" suppresses —
   was not captured. The rebuild fails closed: a credential inside its TTL is reused, a
   stale credential is refreshed only when silent refresh is enabled, and otherwise the
   resolution is an error rather than a stale value. The default helper timeout (30 s) is
   likewise a fail-closed default, not an observation. Status: **Strong inference**.
16. **Several egress hostnames were never captured (2026-10-22)** — `docs/09` §2.9 names the
   egress *groups* and the hosts it names verbatim (`downloads.claude.ai`, `claude.ai`,
   `api.anthropic.com`, `claudeusercontent.com`, `claudemcpcontent.com`,
   `assets.claude.ai`), but the hostnames of the Sentry and Datadog intake, the Google
   static/font hosts, the cdnjs and jsDelivr hosts, and the Python package hosts were not.
   `crates/config::egress` emits those groups with `hostsUnobserved` set rather than
   filling them with invented hostnames, and the group boundaries themselves are an
   inference from the category names. The Bedrock control-plane hosts and the six
   provider default host shapes *are* documented (`docs/12` §1) and are derived exactly.
   Status: **Unknown (hostnames), mitigated structurally**.
17. **Some setup-tab field values were never captured (2026-10-22)** — the key *names* for
   every tab are in the key index (`docs/20` §2.1), but several value vocabularies are
   not: `endUserAttribution` is recorded as an `enum/string` with no values, and
   `builtinToolPolicy` is recorded as a `string` with no grammar. `crates/config::tabs`
   accepts a string for these and does not constrain the value, rather than inventing
   an enumeration. The `array/object` keys (marketplaces, MCP servers, SSH configs) have
   no documented inner grammar either, so the egress derivation walks them
   shape-agnostically. Status: **Unknown (value vocabularies), mitigated structurally**.
18. **The keychain-less fallback is not implemented in the bridge (2026-10-22)** — `docs/09` §4
   says "Store secrets in the OS keychain where available; otherwise use mode `0600` files and a
   clear warning". The bridge refuses a renderer write that carries an inline secret and
   writes a typed credential to the keychain, so on a machine with no usable keychain the setup
   window cannot set a credential at all. The `0600` inline fallback is a separate decision that
   has not been taken; the keychain's runtime behavior is compile-verified only
   (`docs/14` §9 item 8). Status: **Unimplemented fallback, recorded (not silently assumed)**.
19. **The setup window has not been driven against a live configuration library (2026-10-22)** —
   the window has been driven in Chromium against a *stubbed* bridge and checked against the
   captured layout, so the renderer's own behavior is verified; what is not verified is the path
   through the real bridge, a live configuration library and the OS keychain on this machine
   the window, its nine tabs, the Apply/Discard footer and the credential flow are implemented and
   exercised: `tests/e2e/setup-window.spec.mjs` (S01–S12) drives the renderer against a
   *stubbed* bridge, and the native fixture proves the dedicated window is really created and
   that the main renderer is never navigated to it. What is still unexercised is the path through
   the **real** bridge, a **live** configuration library and the **OS keychain** on this machine —
   the keychain's runtime behavior remains compile-verified only (`docs/14` §9 item 8). The
   Egress tab's **Test connectivity** probe is not implemented and is shown disabled. Status:
   **Available, unexercised** (live path).
20. **The model catalog's public key was never captured (2026-10-22)** — the shipped
    verification is "RSA PKCS#1 v1.5 SHA-512 signature" plus a "public key SHA-256" check
    (`docs/12` §3.3), but the key itself is not in the evidence, so there is nothing to verify a
    signature against. A verifier that accepted an unknown key would be a policy bypass, so
    `crates/bridge::helper::catalog_state` treats a stored catalog as **unverified** and takes the
    documented failure path: the bundled catalog plus a non-blocking staleness warning
    (`docs/12` §3.3). The "version floor" check is likewise not implemented, because the floor
    value was never observed. Status: **Unknown (public key, floor), mitigated structurally**.

21. **The Test-connection probe's request shape was never captured (2026-10-23)** — the shipped
    UI shows the button and a success/failure state, but its wire traffic (which endpoint, how
    many requests, which model id) was not in any capture. The rebuild defines it: over the
    *draft's* connection values first, the model target comes from the draft's model list (the
    first id the Anthropic model policy accepts) and only runs discovery (≤3 native pages) when
    the draft carries no usable id; the inference is one shot — `POST /v1/messages`,
    `max_tokens: 1`, `stream: false`, content `"ping"` — and the response `model` must echo the
    requested id or the probe fails. One request when the draft names a model, at most four
    when it does not (three discovery pages + the probe). Nothing is persisted; an interactive
    credential is refused, never emulated. Status: **Unobserved → approved decision
    (documented shape)**.
22. **The idle-timeout's reset window semantics are unobserved (2026-10-23)** — `docs/12` §4.1
    records a per-stream idle window whose expiry terminates the stream, but whether ping /
    content_block_start / message_delta frames reset the window was never captured. The
    rebuild defines: the window arms when the stream opens, resets **only** on content events,
    never on `ping`, and `0` disables it. A drip of nothing is therefore bounded
    (`TEST-AI-016`), and a pinging gateway cannot keep a silent stream alive. Status:
    **Strong inference (fixing the window at content events)**.
23. **The betas an OAuth-mode gateway advertises were never captured (2026-10-23)** — the
    adapter transmits exactly the betas the profile/draft configuration names, comma-joined
    upstream (`AI-ADAPTER-07`); it never *invents* a beta value from an unobserved OAuth flow.
    A first-party OAuth beta whose header value was not captured is therefore unavailable in
    the rebuild, not guessed. Status: **Unknown (value), mitigated structurally**. The Claude
    API adapter (slice 2) holds the same refusal: a `sk-ant-oat…` credential is refused with
    the reason named, never sent.
24. **The first-party `/v1/models` listing's wire details were never captured (2026-10-23;
    Claude API slice 2)** — `docs/18` §6 records "the model list is `/v1/models` or static"
    without a response shape or pagination grammar. The rebuild therefore: sends **no
    `limit` parameter** (the gateway's `limit=1000` is the gateway listing's recorded URL
    shape, not a family-wide constant), follows a cursor `after_id` for at most three pages
    with a 10-second per-page timeout as the app's recorded listing policy (`docs/12` §3.1),
    and parses the page **shape-tolerantly** — a `data` or `models` array, an
    `id`/`model`/`name` string per row, a cursor under `after_id`/`next_cursor`/`last_id`.
    Nothing else is invented. Status: **Unknown** (native default page size and exact row
    grammar); mitigated structurally (the parse cannot misrepresent a row it cannot read).
25. **The Claude API family's own stream idle default was not separately captured
    (2026-10-23)** — `docs/12` §4.1 records "default 300 seconds" for the *gateway*; the
    official-endpoint family's default was never observed. The rebuild applies the
    documented 300-second reading to the family and shows the field nowhere (the Claude
    API setup card has no idle field). Status: **Unknown** (applied default).
26. **The health-check request shape is an adapter-level convenience, not a captured
    contract (2026-10-23)** — `docs/12` §8.2 names `healthCheck()` without a wire shape.
    The gateway crate reads `/v1/models?limit=1`; the Claude API crate reads the plain
    first listing page with no parameter. Both are cheap reachability probes; neither is a
    recorded spell. Status: **Available (implemented), shape unobserved**.
27. **The Bedrock Runtime wire was never captured; the native adapter's contract is a
    reconstructed reading (2026-10-23; Bedrock slice 3)** — `docs/12` §8.5 records the
    family as "AWS SDK Bedrock Runtime stream, Anthropic body" and `docs/14` §7 rows
    100/112/113 record that exact paths, payloads, and auth spellings were never
    observed. The rebuild therefore pins one reading, enforced by the wire fixtures:
    the runtime host is `https://bedrock-runtime.{region}.amazonaws.com` and the
    control-plane host is `https://bedrock.{region}.amazonaws.com` (no FIPS spelling
    is invented); the single optional override `inferenceBedrockBaseUrl` replaces both
    (the form records one override key). Requests go to
    `POST {runtime}/model/{model-or-arn}/invoke[-with-response-stream]`; the path
    segment keeps the ARN's `:` separators and percent-encodes everything else
    (notably `/`). The body is the Anthropic Messages grammar **without a `model`
    field** plus `"anthropic_version": "bedrock-2023-05-31"`. Auth is
    `Authorization: Bearer {material}`; the profile/SSO (`sso`), web-identity
    (`external-idp`), and vendor-profile kinds are refused fail-closed. Streaming
    carries AWS EventStream binary frames (CRC-32C Castagnoli) whose payload is the
    Anthropic event JSON keyed by `:event-type`, `:message-type: exception` frames map
    to terminal provider errors, empty payloads are transport keep-alives that never
    reset the idle window, and a frame shorter than 16 bytes or longer than 1 MiB is
    refused. Discovery reads `GET {control}/foundation-models` with **no invented
    query parameters**, follows `nextToken` under the app's 3-page/10-second policy,
    and parses rows shape-tolerantly (`modelId`/`id`/`model`). Token counting has no
    native endpoint and is refused, never estimated; the model echo is enforced only
    when the answer carries a `model` field; and a request that asks for beta
    identifiers is refused visibly because no beta transport is recorded for this
    surface. The applied idle default is the documented 300-second reading (item 25).
    Status: **Unknown** (native wire unobserved); mitigated structurally (fail-closed
    gates, shape-tolerant parses, wire fixtures pinning every choice).
28. **The Vertex AI wire was never captured; the native adapter's contract is a
    reconstructed reading (2026-10-23; Vertex slice 4)** — `docs/12` §8.5 records the
    family as "official Anthropic Vertex client / Google auth" and `docs/12` §1 names
    the host shape; exact paths, payload spellings, and the streaming shape were
    never observed (`docs/14` §7 row 100; the shipped implementation is minified,
    no source maps). The rebuild pins one reading, enforced by the wire fixtures:
    the base URL is `https://{region}-aiplatform.googleapis.com` (the required
    `inferenceVertexRegion` derives it; the form's optional
    `inferenceVertexBaseUrl` override replaces it **in full** — no other override
    key exists, and the tests never let the derived host reach the internet).
    Requests go to `POST {base}/v1/projects/{project}/locations/{region}/publishers/google/models/{model}:rawPredict`
    (and `:streamRawPredict` when streaming) — the standard Vertex AI
    raw-prediction routes; the path carries project, region, and the model
    (percent-encoded; only the fixed `:rawPredict`/`:streamRawPredict` suffix
    carries a literal `:`). The body is the Anthropic Messages grammar **without
    a `model` field** and **without any `anthropic_version` declaration** (none is
    recorded for this family; inventing one is the `AI-ADAPTER-10` failure shape).
    The credential is a resolved Google access token sent as
    `Authorization: Bearer {material}`: the adapter takes resolved material and
    never mints one — the interactive (Google sign-in), Workforce,
    `external-idp`, and vendor-profile kinds carry sign-in flows that were never
    captured and are refused fail-closed, and the credentials-*file* field is
    draft metadata only (the adapter never parses identity files; service-account
    JWT signing is an invented surface). The setup probe resolves **only the
    helper-script kind** (its command runs once via `run_helper`, never persisting
    — `docs/14` §9 item 21's reading), after the model gate. Streaming is
    *data-only* SSE — `data:` lines whose payload is one Anthropic stream-event
    JSON each, no `event:` name lines, an optional `data: [DONE]` sentinel that
    completes the turn when the message already named a stop reason; SSE comments
    and blank lines are transport keep-alives that never reset the idle window
    (`TEST-AI-016`). The model echo is **conditional** (as in item 27): an answer
    naming a different model ends the turn visibly; absence of a `model` field is
    not an echo failure. No native token count exists — the adapter errors instead
    of estimating (`AI-ADAPTER-09`); **no model listing either** — discovery is
    disabled for every credential kind the family offers (`docs/09` §2.2 "Vertex
    AI"), so the probe refuses drafts without a `claude-` id instead of guessing a
    catalog route (`AI-PROVIDER-05`); no beta transport is recorded — a
    beta-carrying request is refused visibly (item 27's reading); the applied
    idle default is the documented 300-second reading (item 25).
    Status: **Unknown** (native wire unobserved); mitigated structurally (fail-closed
    gates, shape-tolerant parses, wire fixtures pinning every choice).
    (2026-10-23 follow-up: the Vertex family was **removed from rebuild scope** at the
    owner's direction — `docs/04` §3.1. The adapter, its wire fixtures, the bridge
    probe lane, and the config form family were deleted; nothing Vertex-shaped is
    rebuilt. The reading above is retained as the unknown/research record; the code
    and fixtures are at `7b84886` on the `phase-3` lane. The `TEST-AI-003..` rows
    for this family are **Unsupported**, not deferred — `docs/17` §4.13.)

29. **The open model layer (owner decision, 2026-10-23).** The rebuild widens from the
    Anthropic-first family set into an **open multi-provider harness**: any model, any
    provider, plus a user-defined OpenAI-compatible endpoint (design reference: the
    DeepSeek harness). The owner-directed contents:
    - **No model-ID family policy remains.** Every family's validator checks shape only
      (`validate_open_model_id` — a non-empty string), and the former per-family gates
      (the `anthropic`/`gateway` non-Anthropic refusal, `validate_bedrock_model_id`'s
      `anthropic.*`/ARN rule) are gone from every send path. An id the provider itself
      rejects comes back as the provider's own wire error, mapped verbatim
      (`PROVIDER_ERROR` with the provider's message) — never a locally invented refusal.
      The flip tests pin this: `a_non_anthropic_id_travels_verbatim` (gateway and Claude
      API crates), `a_non_bedrock_id_travels_verbatim` (Bedrock crate — the path is
      `/model/{id}/invoke-with-response-stream`, not converse-stream), and the bridge's
      `test_connection_sends_the_first_draft_entry_verbatim`.
    - **Two OpenAI-compatible wire families join the surface:** first-party `openai`
      (base URL optional, default `https://api.openai.com/v1`; credential is a static
      key, required — an empty credential refuses at the adapter's `authenticate` before
      anything sends) and `custom` (base URL required at the lane level; credential
      OPTIONAL — item 30). Both ride the one native wall `crates/providers/openai`
      (`myharness-provider-openai`): the app-internal format stays Anthropic Messages,
      the adapter owns its per-provider wire, there is no shared transport plumbing, no
      router, no translation gateway. `AdapterKind` kebab-spells `"openai"`/`"custom"`.
    - **`mantle`/`foundry` stay stubbed.** Their forms exist; their probe lane refuses
      with the transitional message until their native adapters land (locked, not faked;
      `docs/17` §4.10's transitional-arm posture, unchanged).
    - Companion records: the keyless-send decision is item 30; the wire readings the
      battery pins are item 31; the discovery readings item 32.
    Status of the shipped layer: **Available** for `gateway`, `anthropic`, `bedrock`,
    `openai`, `custom`; **Unsupported** for `mantle`, `foundry` (stubbed until their
    adapters land); `docs/17` §4.14.

30. **The `custom` family's send is keyless by default (owner decision, 2026-10-23).**
    A user-defined OpenAI-compatible endpoint may legitimately want no credential at
    all. So the credential is OPTIONAL for `custom`: absent or blank material resolves
    through `resolve_optional_probe_credential` to an **empty `SecretString`,** and the
    adapter sends **no `authorization` header at all**
    (`a_keyless_custom_endpoint_sends_no_authorization`). The bridge's strict resolvers
    of the three original families are unchanged, and the helper-script credential kind
    stays offered for `custom`. The first-party `openai` family keeps its requiredness
    (item 29). A dangling or malformed stored handle remains an error on every path.

31. **The OpenAI Chat Completions wire readings** (never captured from a shipped
    surface; every choice pinned by the 21-fixture battery in
    `crates/providers/openai/tests/wire.rs`). `POST {base}/chat/completions` +
    `GET {base}/models`; `Authorization: Bearer` only when a credential exists; system
    blocks join `\n\n` into ONE `role:"system"` message; the body carries `max_tokens`
    (not `max_completion_tokens`); **no `stream_options`**; `top_k`, thinking, betas,
    and `cache_control` are absent from the body (the wire has no slot for them —
    nothing is "refused" because there is no field to refuse). Streaming: reasoning
    arrives via
    `delta.reasoning_content` (fallback `delta.reasoning`); tool arguments stream via
    `delta.tool_calls` with the synthesized id `chunk.id-{index}` (`call-{index}` when
    the chunk carries no completion id); close-assembling happens on `finish_reason`
    (the wire has no per-block stop event); an empty `data: {}` frame is a keep-alive
    Ping that never resets the idle window; `data: [DONE]` is required, its absence is
    an error; a mid-stream error chunk maps to a typed class. Mappings: stop →
    `end_turn`, length → `max_tokens`, tool_calls/function_call → `tool_use`,
    content_filter → `ContentFilter`, unknown → `Other` verbatim; usage is
    prompt/completion_tokens + `prompt_tokens_details.cached_tokens`; model echo is a
    PREFIX match (`gpt-4o-2024-11-20` is a legal echo for `gpt-4o`; a real
    substitution is `ModelNotFound`); documents and tool-result media are refused
    explicitly (`UnsupportedModality`) — nothing multimodal is silently dropped;
    assistant Reasoning/RedactedThinking blocks from replayed history are **omitted**
    — the OpenAI wire has no representation for them, and failing the whole send would
    defeat cross-provider histories (the pivot's core use case). **This omission is the
    one deliberate deviation from `RULES.md` §2's letter;** the owner decision stands
    behind it, and a fail-the-send flip is a one-line adapter change if reversed.
    count_tokens has no wire equivalent → explicit refusal, never an estimate
    (`AI-ADAPTER-09`).

32. **OpenAI-compatible discovery reading (owner decision, 2026-10-23).** One page,
    ten seconds, tolerant parse: a `data` array, a bare top-level array, or a keyed
    `models` object; bare-string rows accepted; the id is read from `id` / `model` /
    `name`; `context_window`/`context_length` are noted when present. The Tauri command
    `setup_discover_openai_models` is **additive** — the gateway's
    `setup_discover_models` stays untouched for the e2e fixture's stability. Keyless
    discovery follows item 30.
33. **No system prompt or context assembly in Phase 4 (explicit deviation, 2026-10-24).** The
    shipped bootstrap/system prompt and the context assembly (folders, memory, skills) were
    never captured. `TurnInput::text` sends an **empty system** for Cowork, so the model sees
    only the replayed transcript; the UI does not display a system prompt. Phase 6 Code adds
    exactly one context block naming the selected canonical folder (see item 57); it does not
    invent the shipped bootstrap prompt. This is a recorded deviation, not a claim that the
    shipped app sends none. Status: **Unknown (shipped prompt) → deliberate omission; Code
    folder block is an approved rebuild decision**.
34. **Retry/regenerate was never exercised; no retry action is built.** The composer spec
    describes retry with the same model, but the exact action, endpoint, and transcript
    semantics were not observed. The renderer offers no retry button and the bridge has no
    regenerate command; re-sending a retained draft appends a **new** user turn, which is not
    the same thing. Status: **Unknown**.
35. **The model is fixed per session; there is no picker and no token-capability
    validation.** A task stores the provider/model chosen at creation; the New screen shows a
    neutral "model" label, and the task surface shows the provider-qualified label. Changing
    models requires a new task. `max_tokens` defaults to 4096 and is capped at 32768; the
    rebuild has no model capability data, so it does not validate a request against a
    model's context/output limits — the provider's own wire error is the refusal. Status:
    **Partial** (label available; picker/capability data unavailable).
36. **Attachments and tools are deferred.** The `+` control is disabled with a reason; tool
    declarations are refused before provider contact. Approvals, permission requests, MCP
    execution, and the attachment pipeline belong to the tool/MCP phases and are not
    emulated. Status: **Unavailable** in this phase.
37. **Retention has an engine but no schedule or UI (unobserved).** `sessions::sweep_retention`
    filters by surface/age and honors global and per-session holds, but no background timer
    calls it and Settings exposes no retention control. The shipped schedule and screen were
    not observed, so neither is invented. Status: **Partial**.
38. **Usage is combined across surfaces; the window uses one fixed offset.** The ledger has no
    surface column, so the 7/30/90-day panel sums Cowork and Code together and says so. The
    device's current UTC offset is applied to the whole window; a DST transition inside the
    window is not re-bucketed (the shipped page's behavior across a transition was not
    observed). Cached tokens are shown separately and never folded into input/output. Status:
    **Available (unit); shipped per-surface/DST behavior Unknown**.
39. **Profile revision is not stored; a same-target profile swap is not detected.** The
    profile document has no revision field, so a session stores `providerId`, `profileId`,
    and the native model id only. A send refuses if the applied profile now serves a
    different provider/model (`MODEL_UNAVAILABLE`, no substitution), but a swap that keeps
    both can change the connection (base URL, region, credential) under the same model
    without the session noticing. Status: **Unknown (shipped semantics) → fail-safe on the
    provider/model identity only**.
40. **Draft and cancellation evidence is renderer/unit-level, not native-runtime.** Drafts are
    renderer-owned `localStorage` (`composer-draft:new`, `composer-draft:<sessionId>`),
    plaintext and unencrypted; they are retained on **any** failure (including a provider
    failure, per the composer spec) and cleared only when a turn completes. The New draft is
    moved into the session draft before the first await. A token cancelled before anything is
    persisted returns `CancelledBeforeSend` (`userTurnRecorded: false`); a cancellation after
    persistence is `TURN_CANCELLED` (`true`). The Tauri command/event wiring is
    compile-verified, and the browser E2E (35/35) has no Tauri host, so a live stream or
    usage read driven from the UI is **not** E2E-exercised. Status: **Available (unit +
    renderer E2E for storage); native UI stream unexercised**.
41. **MCP legacy HTTP+SSE is not implemented.** `rmcp` 3.7 carries SSE inside Streamable
    HTTP and does not expose the legacy separate SSE client used by the shipped app's
    `Sse` transport. `ServerConfig::Sse` is modelled and returns `UnsupportedTransport`
    rather than silently falling back to another transport. Status: **Unavailable
    (legacy SSE); Streamable HTTP Available (unit); shipped SSE semantics Unknown**.
42. **MCP timeout defaults and retry policy are unobserved.** The manager's start timeout
    defaults to `None`; `call_tool` takes a caller-supplied timeout because the shipped
    `mcpToolTimeoutSec` default was not captured. Dropping the call future may not cancel
    the server-side request. No retry/reconnect loop is invented. Status: **Unknown**.
43. **Tool-policy serialization and precedence are rebuild-defined.** The shipped
    `builtinToolPolicy` and `toolPolicyEndpoint` shapes were not captured. The rebuild uses
    tagged `ToolPattern`/`ArgumentMatch`, a fail-closed `deny > ask > allow > default(Ask)`
    precedence, a top-level subset argument match, and a SHA-256 canonical argument
    fingerprint. Deny cannot be overridden by a more specific allow. Status: **Rebuild
    decision; shipped policy shapes Unknown**.
44. **MCP approvals are durable, but the native prompt is unexercised.** Per-call/session/
    persistent scopes and the scheduled lifetime are implemented; `Persistent` is gated by the
    resolved `mcpPersistentAlwaysAllowEnabled`; `0` scheduled days offers only `Once`. New grants
    are written to `mcp/approvals.json` (0600) and chained as `mcp.tool.approved` with the tool
    name and argument fingerprint only; restore skips expired and `Once` records. The renderer
    answers `chat://tool-approval` through `respondToolApproval`. A pending prompt can be lost if
    the renderer navigates away, and `ToolRuntime::run_turn` holds one global approval lock across
    a turn, so tool turns serialize across sessions. Status: **Available (unit + scripted
    integration); native UI unexercised; prompt durability/concurrency gaps**.
45. **Egress matching is rebuild-defined and does not cover stdio or DNS/IP checks.** The
    configured policy is fail-closed, exact-host (case-insensitive), `http`/`https` only, and
    host-only (any port). `McpManager::with_egress` checks before `connect`; `None` means no
    policy was supplied. A stdio child can open its own sockets and is **not inspected**. The
    pinned `rmcp` 3.7 Streamable HTTP client follows only same-origin 307/308 redirects (up to
    10) and returns 301/302/303 or cross-origin redirects to the caller, so a redirect cannot
    swap the checked host; a custom transport client would need to re-check every hop. The
    production OAuth refresh client applies the same allowlist to the token URL and disables
    redirects before sending a refresh token. DNS/IP resolution and link-local/metadata targets
    are not checked. The shipped `allowedMcpServers`/egress allowlist defaults and port
    granularity were not observed. Status: **Available (unit, HTTP/SSE); stdio/DNS/IP
    Unknown**.
46. **MCP tool results have no audio path in the provider SDK.** The MCP layer preserves text,
    images, audio, embedded text/blob resources, resource links, and structured content; the
    provider-facing `ToolOutput` has no audio variant, so an MCP audio result becomes an explicit
    error. Tool results are sanitized to text/images/documents and capped at 1 MiB; unsupported or
    oversized output becomes a visible error. Status: **Partial; audio Unsupported**.
47. **MCP wiring is complete on the local/stdio path, with conformance gaps.** The agent tool loop,
    bridge commands/approval prompt, durable approvals, keychain OAuth, schema cache/tool search,
    skills, plugins, plugin MCP servers, app-boot `start_all`, and renderer surfaces are built.
    `TurnInput::validate` accepts tools only through `run_turn_with_tools`, which first
    capability-gates the model (`MODEL_UNSUPPORTED` when the model does not advertise tools).
    Remaining: no live HTTP MCP fixture, no live OAuth server/keychain exercise, no
    marketplace/signature verification, no persistent actionable failure toast, no model-driven
    on-demand tool search, no default MCP start/tool-call timeout, legacy SSE unsupported, no
    DNS/IP egress check, audio unsupported, and a bounded JSON Schema subset. Status:
    **Available (stdio + scripted integration); live HTTP/OAuth and several UX paths
    unexercised**.
48. **Skills are rebuild-defined and placeholders are non-runnable.** A skill is a directory with
    `manifest.json`; precedence is `Org > User > Plugin > Bundled`. Bundled names without captured
    bodies are listed as `runnable: false`; selection is refused. Scripts are never executed.
    Assets are declared-only, canonicalized inside the skill root, capped, and symlinked
    directories are rejected. Truncation is UTF-8-boundary safe. The shipped skill format and the
    Customize surface were not captured. Status: **Available (unit); shipped format/UI Unknown**.
49. **Plugins are local-only and signature verification is not implemented.** A plugin is a local
    `plugin.json` manifest installed into the managed catalog root; org bundles are read-only; no
    network marketplace or `.dxt`/`.mcpb` installer exists. `Required` signature policy refuses
    every install; `Optional` accepts unsigned/unverified plugins and reports `Unverified`.
    Plugin MCP servers are namespaced `<plugin>.<server>` and go through normal MCP policy/
    approval; disabled plugins contribute no servers or skills. Enablement changes apply on the
    next app start, never to a frozen session snapshot. Status: **Available (unit); shipped
    format/marketplace/signature Unknown**.
50. **Tool search is deterministic and UI/dev-only.** `toolSearchEnabled` defaults to `false`; the
    session picker (`mcpTools`) lists live executable tools regardless of that flag but filters
    disabled and policy-hidden tools. Search ranks live and cached hits with a rebuild-defined
    score; cached hits are metadata-only and cannot execute without a live session. There is no
    model-driven on-demand schema/tool search yet. Status: **Available (unit); model-driven search
    Unknown/deferred**.
51. **JSON Schema validation is a bounded subset.** `crates/agent/src/tool_schema.rs` enforces
    `type`, `enum`, `const`, `required`, `properties`, `additionalProperties`, `items`,
    `min`/`max`, `allOf`/`anyOf`/`oneOf`/`not`, and local `$ref`. `pattern`, `format`, and external
    `$ref` are not enforced; a tool server relying on them sees weaker validation than its schema
    declares. Status: **Partial; unsupported keywords are a recorded limitation**.
52. **Provider wire tool names are rebuild-defined.** Internal `<server>:<tool>` names are mapped
    to provider-safe `mcp_<sanitized-prefix>_<16-hex-sha256>` names before a request and mapped
    back before policy/execution. The shipped wire spelling was not captured. The mapping is
    deterministic, distinct, and bounded; a provider call whose wire name was not offered is not
    executed and becomes a visible error result. Status: **Available (unit); shipped spelling
    Unknown**.
53. **Code dashboard usage is local-ledger-only, and the all-time cap is a rebuild decision
    (2026-10-25).** The dashboard derives sessions/messages/tokens/active-days/peak-hour/
    favorite-model/model-share only from Code sessions' `usage_events` and `user`/`assistant`
    turns; Cowork rows are excluded by a `sessions.surface = 'code'` join. The definitions are
    rebuild-defined because the shipped dashboard's exact SQL/range semantics were not captured.
    All-time totals include every Code usage event and turn from the earliest Code activity (an
    event or a user/assistant turn), but the daily strip is capped at 3,660 local days (ten years)
    so the finite series cannot grow without bound. When the earliest activity is older than the
    strip, the response carries `truncated: true` and the UI says the strip starts after the
    earliest activity; the totals above it remain
    all-time. The cap is deliberately explicit rather than silently presenting a partial strip as
    complete. Status: **Available (unit/integration); shipped definitions Unknown; cap is an
    approved rebuild decision**.
54. **Accept-edits has no production edit-tool set (2026-10-25).** The permission gate can
    auto-approve a host-declared file-edit tool whose policy says `Ask`, but this slice ships no
    built-in file-edit tool and production passes no set. Consequently Accept-edits behaves like
    Default for every current MCP tool: approvals are preserved. The seam exists so the mode is
    enforced in the backend when file tools land, not merely stored as a renderer label. Status:
    **Partial; no implicit escalation, but the shipped Accept-edits behavior is not reproduced**.
55. **The Bypass policy key's exact shipped semantics were not captured (2026-10-25).**
    `disableBypassPermissionsMode` is treated fail-closed: Bypass is permitted only when the
    merged capabilities value is explicitly `false`. Absence, `true`, a string, or any malformed
    value disables Bypass. A present-but-malformed `isClaudeCodeForDesktopEnabled` or
    `disableDesktopLocalSessions` also fails closed (Code/local disabled), while absence keeps
    the documented default. This is a rebuild decision, not an observation. A stored Bypass mode
    is rechecked on every send, and Bypass never overrides a policy `Deny`. Status: **Strong
    inference / fail-closed decision**.
56. **Code placement, worktrees, terminal, Fast, and remote are explicitly deferred
    (2026-10-25).** This slice implements local folder sessions only. There is no PTY/terminal,
    SSH/WSL/remote placement, worktree/GitHub integration, or Fast-mode capability. The composer
    renders Fast visibly disabled and never silently falls back to local when another placement
    would be requested. The canonical folder is stored, revalidated before each send, and added to
    the model's system context, but it is **not a filesystem sandbox**: there are no built-in file
    tools, and an MCP server's process cwd is its own configuration. `TEST-CODE-003/005/006/007/008`
    and `TEST-SESSION-010` remain unimplemented, not silently passed. Status:
    **Unavailable/Policy-disabled, deferred; folder context is metadata + prompt, not confinement**.
57. **The native folder picker is compile-verified but not runtime-exercised (2026-10-25).**
    `pick_workspace_folder` uses `rfd`'s XDG portal backend on Linux and returns `None` on
    cancellation; `rfd` does not expose a portal failure separately from cancellation, so a
    failed portal is also observed as `None`. A browser preview reports `BRIDGE_UNAVAILABLE`. The bridge still treats the
    renderer-supplied path as untrusted input, canonicalizes it, requires an existing directory,
    and enforces configured allowed roots. The portal dialog itself was not exercised in this
    environment, and the populated dashboard/session screens were not run inside a Tauri window.
    Status: **Available (compile/unit); native UI unexercised**.
58. **The Code dashboard omits comparative copy and a provider column (2026-10-25).** The
    observed dashboard has a comparative activity line and a Models view; this slice renders the
    real all-time/7-day/30-day activity strip and model token shares, but it does not invent a
    previous-period comparison (the bridge has no prior-window query) and usage events do not
    carry a provider id. The Models tab therefore shows the model id, not a provider label. The
    session header still shows the actual model, and the composer says "Model from applied
    profile" because no model-catalog command exists yet. Status: **Partial; no fabricated
    comparison or provider label**.

59. **Projects: the populated list, the detail screen, and the create-flow
    split are an explicit rebuild design (2026-10-26).** The shipped app was
    observed only at the empty state, the choice dialog, and the form
    (`docs/features/projects.md`); nothing about a populated list, a detail
    route, or the difference between "Start from scratch" and "Use an
    existing folder" was captured. The rebuild renders a one-row-per-project
    list with the bridge-computed folder availability, a detail screen with
    the documented minimum (edit name/instructions, manage memory, open the
    folder, list sessions, delete), and both choice cards open the same form
    (the bridge never creates a folder, so "from scratch" cannot differ
    meaningfully). Status: **unobserved design, labeled as such**.

60. **Projects: folder availability is the canonical-resolution fact
    (2026-10-26).** A project or session folder "resolves" when
    `canonicalize(stored)` equals the stored spelling; a missing, deleted, or
    swapped-symlink folder reports unavailable and nothing recreates or
    retargets it. Sessions bound to a project keep the stale id after
    deletion and resolve "project missing" at read time; the deletion answer
    reports the orphaned-session count (`REQ-PROJ-06`). Status: **rebuild
    decision** (the shipped missing-folder handling was never exercised).

61. **Project instructions are stored and displayed but not injected into
    model turns (2026-10-26).** The shipped bootstrap/context assembly was
    never captured (`docs/14` §9 item 33), so the rebuild does not invent a
    prompt from project instructions. What does cross the boundary is the
    same decision Code already records: the working folder travels as a
    quoted data block ("path as data, not an instruction") on any
    folder-bound turn, Cowork included. Status: **Partial by evidence
    discipline**.

62. **Artifacts: the library derives from session rows; there is no artifact
    content model (2026-10-26).** `New artifact` creates one normal Cowork
    session carrying an `artifactTask` marker; the shipped auto-generated
    prompt was never captured, so nothing is auto-sent (the composer is
    focused; the user's first send starts generation — a recorded deviation
    from the captured "starts generation immediately"). The empty-state
    suggestion sends its own visible copy as the prompt. Versions, star/hide,
    share, export, and the viewer are **Unavailable**: the artifactsPane gate
    was unavailable during inspection and no artifact content schema was ever
    captured; the library never claims a completed artifact.
    `TEST-ART-004/005/006` are Unavailable/Partial accordingly. Status:
    **honest minimal slice**.

63. **Scheduled tasks: the schedule grammar, DST rules, and bounds are
    rebuild-defined (2026-10-26).** The shipped editor was never reached and
    `scheduled-tasks.json` was captured only at schema level. The rebuild
    accepts a 5-field cron subset (`*`, values, lists, ranges, `*/step` on
    minute/hour; day-of-month and month must be `*` — refused, not
    interpreted); a spring-forward gap means the occurrence does not exist
    (the next run is the next matching wall-clock time) and a fall-back
    ambiguity takes the earlier occurrence; an occurrence missed by more than
    60 s past its armed moment is a recorded skip ("the computer was asleep,
    offline, or the app was closed at the scheduled time"), never a late
    execution; names cap at 200 chars and prompts at 16 KiB, refused not
    truncated. Template names, descriptions, and schedule labels are the
    captured copy; the template *prompts* are the rebuild's prefill text.
    Status: **rebuild-defined, tested** (`TEST-SCHED-001..004` at the
    engine/bridge boundary).

64. **Keep-awake ships disabled (2026-10-26).** The switch renders with the
    captured label but cannot be turned on: no platform implementation exists
    in this build, and keep-awake can prevent idle sleep only — never lid
    close, power-off, or being offline (`REQ-SCHED-06`). `TEST-SCHED-005` is
    **Unavailable**. A run executes with the scheduled approval context (only
    `mcpScheduledTaskApprovalLifetimeDays`-bounded grants), and a deleted
    project or moved folder fails the run with an explicit unavailable state
    instead of retargeting (`REQ-SCHED-05`).

65. **Search is metadata-only, with a labelled navigation tail
    (2026-10-26).** The palette matches session titles and project names
    case-insensitively over the rows the bridge already serves; transcript
    content search is not built (the shipped transcript-search worker's
    behavior was never observed). The route entries ("Go to") are a
    rebuild-defined tail — the full result set in the browser preview (where
    no bridge exists), a labelled tail after data results on the desktop.
    The filter menu's Unread option is disabled (no unread tracking exists);
    pinned-task behavior was never observed and pinning is not built;
    Type options are surface-dependent because the sidebar task list is
    scoped by the Cowork|Code mode switch (the Phase 4 decision). Status:
    **Partial, honestly scoped** (`TEST-UX-003`).

66. **Memory: the file-based model and its baseline hash (2026-10-26).**
    Markdown entries are the truth; `index.json` is a derived cache that
    heals on open. A corrupt (non-UTF-8) file is renamed aside
    (`.corrupt-<ms>`) and named in the `events.jsonl` audit trail — never
    served, never deleted. Content bounds are refused, not truncated. The
    baseline hash (the `globalMemoryBaselineHash` analogue) is sha256 over
    sorted `id:content-hash` lines and is refreshed only by app-managed
    mutations, so an external edit stays detectable through
    `baseline_changed` instead of being silently adopted. Reset is explicit
    and audited. The shipped memory file's exact external-edit behavior was
    never observed; these semantics are the rebuild's recorded reading.

67. **Memory: the capability gate is fail-closed per read (2026-10-26).**
    `memoryEnabled` is read from the merged profile on every command —
    absent means **on** (the captured default), an explicit `false` or a
    malformed value means **off**. A broken memory directory degrades memory
    to `UNSUPPORTED`; it never blocks chat. The shipped toggle's malformed-
    value behavior was never observed; failing closed is the rebuild
    decision (`docs/14` §9 rule 4).

68. **The usage split view counts events, not turns (2026-10-26).** The
    Settings usage page's Cowork/Code shares (`docs/screens/settings.md`
    §Usage) are folds of `usage_events` rows joined to their session's
    surface — the same units the combined totals use, so a surface's
    numbers always add up against the combined window. The split covers the
    two surfaces the app writes (`chat`, `code`); the combined totals fold
    everything the ledger holds. Sessions deleted by their owner take their
    ledger rows with them (cascade), in the combined view and the shares
    alike. The shipped page's exact per-surface presentation (and whether it
    counted turns rather than events) was not observed; the chart and split
    here are the rebuild's reading.

69. **Retention deletes nothing unless a profile configures it
    (2026-10-26).** The captured Limits tab lists per-surface retention days
    and the legal-hold switch, but the shipped defaults were never observed,
    and a guessed default for a destructive sweep is the worst kind of
    invention. Absent or malformed day values therefore mean **no automatic
    deletion** for that surface; a malformed `sessionRetentionHold` fails
    closed to **hold** (suspending deletion is the safe direction). The
    rebuild has one chat surface (the Cowork task surface), so
    `coworkSessionRetentionDays` governs it; the shipped app's separate
    `chatSessionRetentionDays` has no distinct rebuild surface and is not
    silently mapped onto it. The sweep runs at boot — the shipped schedule
    was never observed, so there is no timer beyond that.

70. **The update feed is unobserved; verification fails closed
    (2026-10-26).** The shipped app's update wire format, host responses,
    and signing key were never captured (the banner offered `2.42304.2` and
    was not applied), so there is nothing to verify against and no client to
    write. The lifecycle state machine, the enforcement window (72 h
    default, `0` = immediate), the configuration relaunch window (24 h
    default), and the streaming-task session guard are implemented and
    tested in `crates/updates`; the production verifier refuses every
    bundle, so no bundle installs and the running version stays runnable —
    the Phase 7 exit criterion held by construction. This is the same
    posture as the catalog signature check (item 20): capturing the feed
    moves the refusal aside; it does not weaken the gate. **Never install
    without signature verification.**

71. **The Responses wall's readings are rebuild-defined, pinned by fixtures
    (2026-10-26, Phase 8 slice 1).** The OpenAI **Responses** wire
    (`POST {base}/responses`, its own input-item grammar and SSE stream) was
    never observed in the shipped app — Phase 8 extensions are rebuild
    additions — so every reading in `crates/providers/openai-responses` is a
    decision, not a capture: the family is **first-party OpenAI only** (a
    static key is required; there is no keyless `custom` variant of this
    wire); `store: false` is sent explicitly (a desktop harness with its own
    store does not opt into server-side retention by omission); system
    blocks join `\n\n` into one `instructions` string (`cache_control` has
    no spelling here and is dropped); documents map **natively** to
    `input_file` data URIs (the one wire that carries a PDF) and images to
    `input_image`; media inside tool results is **refused** on v1 (the
    result-content spelling for media was not pinned — a guessed shape the
    endpoint mangles is worse than an explicit refusal) and tool results
    travel as `function_call_output` with no `is_error` spelling; assistant
    `Reasoning`/`RedactedThinking` replay is **omitted** (this wire replays
    reasoning only through encrypted items the app does not hold);
    thinking budgets, `top_k`, stop sequences, and betas are dropped
    (recorded, no pinned spelling); the model echo is a **prefix match**
    (`AI-MODEL-01`); usage maps `input_tokens`/`output_tokens` +
    `input_tokens_details.cached_tokens`, and
    `output_tokens_details.reasoning_tokens` (a subset of output with no
    app channel) is dropped (`AI-USAGE-01`); stop maps
    `completed`→`end_turn` (or `tool_use` when the turn emitted function
    calls — the terminal event carries no per-choice finish reason),
    `incomplete_details.reason: max_output_tokens`→`max_tokens`,
    `content_filter`→`ContentFilter`, anything else verbatim
    (`AI-ADAPTER-08`); **there is no `[DONE]` sentinel** — the terminal
    events are `response.completed`/`response.failed`/`error`, and a stream
    that ends without one is contract-malformed (no stop reason invented);
    documented lifecycle events with no app mapping are consumed as no-ops,
    an **undocumented** event type is surfaced as an error (`AI-MM-01` — a
    grammar the adapter does not know may be carrying content it would drop
    silently); annotations surface verbatim as citations (`AI-STREAM-01`);
    tool calls assemble from `output_item.added` (the item's own `id`) →
    `function_call_arguments.delta` → `output_item.done` (`AI-TOOL-01`);
    the announcement's item id lives inside the item object — the
    top-level `item_id` belongs to the delta events; and token counting is
    an explicit refusal, never an estimate (`AI-ADAPTER-09`). The
    error-class mapping consults the envelope's `type` spelling then its
    `code` spelling — the stream's bare `error` event carries only a
    `code` (its `type` is the event name). The adapter and its battery are
    the whole of slice 1; slice 2 (2026-10-26) built the family's setup
    surface: `ProviderKind::OpenAiResponses` (serde `"openai-responses"`)
    with the family's own key spellings `inferenceOpenAiResponsesApiKey` /
    `inferenceOpenAiResponsesBaseUrl` (owner-defined rebuild keys, recorded
    like every open-layer family's — the Chat Completions slot never leaks
    onto this wire), the form family (static key required; base URL
    optional), egress/export, the strict-resolver probe lane (an absent
    credential is a `CONFIG_VALIDATION` refusal at the bridge with zero
    requests — the wire has no keyless variant, so unlike the `custom`
    family the refusal is the resolver's, not the adapter's), the chat
    factory arm (the provider id is the wall's own `id()` spelling,
    `openai-responses`), the additive discovery command, and the renderer
    unlock. The signed-package machinery (manifest, lazy loading,
    registration) remains unbuilt.

72. **Wire-battery mock traps (2026-10-26).** The Responses battery's
    mock-endpoint harness hit four durable traps, now fixed in
    `tests/wire.rs` and worth remembering for every future battery: (a) the
    listener must accept **non-blocking** with the stop flag re-checked
    between connections — a blocking `incoming()` leaves `Drop`'s `join()`
    waiting on an `accept()` that never returns once the tests stop
    connecting (the zero-request cases hang first); (b) a chunked SSE
    response must end with the terminating `0\r\n\r\n` chunk — without it
    hyper reads the close as a truncated body and errors the stream
    ("error decoding response body") instead of ending it cleanly; (c) a
    server must **read the request before answering** — a response head
    that arrives before the client's request body finished sending is a
    hyper send error ("error sending request"); (d) never wait for the
    client to close the connection (a keep-alive tail read): reqwest
    returns the completed connection to its pool and its close is async
    work on the test's runtime, which is blocked in `join` — the wait
    stalls for the pool's ~90 s idle eviction. The accepted socket is also
    forced non-blocking explicitly rather than relying on the listener's
    flag propagating.

73. **The provider coverage matrix and its deliberate deferrals
    (2026-10-26, final sprint).** The governing criteria (`docs/13` Phase 8:
    "at least one reference extension beyond the shipped six") are satisfied
    twice — the Responses wall and the local wall — so no further provider is
    REQUIRED by an acceptance criterion. The families' end-of-sprint state:
    gateway, anthropic (Claude API), bedrock, openai (first-party + custom),
    openai-responses, and local (Ollama/llama.cpp compat surface) are
    **implemented with wire batteries and full setup surfaces**;
    bedrock-mantle and foundry stay **stubbed and locked** — their forms
    exist, their probe lanes refuse with the transitional message, their chat
    sends refuse `ADAPTER_NOT_AVAILABLE` — because their native wires were
    never captured and the owner's open-model pivot made them lower priority
    than the shipped criteria; gemini and minimax-native are **deliberately
    deferred** — listed in `docs/13` Phase 8's candidate list but not
    required by any acceptance test, and each would repeat the now-proven
    extension pipeline at real cost; the deferral is recorded here rather
    than silently dropped from the roadmap. The local family's v1 scope is
    the runtimes' documented OpenAI-compatible surfaces — Ollama's native
    `/api/chat` NDJSON grammar is an unobserved reading recorded as unknown,
    and a LAN-hosted Ollama is out of the loopback-only v1 contract.

74. **The final-sprint threat-model review (2026-10-26)** lives in
    `docs/final-sprint-status.md` §threat-model (on disk, never committed):
    the ten boundaries walked, the findings fixed this sprint with their
    regression tests, and the accepted residuals. The sprint's substantive
    security work: the local wall's destination gate (loopback pinning,
    rebinding refusal, redirect refusal — regression-tested in the crate's
    battery) and the extension manifest's structural gates (traversal,
    integrity, size — regression-tested in `crates/extensions`).

75. **The first-profile creation is a rebuild-defined upsert (2026-10-27,
    whole-app campaign WA-06).** The shipped app's first-run setup flow was
    never observed (the inspected install always had profiles), and the
    rebuild's setup namespace had no creation path at all: `write` requires an
    existing entry, `duplicate` requires a source profile. The campaign's
    native sweep made the gap visible — the boot effect's no-profile early
    return also skipped the family catalog and tab schemas, so the picker was
    empty and every pane blank. Owner-direction was to continue the campaign
    and land it; the rebuild defines: the first-run draft is a blank local
    document with no id, `writeProfile` upserts an id-less draft by creating
    the entry (fail-closed: no provider family → refusal, nothing created),
    stamps the generated id, and answers with the stored document. Recorded
    as a rebuild decision, not an observation. Regression:
    `an_id_less_draft_creates_the_first_profile`,
    `an_id_less_draft_without_a_provider_is_refused` (bridge), setup E2E
    S17/S18. The click-through creation in the real release shell could not
    be completed: XTEST/PRIMARY text entry into the release WebKitGTK webview
    is lossy in the sweep's WM-less Xvfb session (environment limit, same
    class as the screen-reader gap in `docs/17` §4.20); the populated picker
    and the enabled Apply were verified on the release binary, the flow
    itself in the browser suite.

76. **Unreproduced silent shell exits under the sweep's Xvfb session
    (2026-10-27, whole-app campaign WA-07).** Twice, a release-shell instance
    under the campaign's isolated Xvfb/dbus session exited silently — once
    around a Projects navigation click, once during a setup-window
    interaction — each with no Rust panic, no core, and no stderr beyond the
    boot's MESA-EGL DRI3 warnings. Neither instance is reproducible on retry
    (instance 2 survived the full sweep including the same navigation);
    no assertion can be attached. Treated as an environment signal
    (WebKitGTK GPU-less EGL compositing under Xvfb without a window manager)
    rather than an app defect; `Unknown`, monitored — a recurrence under a
    real desktop session would reopen it as a P1.

77. **The setup window shows the user their own hosts (2026-10-28, whole-app
    campaign WA-08).** The renderer read had inherited the export bundle's
    private-host redaction, so a self-hosted base URL (the `custom` and
    `local` families' whole purpose) displayed as `<private-host>` and an
    untouched Apply failed URL validation. Campaign decision: host redaction
    is the export bundle's gate (`TEST-CFG-007/008` — a bundle leaves the
    machine, the setup window does not); the renderer read keeps handle,
    credential-shaped-value, and query-string redaction. `redact_document_for`
    applies `redact_private_hosts_in` only for the Export target;
    `renderer_read_keeps_the_users_own_hosts` pins both sides.

78. **The General pane's account-level fields stay unavailable (2026-10-28,
    interface-parity phase).** The documented General pane opens with a Profile
    block (avatar, work descriptor, "Instructions for Claude") that is
    account-level state the rebuild does not carry — the rebuild's profile
    concept is the third-party inference profile, not an Anthropic account. The
    pane states this and keeps the fields unavailable rather than inventing
    them; the three fields that need no account backend are built for real
    (chat font select — Anthropic Sans/Serif applied to the transcript;
    Motion System/Reduced, a reduced-motion switch; the response-completions
    notification switch wired to the Notifications API on turn completion).

79. **OpenRouter joins as a prebuilt family (2026-10-28, owner direction).**
    The owner asked for OpenRouter "like a prebuilt/inbuilt provider" after
    seeing the family list. OpenRouter speaks the OpenAI Chat Completions
    wire, so it rides the `crates/providers/openai` wall as a third preset
    kind (`AdapterKind::OpenRouter`, id `openrouter`): the endpoint is the
    wall's own preset (`https://openrouter.ai/api/v1`), the form carries a
    required static key and **no base URL field** (production profiles
    normalize to the preset; an explicit caller base still wins so the wire
    tests and the probe lane can point at a stub), and the catalog marks it
    probe/discovery-capable. The static-key families (openai, openrouter)
    now refuse an absent credential at the bridge with zero requests
    (`CONFIG_VALIDATION`, the responses posture) instead of at the adapter —
    `test_connection_blocks_an_openai_send_without_a_credential` was updated
    to the earlier, clearer refusal. Egress: `openrouter.ai`.
