# Settings

**Features:** `FEAT-SET-*`
**Primary docs:** [09 — Settings and Configuration](../09-settings-and-configuration.md), [screens/settings.md](../screens/settings.md)

## Purpose

Settings is the end-user preference and local-control surface. Provider credentials and managed
policy live in the separate **third-party inference setup** window, not here.

## Structure

A centered modal over the current route with:

- left: search + section nav + Customize shortcuts;
- right: scrollable content;
- close X/Escape;
- hash-based sections;
- immediate persistence for most toggles (no global Save).

Sections observed:

1. General
2. Privacy
3. Usage
4. Claude Code
5. Cowork
6. Import & export
7. System
8. Developer
9. Customize links (Skills, Connectors, Plugins)

## Section notes

### General

Profile avatar/work description, global instructions, theme (System/Light/Dark), chat font, motion,
response notifications. Theme changes apply live and persist.

### Privacy

Third-party inference disclosure, what Anthropic may still receive (crash/usage/update signals),
nonessential services, and a diagnostic-report action. No local prompt/completion telemetry.

### Usage

7/30/90-day token usage and web searches for Cowork/Code, daily chart, provider-billing caveat.
Device-local; cache tokens separate and excluded from token limits.

### Claude Code

Code appearance, code font, interface font, transcript size/width/view, worktree location/branch,
session retention/archive, sandbox/browser controls, model-switch behavior, output style.

### Cowork

Working folder/change, trusted folders/manage, preferred browser/link handling/allowed sites, global
Cowork instructions, memory toggle/list.

### Import & export

Policy-disabled in this deployment. Shows an explanation rather than a broken action.

### System

Version/update state, run on startup, Quick Entry shortcut, tray preference, keep awake.

### Developer

Local MCP servers, edit config, view logs, add/remove server. MCP startup errors link here.

## Search

The settings search filters sections/rows. Empty state for no match; selecting a result should jump
to the section/row and highlight it. Exact ranking is unknown.

## Behavior rules

- Toggles persist immediately unless a section has an explicit Save.
- Policy-locked fields are disabled with a “managed by” explanation.
- Destructive actions (delete memory, clear trusted folders, reset) require confirmation.
- Theme/font/motion apply without restart.
- Locale changes in the separate Language dialog; restart requirements are unknown.
- Closing settings returns to the underlying route and task state.
- Settings never displays API keys/tokens; those are in setup only.

## States

| State | Behavior |
|---|---|
| Open | Modal + scrim; focus trapped |
| Section loading | Skeleton/spinner; no empty flash |
| Policy-locked | Disabled + reason |
| Invalid numeric | Inline error; not persisted |
| Search no match | Clear message + reset |
| Developer config missing | Explain + create/open path |
| Error loading | Retry; don't discard current state |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-SET-01` | Settings sections are navigable by hash and searchable. |
| `REQ-SET-02` | Toggles persist immediately and policy locks are visible. |
| `REQ-SET-03` | Theme/font/motion/transcript preferences apply live and persist. |
| `REQ-SET-04` | Usage is device-local and labeled as not provider billing. |
| `REQ-SET-05` | Destructive settings actions require confirmation. |
| `REQ-SET-06` | Settings closes without losing task/session state. |
| `REQ-SET-07` | No credentials are displayed in Settings. |

**Tests:** `TEST-UX-008`, `TEST-CFG-009`, `TEST-PRIV-001`, `TEST-SESSION-009`.
