# Screen — Artifacts Library

**Screen:** `UI-SCREEN-10`
**Route:** `/cowork-artifact`
**Features:** `FEAT-ART-01`, `FEAT-ART-03`–`FEAT-ART-06`

## Purpose

The library of artifacts created by tasks. In this build the library is empty; the viewer/side pane
gate is unavailable. The page still exposes creation, search, and a view toggle.

## Layout

```text
Artifacts                                      [search] [view] [New artifact]
────────────────────────────────────────────────────────────────────────────
                         ┌──── folder/doc illustration ────┐
                              Create your first artifact
                         [ 👓 What needs my attention ]
```

Observed at 1920×1080:

- Serif **Artifacts** title at the left of the content column.
- Top-right: search icon, a list/view toggle icon, and a black **New artifact** button.
- Empty illustration: black outlined folder/document stack.
- Empty copy: **Create your first artifact**.
- Empty action: **What needs my attention** with a glasses/idea icon. This is a suggested artifact
  prompt, not a generic “Create” button.
- No artifact cards or fake content in the empty state.

## New artifact behavior

Clicking **New artifact** immediately:

1. Creates a live-artifact session.
2. Inserts a generated discovery prompt (asks what kind of live artifact would be useful, may inspect
   connectors).
3. Navigates to the task detail screen.
4. Starts generation.

There is **no modal** and no separate name/type form. This is an expensive side effect; the UI must
make the consequence clear and provide Stop. A stopped task remains in history.

The empty-state suggestion behaves similarly but with its own prompt. Do not implement it as a fake
card.

## Populated library

Not observed. The API supports artifacts with metadata, thumbnails, HTML content, versions, stars,
hide/show, share, import, auto-publish, MCP tools, and PDF export. A rebuild must design the
populated layout explicitly and label it as an unobserved design, not reconstruct it from imagination.

Recommended minimum:

- Grid/list of cards with thumbnail, name, last modified, star/hide actions.
- Search and filter.
- Empty/no-results states.
- New artifact action.
- Row menu for rename, share, export, delete.
- Selection opens an isolated viewer/pane.

## Viewer / side pane

`artifactsPane` is unavailable in this environment. The library must not render a fake pane. If the
gate is supported:

- Open a dedicated viewer route or side pane.
- Render content on the dedicated user-content origin/partition.
- No preload/bridge in the frame.
- Provide versions, share, export, star/hide, and delete.

## States

| State | Behavior |
|---|---|
| Loading | Skeleton; no empty-state flash |
| Empty | Illustration + suggestion CTA |
| Creating | Navigate to task; stop control |
| Stopped creation | Task remains; no phantom artifact |
| Populated | Cards/grid (unobserved) |
| No search results | Keep query + clear action |
| Viewer loading | Isolated frame loading state |
| Render error | Error in viewer; parent remains safe |
| Gate unavailable | Library works; no fake pane |
| Deleted | Confirmation; remove/hide per policy |

## Interactions

| Action | Result |
|---|---|
| New artifact | Create task + start generation |
| Suggestion CTA | Create task with suggested prompt |
| Search | Filter artifacts by name/metadata |
| View toggle | Switch grid/list (layout unobserved) |
| Card | Open viewer/pane |
| Star/hide | Update metadata; library filter |
| Share/export | Explicit dialogs; unobserved |

## Security

Artifact HTML/SVG is untrusted. See [features/artifacts.md](../features/artifacts.md) for the full
isolation requirements. The empty library itself has no untrusted content; the viewer does.

## Evidence

- `20-artifacts.png`, `69-artifacts.png`.
- `70-new-artifact.png` (live task, personal prompt; private archive only).
- `CoworkArtifacts` API inventory in `runtime-api-surface.json`.

## Test mapping

`TEST-UX-005`, `TEST-ARCH-006`, `TEST-SEC-001`, `TEST-SESSION-004`.

## Acceptance checks

1. Empty library matches the observed layout and copy.
2. New artifact starts exactly one task and navigates immediately.
3. Stop is available during generation; a stopped task is not shown as a completed artifact.
4. No modal is invented for creation.
5. Viewer is isolated when the gate is supported.
6. Search/no-results states remain usable.
7. Personal prompts/titles are absent from published evidence.
