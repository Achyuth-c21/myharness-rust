# 08 — Interaction Catalog

This catalog specifies how controls behave, including states that are easy to miss. “Observed” means
the interaction was exercised; “source-defined” means the behavior is evidenced by the shipped
bundle/API but not fully exercised.

## 1. Shell interactions

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-001 | Sidebar toggle | Title-bar button or `Ctrl+B` collapses/expands the sidebar; state persists in window layout |
| UI-INT-002 | Panel toggle | Title-bar control opens/closes an auxiliary pane when the surface supports it |
| UI-INT-003 | Back / forward | Navigates SPA history; disabled/greyed at history ends |
| UI-INT-004 | Cowork / Code switch | Segmented control changes mode and route; selected mode is visually raised and text-darkened |
| UI-INT-005 | New | Opens `/new`, clears the active composer context, focuses the composer |
| UI-INT-006 | Nav items | Hover fills a neutral row; active route shows a stronger fill; click navigates |
| UI-INT-007 | Task row | Click opens the task; hover reveals overflow; unread dot, pinned state, and status icon are independent |
| UI-INT-008 | Task overflow | Menu with Pin/Unpin, Mark unread/read, Rename, Archive, Delete; shortcut letters `P`, `U`, `R`, `A`, `D` are shown |
| UI-INT-009 | Rename | Inline edit or dialog; Enter commits, Escape cancels, empty names rejected |
| UI-INT-010 | Archive/delete | Destructive confirmation or undo pattern; delete is irreversible after confirmation |
| UI-INT-011 | Account row | Opens menu with Settings, Language, Inference configuration, changelog, learn more, sign out |
| UI-INT-012 | Update banner | Opens update flow; not activated during inspection |
| UI-INT-013 | Window close | `Ctrl+W`; on Linux without tray, quits after cleanup |
| UI-INT-014 | Window resize | Persists size/position; composer and transcript reflow; menus reposition |
| UI-INT-015 | Zoom | `Ctrl+0`, `Ctrl+Plus/=`, `Ctrl+-`; zoom factor persists |

The task menu letters are accelerators while the menu is open, not global shortcuts. **Observed.**

## 2. Search palette

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-020 | Open | Search icon or `Ctrl+K`; focus moves to query |
| UI-INT-021 | Type | Results filter live; query is trimmed; empty query shows recent/actions |
| UI-INT-022 | Arrow keys | Up/down change the highlighted result; list scrolls to keep it visible |
| UI-INT-023 | Enter | Opens the highlighted session/result |
| UI-INT-024 | Actions | Action rows are selectable (e.g. create/new); Tab moves focus |
| UI-INT-025 | Filter | Filter control opens Type/Status/Last activity/Group by/Sort by |
| UI-INT-026 | Close | Escape or outside click; focus returns to the trigger |
| UI-INT-027 | Empty results | “No results” message; query remains editable |
| UI-INT-028 | Long titles | Truncated with ellipsis; tooltip or full text available on focus/hover |

Search is scoped to local sessions/tasks in this build. Remote/cloud results are not evidenced.
**Observed** for palette structure and keyboard hints.

## 3. Filter menu

| ID | Filter | Values observed/defined |
|---|---|---|
| UI-INT-030 | Type | All types, Cowork, Code (labels dependent on surfaces) |
| UI-INT-031 | Status | Active, Archived, Unread, etc. |
| UI-INT-032 | Last activity | Any time, Today, Last 7 days, Last 30 days |
| UI-INT-033 | Group by | None, Date, Project, Status |
| UI-INT-034 | Sort by | Last activity, Date created, Name |
| UI-INT-035 | Apply/reset | Changes apply immediately; a clear/reset affordance is available |

Exact option labels should be copied from the locale catalog during rebuild; the menu was observed
open but not every combination was exercised. **Observed structure; strong inference on some values.**

## 4. Composer

### 4.1 Input and send

