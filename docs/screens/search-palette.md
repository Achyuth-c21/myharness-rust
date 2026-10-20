# Screen — Search Palette and Filter Menu

**Screens:** `UI-SCREEN-22`, `UI-SCREEN-23`
**Triggers:** `Ctrl+K`, sidebar search icon; filter icon in the Tasks header
**Features:** `FEAT-SHELL-05`
**Semantics:** [features/search-and-filter.md](../features/search-and-filter.md)

## Search palette

- Centered overlay ~726×470 px, white, rounded, strong shadow.
- Scrim over the main app; sidebar still visible.
- Search row ~48 px:
  - leading magnifier;
  - placeholder **Search chats and projects**;
  - clear `X` at right.
- Result list with a scrollbar.
- Each row:
  - leading icon: task list for Cowork, `</>` for Code;
  - title;
  - right-side relative date/time (`Today`, etc.);
  - selected row is a light-gray rounded highlight across the row;
  - a small keyboard/enter affordance may appear at the selected row's right.
- Observed query results are sessions/projects, not action commands. Do not claim action rows.
- No footer legend is visible in the capture.

The palette is local search over session metadata/transcript index. In 3P mode it does not query an
Anthropic account.

### Behavior

| Action | Result |
|---|---|
| Open | Focus search; select first result |
| Type | Filter live; clear button appears |
| `ArrowUp/Down` | Move highlight; scroll into view |
| `Enter` | Open selected result |
| `Escape` | Close; restore focus |
| Outside click | Close |
| `X` | Clear query, keep palette open |
| No results | Empty message; query retained |
| Indexing | Progress/loading; do not show a false empty |

### States

- Closed.
- Loading/indexing.
- Recent/empty query (expected; not captured).
- Results.
- No results.
- Error (non-blocking; query retained).
- Many results (scroll/virtualize).
- Filtered by active sidebar filters? Unknown; define explicitly.

### Accessibility

- Dialog with a labelled search input.
- Combobox/listbox semantics; `aria-activedescendant` follows the highlight.
- Result count announced politely.
- Title and date are in the accessible name.
- Focus trap while open; restore to the trigger on close.

## Filter menu

Anchored to the Tasks filter icon; ~230 px wide, white, rounded, shadow.

Rows:

| Row | Observed value | Submenu |
|---|---|---|
| Type | All | Yes |
| Status | Active | Yes |
| Last activity | All | Yes |
| Group by | None | Yes |
| Sort by | Last activity | Yes |

Each row is a label + current value + chevron. Selecting opens a submenu with options; changes apply
immediately to the sidebar list. No Apply/Clear button is visible; a reset may live in a submenu or
appear when a filter is active (unknown).

Expected options (from UI copy/runtime):

- Type: All / Cowork / Code.
- Status: Active / Archived / Unread.
- Last activity: All / Today / 7 days / 30 days.
- Group by: None / Date / Project / Status.
- Sort: Last activity / Date created / Name.

### Behavior

- Applying a filter updates the task list without closing the app.
- Multiple filters compose (AND).
- Active filters should be discoverable (count/badge), even after the menu closes.
- Pinned tasks may remain visible; behavior is **unknown** and must be defined.
- Empty filtered list must show “No tasks” + reset, not the normal empty illustration.

## Evidence and caution

- `63-search.png` — contains personal query/results; private archive only.
- `64-filter.png` — filter values are generic.
- Runtime API and localStorage keys for search/filter state.

Do not publish search queries, task titles, or dates from personal sessions.

## Test mapping

`TEST-UX-003`, `TEST-PRIV-001`, `TEST-UX-012`.

## Acceptance checks

1. Palette opens from keyboard and icon; focus is trapped/restored.
2. Results update live and keyboard selection stays visible.
3. Escape/outside click close without navigating.
4. No-results/error/indexing states are distinct.
5. Filters compose and have a visible reset.
6. Filtered-empty state differs from true empty.
7. Queries are never sent to telemetry.
8. Personal search results are absent from fixtures/screenshots.
