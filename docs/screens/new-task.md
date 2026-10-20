# Screen — New Task / Cowork Home

**Screen:** `UI-SCREEN-02`
**Route:** `/new` (also the default landing route)
**Features:** `FEAT-CHAT-01`, `FEAT-CHAT-02`, `FEAT-COWORK-01`

## Purpose

The default landing screen. It combines the Cowork greeting, the main composer, project/folder
context, and a compact list of active tasks. Sending creates a session and navigates to the task
detail screen.

## Layout

Measured at 1920×1080, 1× scale:

```text
┌────────────── 321 px ──────────────┬──────────────── remainder ─────────────────┐
│ title bar controls                 │                                             │
│ Cowork | Code switch               │              greeting (~36 px serif)        │
│ + New                              │                                             │
│ Projects / Artifacts / Scheduled    │      ┌──── composer 720 px ────┐             │
│ Customize                          │      │ placeholder             │             │
│                                    │      │ +  model       send     │             │
│ Tasks  [search] [filter]           │      ├ Project or folder     (i)┤             │
│  task rows …                       │      └─────────────────────────┘             │
│                                    │      Active                    Clear active   │
│                                    │      task row                                    │
│ Update available                   │      task row                                    │
│ account · provider                 │                                             │
└────────────────────────────────────┴─────────────────────────────────────────────┘
```

The main column is centered; the composer and active list share a ~734 px content width. The
sidebar scrolls independently. The account/update area is pinned to the bottom.

## Regions

### Title bar

- Sidebar toggle, panel toggle, back, forward.
- Drag region; window controls follow the platform (frameless custom title bar on Linux).
- Back/forward are disabled when there is no history.

### Sidebar

- Segmented **Cowork / Code** control. Cowork is selected on this route.
- **New**, **Projects**, **Artifacts**, **Scheduled**, **Customize**.
- Tasks header with search and filter icons.
- Task rows grouped by recency; pinned and unread states; warning icon for errored sessions.
- **View all** when the list is truncated.
- Bottom: update banner, account row (display name · provider).

### Greeting

Observed string: **“You’re here!”** with an orange starburst to the left. Serif display type, large
(~36 px). The Code surface uses a different greeting (“What’s up next, …?”) and must not reuse this
string.

### Composer

- Placeholder: **“How can I help you today?”**.
- Multiline input; card grows with content.
- Bottom row: `+` add, model label centered, send arrow at right.
- Second row: **Project or folder** chip with folder icon and chevron; info icon at far right.
- Model label truncates with an ellipsis in the middle; hover exposes the full ID.
- Empty send is a light circular button; with text it becomes the primary action.

The model label is the current-state patched label in the captured build. A shipped rebuild shows the
configured provider/model; see [model-picker.md](model-picker.md).

### Active list

- Header **Active** and **Clear active** at right.
- Each row: surface icon, title, relative time, chevron on hover.
- No description column.
- Clicking opens `/cowork/<session-id>`.
- Empty active list: the header may remain with no rows; the screen does not show the Projects empty
  illustration.

## Interactions

| Action | Result |
|---|---|
| Type | Composer grows; send becomes active |
| Enter | Send/create session |
| Shift+Enter | Newline |
| `Ctrl+U` | Add files/photos |
| `+` | Add menu: files, Skills, Connectors, plugins |
| Project or folder | Project/folder popover with search, recent projects, Add folder |
| Model label | Model picker |
| Send | Create session and navigate to task |
| Active row | Open task |
| Clear active | Clear the active list (does not delete history) |
| Search icon | Search palette |
| Filter icon | Filter menu |
| `Ctrl+,` | Settings |
| `Ctrl+K` | Search palette |

## States

| State | Treatment |
|---|---|
| Empty history | Greeting + composer; no task sections |
| Active tasks | Active list with statuses |
| Streaming session | Active row indicates working |
| Errored session | Warning icon in sidebar/active row |
| Update available | Bottom banner with version/action |
| MCP error | Toast over bottom-right; does not displace composer |
| Provider missing key | Model row shows “needs API key”; send blocked with configuration action |
| Offline | Send error; draft retained |
| Narrow window | Sidebar may collapse; composer stays within viewport |

## Accessibility and keyboard

- Greeting is a heading; composer textarea has a programmatic label/placeholder.
- Add/model/project buttons have accessible names and visible focus rings.
- Enter-to-send must be suppressed during IME composition.
- Active rows are links/buttons with title + relative time in the accessible name.
- Focus returns to the composer after closing a picker.
- The update banner and MCP toast are announced non-disruptively.

## Data and APIs

- Creates a local session (`local_<uuid>`) and records model, cwd/folder, permission mode, enabled
  tools, and prompt.
- Active list is derived from session metadata, not a separate store.
- Drafts are renderer-owned (localStorage/IndexedDB); they are not authoritative session data.

## Evidence

- `50-new-task.png`, `54-new-clean.png`, `60-new-home.png`.
- `55-attach-menu.png`, `58-slash-menu.png`, `59-project-folder-menu.png`.
- `51/52-model-picker` for the composer model row.
- DOM tokens in `evidence/ui-tokens.json`.

Personal task titles/names in these captures are private; do not publish them.

## Test mapping

`TEST-UX-001`, `TEST-UX-002`, `TEST-SESSION-001`, `TEST-AI-008`, `TEST-UX-012`.

## Acceptance checks

1. Empty state renders at `/new` without a task flash.
2. Typing/Send creates exactly one session; double-click does not create two.
3. Shift+Enter inserts a newline; Enter sends.
4. Project/folder selection is reflected in the created session.
5. Missing provider key blocks send without selecting another model.
6. Active list row opens the correct session.
7. Focus/IME behavior works.
8. No personal task data appears in default fixtures.