| ID | Control | Behavior |
|---|---|---|
| UI-INT-040 | Textarea | Multiline, auto-grows to a maximum, then scrolls; placeholder “How can I help you today?” |
| UI-INT-041 | Enter | Sends when the composer is focused, unless Shift+Enter inserts a newline |
| UI-INT-042 | Send button | Icon-only; disabled/neutral when empty; becomes primary when text/attachment exists |
| UI-INT-043 | Stop button | Replaces Send while streaming; stops the current turn and leaves the partial transcript |
| UI-INT-044 | Attachment files | `Ctrl+U`, `+` menu, or file input; multiple files supported; chip/list with remove |
| UI-INT-045 | Folder/project | “Project or folder” chip opens selector; selected context appears in the composer |
| UI-INT-046 | Permission mode | Selector for Code/Cowork; default label; bypass mode is visually distinct and warning-labeled |
| UI-INT-047 | Fast mode | Toggle in Code composer; state is visible and persisted per session/setting |
| UI-INT-048 | Model | Label shows provider/model; click opens picker; long IDs truncate with ellipsis |
| UI-INT-049 | Slash | Typing `/` opens command menu; arrows navigate; Enter inserts/sends; Escape closes |
| UI-INT-050 | Skills | `+` → Skills opens a flyout; choosing a skill activates/inserts it; exact mechanism unobserved |
| UI-INT-051 | Connectors | `+` → Connectors opens a flyout with per-connector toggles; persistence scope unverified |
| UI-INT-052 | Drag/drop | File drop is supported or strongly inferred from attachment architecture; visual drop state unknown |
| UI-INT-053 | Disabled send | No text/attachments, uploading, or policy-blocked provider leaves Send disabled with explanation |
| UI-INT-054 | Error | Attachment too large/unsupported shows inline error; failed send preserves draft |
| UI-INT-055 | Draft persistence | Drafts are local to the composer/session; exact restore behavior unknown |

Observed menu structure:

- **Add:** Add files or photos, Skills, Connectors, plugins.
- **Skills:** `consolidate-memory`, `context`, `docx`, `explain-usage`, `find-skills`,
  `frontend-design`, `pdf`, `pdf-reading`, `pptx`, `schedule`, `setup-claude`,
  `vercel-composition-patterns`, `xlsx`, plus Manage/Browse.
- **Connectors:** rows with icon, name, and state, plus **Manage connectors**. Names are
  profile-dependent and redacted from this repository.
- **Slash:** Add files, Usage, and skill/command names with icons; no descriptions, hints, or source grouping were visible.

The exact list is profile-dependent. Do not hard-code it as a universal set.

### 4.2 Composer states

| State | Visual/behavior |
|---|---|
| Empty | Placeholder, neutral send, folder chip, info icon |
| Typing | Send enabled/primary; textarea grows |
| Attached | File chips/thumbnails with remove controls; upload progress if slow |
| Streaming | Stop visible; input may remain editable or be disabled depending on mode |
| Permission-blocked | Send disabled or permission prompt; message explains policy |
| Provider missing key | Model row says “needs API key”; selection blocked or prompts configure |
| Offline | Send may fail inline; draft remains |
| Error | Red inline message; retry available |

## 5. Model picker and browser

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-060 | Open | Click model label; menu anchors above/below based on space |
| UI-INT-061 | Current model | Checkmark and highlighted row; full id in tooltip when truncated |
| UI-INT-062 | More models | Flyout lists additional flat rows; provider and native id are visible; grouping was not observed |
| UI-INT-063 | Needs key | Row includes “needs API key” description/info; selecting prompts configuration or is blocked |
| UI-INT-064 | Search | No search field was present in the captured picker or flyout; search/grouping is a proposed improvement, not observed |
| UI-INT-065 | Keyboard | Arrow keys navigate; Enter selects; Escape closes; focus returns |
| UI-INT-066 | Effort | Separate effort control/section; options low/medium/high/xhigh/max when supported |
| UI-INT-067 | 1M variant | Model may appear with `[1m]` or a 1M context badge/description |
| UI-INT-068 | Missing discovery | Empty state with retry/configure action; does not silently choose a different model |
| UI-INT-069 | Long list | Scrolls within viewport; no page overflow |

The current model IDs are from the local adapter and are **not** shipped model names. Do not use them
as canonical fixtures.

