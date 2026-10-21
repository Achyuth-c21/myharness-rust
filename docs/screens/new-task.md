# Screen — New Task / Cowork Home

**Screen:** `UI-SCREEN-02`
**Route:** `/new` (also the default landing route)
**Features:** `FEAT-CHAT-01`, `FEAT-CHAT-02`, `FEAT-COWORK-01`

## Purpose

The default landing screen. It combines the Cowork greeting, the main composer, project/folder
context, and a compact list of active tasks. Sending creates a session and navigates to the task
detail screen.

## Layout

Measured at 1920×1080, 1× scale (re-attested 2026-10-21 from a user-provided reference capture of
the shipped app; supersedes the single-window figure below where they differ):

```text
┌────────────── 425 px ──────────────┬──────────────── remainder ─────────────────┐
│ title bar controls                 │                                             │
│ Cowork | Code switch (tab icons)   │      ✳ greeting (~40 px serif, ~34vh down)  │
│ + New                              │                                             │
│ icon: Projects / Artifacts /       │   ┌──── composer card ~735 px ────┐         │
│      Scheduled / Customize         │   │ placeholder                   │         │
│                                    │   │ +            claude  [send]   │         │
│ Tasks ›  [search] [filter]         │   └───────────────────────────────┘         │
│  task rows …                       │   ┌ Project or folder            (i) ┐       │
│                                    │   └──────────────────────────────────┘       │
│ Update available →                 │      Ideas for you                           │
│ ✳ account · provider  chevron      │      suggestion row (icon + label) ×3        │
└────────────────────────────────────┴─────────────────────────────────────────────┘
```

An earlier capture recorded the sidebar at ~321 px and the composer at ~720 px; both figures fit a
~22.2vw fluid sidebar rule (≈321 px near a default-sized window, ≈425 px at 1920). Treat the sidebar
and composer widths as fluid under that rule rather than fixed.

The main column is centered; the composer card and its selector strip and the suggestion rows share
a ~734 px content width. The sidebar scrolls independently. The account/update area is pinned to
the bottom.

## Regions

### Title bar

- Sidebar toggle, panel toggle, back, forward — the panel toggle sits second (2026-10-21 reference;
  earlier round-4 notes moved it right, the capture restores the documented order).
- Drag region; window controls follow the platform (frameless custom title bar on Linux). Observed
  on the home surface: a single round close control at the top-right corner; no minimize/maximize
  at this size.
- Back/forward are disabled when there is no history.

### Sidebar

- Segmented **Cowork / Code** control; each tab carries a small icon (shuffle-arrows / `</>`,
  2026-10-21 reference). Cowork is selected on this route.
- **New**, **Projects**, **Artifacts**, **Scheduled**, **Customize** — each row carries a small
  leading icon (re-attested 2026-10-21; exact glyphs left to the icon set, docs/07 §1.5).
- Tasks header with a trailing disclosure chevron plus search and filter icons.
- Task rows grouped by recency; pinned and unread states; warning icon for errored sessions.
- **View all** when the list is truncated.
- Bottom: **Update available** banner with a trailing arrow, then the account row — starburst
  avatar, display name, "· provider" label, disclosure chevron (re-attested 2026-10-21; the row's
  menu behavior is unchanged).

### Greeting

Observed string: **“You’re here!”** with an orange starburst to the left. Serif display type, large
— ~40 px at 1920×1080 (2026-10-21 reference re-attest; earlier note said ~36 px), sitting ~34% down
the window height. The Code surface uses a different greeting (“What’s up next, …?”) and must not
reuse this string.

### Composer

- Placeholder: **“How can I help you today?”**.
- Multiline input; card grows with content.
- Bottom row: plain `+` at the lower-left; plain model label at the right (reads “claude” in the
  captured build — not a pill, no chevron, 2026-10-21 reference); rounded-square outlined send
  button with an upward arrow at the far right (supersedes the earlier “light circular button” note).
- The **Project or folder** selector is a DISTINCT section directly beneath the input card — same
  width, its own rounded strip — containing folder icon + label + chevron, with the info icon at
  the far right of that strip (2026-10-21 reference).
- Model label truncation: unchanged (middle-ellipsis; hover exposes the full ID).

The model label is the current-state patched label in the captured build. A shipped rebuild shows the
configured provider/model; see [model-picker.md](model-picker.md).

### Active list

- Header **Active** and **Clear active** at right.
- Each row: surface icon, title, relative time, chevron on hover.
- No description column.
- Clicking opens `/cowork/<session-id>`.
- Empty active list: the header may remain with no rows; the screen does not show the Projects empty
  illustration.
- Empty home (re-attested 2026-10-21 reference): with no active tasks the area under the composer
  shows **“Ideas for you”** followed by three suggestion rows — “Send me a daily briefing”,
  “Organize my inbox”, “Customize Cowork for me” — each a small outlined icon + label. The Active
  list replaces this block when session data exists. Suggestion click behavior was not observed
  (Unknown); the rebuild reuses the existing composer draft (prefill + focus) rather than inventing
  a session start.

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
