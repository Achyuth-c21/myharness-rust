# Screen — Third-Party Inference Setup

**Screen:** `UI-SCREEN-21`
**Route/window:** `app://localhost/setup-desktop-3p`
**Features:** `FEAT-SET-09`, `FEAT-ACCT-03`, `FEAT-MOD-08`
**Semantics:** [09](../09-settings-and-configuration.md), [features/third-party-setup.md](../features/third-party-setup.md)

## Window

This is a separate Electron window, not a modal in the main renderer.

Observed at 900×720 (content area):

```text
┌ Configure third-party inference ───────────── [● profile ⌄] [Export ⌄] ┐
│ ┌ search settings ─┐ ┌──────────────────────────────────────────────┐ │
│ │ Connection       │ │ Connection                                   │ │
│ │ Capabilities     │ │ Choose where Claude Desktop sends inference. │ │
│ │ Connectors   •   │ │ [ Gateway ⌄ ]                                │ │
│ │ Telemetry & upd. │ │ ┌ GATEWAY CREDENTIALS ───────── [Test] ────┐ │ │
│ │ Limits           │ │ │ Credential kind     [Static API key ⌄]  │ │ │
│ │ Appearance       │ │ │ Gateway base URL *  [http://…]          │ │ │
│ │ Plugins          │ │ │ Gateway API key *   [••••••••] [eye]    │ │ │
│ │ Egress           │ │ │ Auth scheme         [bearer ⌄]          │ │ │
│ │ Source           │ │ │ …                                        │ │ │
│ └──────────────────┘ └──────────────────────────────────────────────┘ │
│                                              [ Apply Changes ]        │
└───────────────────────────────────────────────────────────────────────┘
```

- Header title on the left; profile selector and **Export** at the right.
- Left nav ~217 px with search and nine entries.
- Content ~660 px, scrolls; footer Apply Changes is fixed.
- A small dot next to **Connectors** indicates a validation/error/unsaved condition. Do not interpret
  it without inspecting the tab.
- Apply is the only save action. Discard Changes appears when a draft differs (or in the profile
  menu); it restores applied values.

The profile name in the captured window is user-created and must not be published.

## Tabs

| Tab | Main content |
|---|---|
| Connection | Provider card, credentials, endpoint, models, proxy |
| Capabilities | Surfaces, restrictions, browser, tools, placement |
| Connectors | Managed MCP, approvals, extensions, history import |
| Telemetry & updates | Org UUID, telemetry blocks, OTel, update windows |
| Limits | Retention per surface, legal hold, token window |
| Appearance | Announcements, attribution, deployment labels |
| Plugins | Org marketplaces/plugins, tool policy |
| Egress | Derived allowlist + test/copy/download |
| Source | Bootstrap URL, managed/read-only warning |

Field-level semantics and validation are in [09](../09-settings-and-configuration.md). This screen
doc covers layout and interaction.

## Connection tab

- Provider selector is a full-width card/dropdown with provider icon. Changing it replaces the
  credential/endpoint form but does not apply until Apply.
- Credential section has a **Test connection** button at the top right.
- Two-column label/control layout; help text below labels.
- Required fields have a red `*`; password fields have reveal and lock icons.
- Selects use native-looking chevrons; switches are blue when on.
- The model section (below the fold) has discovery, static list, effort, 1M, pricing, catalog.
- Proxy/PAC and VM IPv6 are in the same scroll.

Provider switch is edit-only in this investigation. Changing fields without Apply should not alter
the active provider.

## Capabilities tab

- Card **Allowed surfaces**:
  - unified Chat/Cowork beta switch;
  - Cowork, Code, Chat rows with descriptions and switches.
- Card **General restrictions**:
  - allowed egress hosts tag input;
  - allowed workspace folders;
  - block reads outside working dirs;
  - auto mode/bypass permissions;
  - built-in tools/browser/Chrome/dictation/announcements/instructions;
  - placement restrictions.
- Nested rows are indented; switches align right.
- Changes are draft-only until Apply.

## Dialogs and validation

- Validation errors appear inline near fields or as an error summary; exact summary layout was not
  captured.
- Apply is disabled while invalid? Observed enabled in a valid profile; invalid behavior is unknown.
- Test connection can fail without blocking Apply? Unknown; do not persist test results.
- Export can include credentials; require warning/redaction.
- Discard requires confirmation if there are changes (expected).
- Closing with unsaved changes should prompt; exact behavior unknown.

## States

| State | Behavior |
|---|---|
| Loading | Window opens with skeleton/health state |
| Provider boot failed | Setup still opens; no blank app |
| Draft clean | Apply disabled/neutral |
| Draft dirty | Apply enabled; discard available |
| Invalid | Inline errors; Apply blocked or fails atomically |
| Testing | Spinner on Test; no credential write |
| Applying | Busy; previous profile remains until success |
| Relaunch required | Warning/session guard; explicit choice |
| Managed | Fields read-only with source |
| Health tainted | Banner + recheck/repair |
| Export | Warning; choose destination |
| Error | Error banner; do not discard draft |

## Keyboard and accessibility

- Window uses standard focus order: header controls, search/nav, content, footer.
- Tabs are links/buttons with selected state; arrow-key behavior expected.
- Labels are associated with controls; help text is descriptive.
- Password reveal is keyboard-operable; never read secrets aloud.
- Switches expose checked state; disabled/managed reasons are programmatic.
- Apply/discard are reachable without scrolling.
- Focus is not trapped in the scrolling content.

## Security

- Do not screenshot or publish API keys, tokens, profile names, private gateway URLs, MCP hosts, or
  the Egress list.
- The setup window shares the `app://localhost` origin with the main app; bridge APIs still check
  top-frame/origin. Treat it as privileged.
- Export/support bundle is sensitive.
- Using **Test connection** or **Apply** changes state; evidence collection should use Discard.

## Evidence

- `35-inference-config.png` through `43-setup-source.png`.
- Text captures `<private-archive>/evidence/screens/setup-3p-*.txt` (not in this repository).
- Provider form captures (no credentials).

## Test mapping

`TEST-CFG-001..012`, `TEST-AI-001..007`, `TEST-SEC-004`.

## Acceptance checks

1. Window opens independently and survives provider boot failure.
2. Tabs and provider switch preserve draft values.
3. Apply is atomic; failure leaves the active profile unchanged.
4. Discard restores applied values.
5. Managed fields are read-only and labeled.
6. Credential kinds are exclusive; Test does not persist.
7. Egress/export redaction works.
8. No secret appears in screenshots/docs.