## 6. Projects

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-070 | Create new project | Opens dialog with two creation modes |
| UI-INT-071 | Start from scratch | Name, Instructions, Choose project location, **Memory is on**; Create disabled until valid |
| UI-INT-072 | Use existing folder | Native folder picker; selected path displayed; Create uses the folder |
| UI-INT-073 | Back | Returns to the first dialog step without saving |
| UI-INT-074 | Dismiss | Escape/close discards input after confirmation or immediately (unknown) |
| UI-INT-075 | Project context | Composer selector chooses a project/folder; context chip updates |
| UI-INT-076 | Project row | Opens project detail (unknown layout) |
| UI-INT-077 | Memory status | “Memory is on” in the form; exact toggle behavior was not exercised |

## 7. Scheduled tasks

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-080 | New task | Opens editor or menu; schedule and prompt required |
| UI-INT-081 | Template | Selecting a template pre-fills title/prompt/schedule |
| UI-INT-082 | Sort | “Next run” and other sort options |
| UI-INT-083 | Search | Local filter of tasks |
| UI-INT-084 | Keep awake | Toggle in banner; disabled by policy hides switch |
| UI-INT-085 | Run status | Last/next run and success/failure state (unknown exact layout) |
| UI-INT-086 | Edit/delete | Row overflow; confirmation for delete |
| UI-INT-087 | Offline/awake | If computer sleeps or is offline, the run is skipped; UI explains this |

## 8. Customize

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-090 | Tabs | Skills / Connectors / Plugins; selected tab underlined/filled |
| UI-INT-091 | Search | Filters cards/rows live |
| UI-INT-092 | Add | Menu for creating/uploading/installing; policy may hide options |
| UI-INT-093 | Skill card | Overflow menu for edit/delete/details; author/date metadata |
| UI-INT-094 | Connector filter | All / Connected / Not connected; updates row set |
| UI-INT-095 | Connect | Starts OAuth/credential flow; pending/error states |
| UI-INT-096 | Connector status | Running, connected, failed, needs auth; error includes details/action |
| UI-INT-097 | Plugin discover | Opens Directory/marketplace; install confirmation |
| UI-INT-098 | Plugin policy | User-added/upload options hidden when admin disables them |

## 9. Settings

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-100 | Open | `Ctrl+,` or account menu; opens at General/last section |
| UI-INT-101 | Section nav | Click changes hash and content; active section highlighted |
| UI-INT-102 | Search | Filters settings sections/rows; empty state for no match |
| UI-INT-103 | Toggle | Switch flips immediately; policy-locked toggles are disabled with explanation |
| UI-INT-104 | Select | Native/custom select opens; selection persists immediately or on close |
| UI-INT-105 | Theme | System/Light/Dark radio group; changes apply live |
| UI-INT-106 | Manage dialogs | Trusted folders, allowed sites, memory managers open secondary dialogs |
| UI-INT-107 | Close | X/Escape returns to route; changes are already persisted unless a form has Save |
| UI-INT-108 | Validation | Invalid numeric fields show inline errors and do not persist |
| UI-INT-109 | Developer config | “Edit config” opens the MCP JSON file; “View logs” opens the server log |
| UI-INT-110 | Import/export | Disabled state explains that the administrator must enable it |

## 10. Third-party setup window

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-120 | Profile selector | Switches between stored profiles; unsaved draft prompts before switching |
| UI-INT-121 | Tabs | Connection, Capabilities, Connectors, Telemetry & updates, Limits, Appearance, Plugins, Egress, Source |
| UI-INT-122 | Provider select | Replaces the connection form; credential kind options change |
| UI-INT-123 | Credential kind | Static API key / Interactive sign-in / Helper script; conditional fields |
| UI-INT-124 | Required fields | Marked `*`; Apply blocked or validation shown when missing |
| UI-INT-125 | Custom headers | Add/remove rows; header name/value validation; credential-like values rejected/warned |
| UI-INT-126 | Model discovery | Toggle; test/retry controls; errors surfaced without applying |
| UI-INT-127 | Apply | Validates, writes profile, and may relaunch; not exercised |
| UI-INT-128 | Discard | Reverts the draft to the applied configuration; used during investigation |
| UI-INT-129 | Export | Copies/downloads configuration; potentially contains secrets, treat as sensitive |
| UI-INT-130 | Source | Bootstrap URL; remote overlay values become read-only |
| UI-INT-131 | Test connection | Probes credentials/endpoint; not exercised |
| UI-INT-132 | Egress probe | Tests host reachability; not exercised |
| UI-INT-133 | Profile switch | May require relaunch; session guard may warn about active work |

