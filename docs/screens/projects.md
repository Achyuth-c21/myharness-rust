# Screen — Projects

**Screens:** `UI-SCREEN-06` (empty), `UI-SCREEN-07` (choice dialog), `UI-SCREEN-08` (create form)
**Route:** `/projects`
**Features:** `FEAT-PROJ-01`–`FEAT-PROJ-05`

## 1. Projects page

```text
Projects                                      [search] [sort] [New project]
────────────────────────────────────────────────────────────────────────────
                         ┌──── illustration ────┐
                              Looking to start a project?
                         Give Claude a folder you already work from
                                   [ New project ]
```

Observed:

- Serif page title **Projects** at the left of the content column.
- Top-right icons: search, sort, and a black **New project** button.
- Empty illustration (folders/hand), **“Looking to start a project?”**, explanatory copy, and a
  single outlined **New project** button. The header also has a black **New project** button.
- The two creation paths live in the dialog, not on the empty page.
- No sample projects and no fake counts.
- Populated list was not observed; do not invent card/table geometry.

The page uses the full main content width, not the narrow composer column.

## 2. Create a new project — choice dialog

Modal:

- ~560 px wide, centered, white surface, large radius, scrim.
- Title: **Create a new project**.
- Description: “A dedicated place for ongoing work, where context builds over time. Files and
  instructions stay in a folder on your computer.”
- Close X at top right.
- Two stacked option cards:
  1. **Start from scratch** — “Set up a new folder with instructions and files.” Plus icon, chevron.
  2. **Use an existing folder** — “Give Claude a folder you already work from.” Folder icon, chevron.
- Escape/outside click closes; whether a confirmation appears for a partly filled form is **unknown**.

## 3. Start a new project — form

Observed form title: **Start a new project**.

- ~560 px wide, centered.
- **Name** — required text input; placeholder `Project name`.
- **Instructions** — multiline; placeholder “Tell Claude how to work in this project (optional)”.
- **Choose project location** — info icon; a folder field with folder icon and placeholder
  `Select a folder...`.
- Footer left: **Memory is on** (memory icon + status/link). The capture shows it as a status/control,
  not a checkbox row; exact toggle behavior was not exercised.
- Footer right: **Back** (secondary) and **Create** (primary).
- Create is disabled until the name and folder are valid (observed disabled state).
- Focus starts in the Name field; Tab reaches instructions, folder, memory, Back, Create.
- Exact validation copy was not captured; use field-level errors and keep input values.

The “use existing folder” flow likely selects a folder and still collects name/instructions; it was
not exercised.

## 4. States

| State | Behavior |
|---|---|
| Loading | Skeleton list; no empty flash |
| Empty | Illustration + one New project entry |
| Choice dialog | Two cards; close/back |
| Form | Required fields; Create disabled until valid |
| Native picker open | Modal stays; no duplicate dialog |
| Invalid folder | Inline error; do not create |
| Duplicate name | Allowed or warned (unknown); identity is the project id |
| Creating | Button busy; prevent double submit |
| Create error | Error banner; values retained |
| Populated | List/table (unobserved) |
| Missing folder later | Project marked unavailable; no silent recreation |

## 5. Interactions

| Action | Result |
|---|---|
| New project | Opens choice dialog |
| Start from scratch | Opens the form |
| Use existing folder | Opens form/folder flow |
| Choose folder | Native picker; canonical path |
| Memory is on | Opens memory control or toggles; exact behavior unobserved |
| Create | Persists project; closes dialog; list/detail updates (detail unobserved) |
| Back | Returns to choice dialog, preserving fields if any |
| Escape/X | Closes without creating |
| Search/sort | Filters/sorts projects; layout unobserved |

## 6. Data

Project records should include name, instructions, canonical folder, memory flag, timestamps, and
id. Project memory is separate from global memory. The session composer can select a project as
context. See [11](../11-data-and-storage.md), [features/projects.md](../features/projects.md).

Exact API/store shape for projects was not reverse-engineered; the UI evidence and session fields
(`cwd`, project/folder context) are the basis. Do not claim a specific schema as verified.

## 7. Accessibility

- Modal is a labelled dialog; focus is trapped and restored to New project.
- Option cards are buttons with full title+description accessible names.
- Name/instructions have explicit labels.
- Folder field exposes the chosen path; picker is keyboard-accessible.
- Create’s disabled state is programmatic, not color-only.
- Error text is associated with the field.

## 8. Evidence

- `10-projects-empty.png`, `66-projects.png`.
- `11-projects-new-dialog.png`, `67-new-project-dialog.png`.
- `12-project-start.png`, `68-project-form.png`.

The folder path in the form capture is personal and must be masked in publication.

## 9. Test mapping

`TEST-UX-004`, `TEST-SESSION-005`, `TEST-SEC-002`.

## 10. Acceptance checks

1. Empty state has one New project entry and no fake projects.
2. The dialog offers both paths; Create remains disabled until valid.
3. Folder selection is required for the existing-folder path.
4. Create is idempotent; double-click does not create two projects.
5. Memory state is visible and persisted.
6. Escape closes without creating; focus returns.
7. Missing folder later is an explicit error.
8. Screenshots mask local paths.
