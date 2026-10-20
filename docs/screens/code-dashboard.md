# Screen — Code Dashboard

**Screen:** `UI-SCREEN-04`
**Route:** `/epitaxy`
**Features:** `FEAT-CODE-01`–`FEAT-CODE-05`

## Purpose

The landing screen for Code. It summarizes local Code usage, lists repositories/sessions, and hosts
the Code composer for starting a session against a folder or repository.

## Layout

Observed at 1920×1080:

- Sidebar switches to Code mode:
  - **New**, **Customize**, **More**.
  - Repository list with per-row add (`+`), search, filter.
  - Sessions/starred sections.
  - Update banner/account footer.
- Main column:
  - Greeting: “What’s up next, `<display name>`?” with starburst.
  - Overview card (~520×380) centered above the composer.
  - Composer pinned near the bottom, ~830 px wide.
  - Small pixel-art robot decoration at the lower right.

The greeting uses the local display name. It is personal data and must be replaced with a placeholder
in published screenshots.

## Overview card

Header tabs: **Overview** and **Models**. Right-side range: **All**, **30d**, **7d**.

Overview:

| Stat | Description |
|---|---|
| Sessions | Number of Code sessions in range |
| Messages | User/assistant message count |
| Total tokens | Input + output tokens (cache handled separately) |
| Active days | Days with Code activity |
| Peak hour | Most active hour |
| Favorite model | Most-used model in range |

Below: an activity heatmap (weeks × days, blue intensity) and a comparison sentence. Values are
device-local analytics from the usage ledger, not provider billing. In published evidence, replace
all values with placeholders.

Models tab: model-level usage summary. Exact layout was not captured; it should show provider/model,
tokens, sessions, and share for the selected range.

## Composer

- Placeholder: **“Describe a task or ask a question”**.
- Context row above: **Local** (placement), folder chip, plus **Choose folder**.
- Bottom row: `+`, permission-mode selector, model label, send.
- The captured build shows **Bypass permissions** as the active mode; the warning treatment and policy
  gating are described in [code](../features/code.md).
- Fast mode is available when the selected model supports it.
- Send starts a Code session; no silent local fallback if a remote host/folder is selected.

## Sidebar

- Repositories/folders with add and search/filter.
- Sessions grouped by recency; starred items.
- “More” exposes additional Code management.
- Repository names are user data; do not publish.

## States

| State | Behavior |
|---|---|
| No repositories | Empty sidebar + folder chooser |
| Overview loading | Skeleton; no zero-value flash |
| No activity | Zero-state heatmap and explanatory copy |
| Session running | Active indicator; stop available in session |
| Folder unavailable | Error + choose another; no silent recreate |
| Remote disconnected | Reconnect/error; no local fallback |
| Update available | Bottom banner |
| Model missing | Send blocked; explicit picker action |

## Interactions

| Action | Result |
|---|---|
| All/30d/7d | Recompute overview from local ledger |
| Overview/Models | Switch summary tab |
| Choose folder | Native folder picker; sets cwd |
| Local placement | Local session; remote requires explicit host |
| Permission mode | Select policy-permitted mode |
| Fast mode | Toggle only if capability exists |
| Send | Create session and open Code session view |
| Repository add | Bind an existing folder/repo |
| Search/filter | Filter repositories/sessions |

## Data and APIs

- Overview is computed from `usage-ledger/*.ndjson` and Code session metadata.
- Folder selection goes through the main-process file picker; renderer receives a canonical path.
- Session creation records cwd, originCwd, worktree, model, permission mode, tool snapshot.
- No Anthropic-hosted analytics endpoint is called in 3P mode.

## Accessibility

- Cards have text labels, not color-only meaning.
- Heatmap has an accessible summary/table alternative.
- Range/tab controls are keyboard-operable and expose selected state.
- Composer controls have accessible names; the folder path truncates with a title/tooltip.
- The pixel-art decoration is decorative and hidden from assistive technology.

## Evidence

- `61-code.png` (personal names/stats; private archive only).
- `runtime-api-surface.json` for Code/session APIs.
- Settings text capture for Code preferences.

## Test mapping

`TEST-UX-001`, `TEST-CODE-001..003`, `TEST-SEC-002`, `TEST-ARCH-008`.

## Acceptance checks

1. Empty, loading, and populated overview states are distinct.
2. Range changes update stats consistently with the ledger.
3. Folder selection is reflected in the composer and session metadata.
4. Bypass mode is hidden/blocked when policy disables it.
5. Fast mode cannot be enabled for an unsupported model.
6. Remote placement never silently becomes local.
7. Published screenshots contain no repository names, stats, or user name.