### Provider form conditional behavior

- **Gateway:** base URL, API key or interactive sign-in/helper, auth scheme (bearer/x-api-key),
  artifact preview origin, custom headers, stream idle timeout, gateway-managed Claude Code, model
  discovery and explicit model list, proxy/VM network.
- **Claude API:** API key or interactive sign-in; base URL optional; model list; custom headers.
- **Bedrock:** region, profile/SSO/web identity/static bearer/helper; base URL; service tier;
  model IDs must be Anthropic-prefixed Bedrock IDs or inference-profile ARNs.
- **Bedrock Mantle:** region/base URL; credential; model routing.
- **Vertex AI:** project/region; Google sign-in/Workforce/helper/vendor profile; credentials file;
  model ID must start with `claude-` and contain no path/whitespace characters.
- **Foundry:** resource/base URL; API key/interactive/helper; deployment name must reference an
  Anthropic model.

## 11. Account menu and language

| ID | Interaction | Behavior |
|---|---|---|
| UI-INT-140 | Account menu | Opens above/below account row; current identity and provider shown |
| UI-INT-141 | Settings | Opens Settings modal |
| UI-INT-142 | Language | Opens dialog with locale list and checkmark |
| UI-INT-143 | Inference configuration | Opens/focuses separate setup window |
| UI-INT-144 | Changelog | Opens changelog (unknown surface) |
| UI-INT-145 | Learn more | Opens documentation/website in browser |
| UI-INT-146 | Sign out | Destructive/credential-clearing flow; not exercised |
| UI-INT-147 | Locale change | Applies strings immediately or after restart; native locale updated |
| UI-INT-148 | Language close | Escape/X; current locale unchanged unless selected |

## 12. Destructive and sensitive actions

Always require explicit confirmation or provide undo:

- Delete task/artifact/project/scheduled task.
- Archive all sessions.
- Delete/reset memory.
- Clear trusted folders or folder grants.
- Sign out / remove account.
- Apply a provider configuration that changes inference.
- Install a plugin/extension or grant MCP scopes.
- Enable bypass permissions.
- Export data or a support bundle.
- Install an update / restart.

The UI uses risk copy for bypass permissions and credential storage. Do not make destructive actions
single-click without an undo or confirmation.

## 13. Keyboard and focus rules

- `Esc` closes the topmost menu/dialog; it should not close the app.
- `Enter` activates the focused control; in a multiline composer, `Enter` sends and `Shift+Enter`
  inserts a newline.
- `Tab` moves forward through focusable elements; `Shift+Tab` moves backward.
- Menus support `ArrowUp/Down`, `Home/End`, and typeahead where implemented.
- The search palette supports `Ctrl+K` even when another control has focus.
- Focus returns to the invoking control when a dialog/popover closes.
- Disabled controls are skipped by keyboard focus.
- Native file dialogs suspend in-page focus until they return.

**Observed/expected.** Complete keyboard behavior for the transcript, artifact viewer, and Code panes
is unknown.

## 14. Interaction requirements

| ID | Requirement |
|---|---|
| `UI-INT-REQ-01` | Every async action has loading, success, error, and retry states. |
| `UI-INT-REQ-02` | Destructive actions require confirmation or undo. |
| `UI-INT-REQ-03` | Menus/popovers close on Escape/outside click and restore focus. |
| `UI-INT-REQ-04` | Composer draft survives recoverable errors and navigation away/back. |
| `UI-INT-REQ-05` | Model selection never silently falls back to another model. |
| `UI-INT-REQ-06` | Provider forms validate before applying and never persist partial credentials. |
| `UI-INT-REQ-07` | Native pickers are invoked through the main process, not direct renderer paths. |
| `UI-INT-REQ-08` | Status is announced accessibly and conveyed by more than color. |
| `UI-INT-REQ-09` | Shortcuts are platform-correct and discoverable. |
| `UI-INT-REQ-10` | No destructive or credential action is triggered by a single ambiguous icon click. |
