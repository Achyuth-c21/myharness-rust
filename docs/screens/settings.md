# Screen — Settings

**Screen:** `UI-SCREEN-18`
**Trigger:** account menu → Settings, or `Ctrl+,`
**Sections:** `#settings/general`, `#settings/data-privacy-controls`, `#settings/usage`, `#settings/claude-code`,
`#settings/cowork`, `#settings/import`, `#settings/desktop`, plus hash variants observed at runtime.
**Features:** `FEAT-SET-01`–`FEAT-SET-07`

## Modal geometry

Observed at 1920×1080:

- Scrim over the active route.
- Modal ~1400×960, centered (x≈260, y≈48).
- Left nav ~208 px, light-gray panel; right content on white.
- Top-left search field ~175 px wide.
- Close X at top-right of the content.
- Content scrolls independently; nav stays fixed.
- No global Save/footer. Toggles save immediately.

## Left navigation

Groups:

1. **Settings** — General, Privacy, Usage, Claude Code, Cowork, Import & export.
2. **This computer** — System, Developer.
3. **Customize** — Skills, Connectors, Plugins.

The active section has a filled rounded highlight. Selecting a Customize item navigates to the
Customize route (it is a shortcut, not a settings pane).

## General

Observed:

- **Profile** heading.
- **Avatar** — circular `?` placeholder; no uploaded avatar in this profile.
- **What best describes your work?** — select (`Select` placeholder).
- **Instructions for Claude** — multiline, placeholder “e.g. keep explanations brief and to the
  point”; helper text explains it applies across associated accounts/chats/Cowork.
- **Appearance**:
  - Theme segmented icon control: System (monitor), Light (sun), Dark (moon).
  - Chat font select: `Anthropic Serif`.
  - Motion: System / Reduced segmented control; helper text “Reduce animation in streaming responses
    and other interface elements.”
- **Notifications**:
  - Response completions switch; helper text “Get notified when Claude has finished a response.
    Useful for long-running tasks.”

No name field was visible; the avatar/identity comes from the local account.

## Privacy

Disclosure page with sections/cards explaining third-party inference, what Anthropic may still
receive, nonessential services, and diagnostic upload. It is not a local telemetry dashboard. See
[features/updates-telemetry-privacy.md](../features/updates-telemetry-privacy.md).

## Usage

- Range selector 7d/30d/90d.
- Cowork and Code token/search summaries.
- Daily token chart.
- Billing caveat. Values are device-local and personal; never publish.

## Claude Code

Code appearance, code font, interface font, transcript size/width/view, worktree location/branch,
session retention/archive, sandbox/browser, switch-model behavior, output style. Exact controls are
listed in [09](../09-settings-and-configuration.md).

## Cowork

Working folder and **Change**, trusted folders and **Manage**, preferred browser/link handling/
allowed sites, Cowork instructions, global memory toggle and memory list. Memory content is personal
and local-only.

## Import & export

Policy-disabled state: explanatory copy says import is unavailable because the administrator has not
enabled it for third-party inference. The export switch exists in config but is inactive. This is not
a broken control.

## System

Version/update state, run on startup, Quick Entry shortcut (feature-gated), tray preference (no tray
on this Linux desktop), keep computer awake. Exact labels are in [09](../09-settings-and-configuration.md).

## Developer

Local MCP server list, edit config, view logs, add/remove. MCP error toasts link here. Do not show
command lines/tokens in published captures.

## Search

- Query filters section names/rows.
- Empty/no-match state.
- Selecting a result navigates to the section/row.
- Ranking/jump highlight behavior is **unknown**.
- `Escape` closes Settings only when search isn’t consuming it (expected).

## States

| State | Behavior |
|---|---|
| Open | Focus trap; Escape/X closes |
| Section load | Skeleton/spinner |
| Policy-locked | Disabled control + “managed” explanation |
| Invalid number | Inline error; not persisted |
| Save failure | Error; previous value retained |
| Search no match | Message + clear |
| MCP error | Persistent toast with Developer link |
| Theme change | Applies live; no restart |
| Font/motion | Applies live |
| Close | Returns to underlying route/scroll |

## Controls and behavior

- Theme: System follows OS; Light/Dark persist. Changing theme updates tokens immediately.
- Chat font: select; changing it must re-render already-visible text.
- Motion: System respects OS `prefers-reduced-motion`; Reduced forces it.
- Notifications: OS notification permission errors must be visible; no fake “on” state.
- Account/instructions changes are local; no Anthropic account API in 3P.
- Destructive actions require confirmation.

## Accessibility

- Modal is a labelled dialog; focus is trapped.
- Left nav is a list of buttons/links with current-section state.
- Theme/motion segmented controls expose radio semantics.
- Search has a label and announces result count.
- Scrollable content receives focus or is reachable by keyboard.
- Closing restores focus to the account menu/settings trigger.

## Evidence

- `26-settings.png`, `71-settings-general.png`.
- `27-settings-privacy.png`, `28-settings-usage.png`, `29-settings-claude-code.png`,
  `30-settings-cowork-top.png`, `31-settings-import-export.png`, `32-settings-system.png`,
  `33-settings-developer.png`.
- `73-settings-dark.png`.
- Text captures in `<private-archive>/evidence/screens/settings-*.txt` (not in this repository).

## Test mapping

`TEST-UX-008`, `TEST-CFG-009`, `TEST-PRIV-001`, `TEST-SESSION-009`.

## Acceptance checks

1. All sections open by click and hash; active state is correct.
2. Theme/font/motion apply live and persist.
3. Toggles save immediately; failures revert visibly.
4. Policy-locked controls are disabled with reasons.
5. Usage is local and never mislabeled as billing.
6. Developer/MCP error links work.
7. Closing preserves the underlying task.
8. Screenshots mask paths, usage, memory, and account identity.
