# Projects

**Features:** `FEAT-PROJ-01`–`FEAT-PROJ-06`
**Primary docs:** [screens/projects.md](../screens/projects.md), [11](../11-data-and-storage.md)

## Purpose

A project groups a working folder, instructions, and memory so repeated tasks share context. It is
distinct from a task/session and from a plain folder.

## Projects page

- Route: `/projects`.
- Serif **Projects** title, top-right search, sort, and a black **New project** button.
- Empty state (observed): folder/hand illustration, **“Looking to start a project?”**, “Give Claude a
  folder you already work from”, and a single outlined **New project** button.
- There is **one** empty-state creation action. The two-path choice appears after clicking it, inside
  the dialog. Older notes that described two empty-state buttons were corrected from screenshot 66.
- Populated state/list layout was **not observed** (no project was created). Do not invent a card
  grid; design it explicitly in the rebuild.
- No fake sample projects are present.

## Create flow

### Step 1 — choice dialog

**Title:** **Create a new project**. Description: “A dedicated place for ongoing work, where context
builds over time. Files and instructions stay in a folder on your computer.”

Two option cards:

1. **Start from scratch** — “Set up a new folder with instructions and files.” Plus icon.
2. **Use an existing folder** — “Give Claude a folder you already work from.” Folder icon.

Both have chevrons. A close X is visible; Escape/outside-click dismissal is inferred, and confirmation
behavior for a dirty form is **unknown**.

### Step 2 — project form

Observed title: **Start a new project**. Captured width ~560 px.

| Field | Observed behavior |
|---|---|
| **Name** | Required text input; placeholder `Project name` |
| **Instructions** | Multiline, optional; placeholder “Tell Claude how to work in this project (optional)” |
| **Choose project location** | Info icon; folder field with folder icon and placeholder `Select a folder...` |
| **Memory is on** | Footer status/control with memory icon; on in the capture. Exact toggle behavior unverified |
| **Back** | Secondary; returns to the choice dialog |
| **Create** | Primary; disabled until required fields are valid (observed disabled state) |

There is no “What are you working on?” label and no radio group in the observed form. The earlier
description of a start-from-scratch/existing-folder radio was not supported by the screenshots.

Validation observed:

- **Create** is disabled while the required fields are empty; it was not exercised with valid input.

Rebuild rules (original copy/behavior not captured; label these as implementation decisions):

- Reject empty/whitespace names and require a folder path.
- Canonicalize the path and check it against policy.
- Duplicate names may be allowed; use the project ID as identity (strong inference).
- Do not enable Create while a native folder picker is unresolved.
- Use accessible field-level errors and retain input; exact inline copy was not captured.

## Project context in the composer

The composer’s **Project or folder** chip opens a selector with search, recent projects/folders, and
**Add folder**. Selecting a project:

- sets the working context for the task;
- may expose project instructions/memory to the model;
- is recorded in session metadata (`cwd`, project/folder fields);
- is visible in the composer and task header.

“Project or folder” is intentionally broad: a one-off folder does not require creating a project.

## Project detail

**Unknown.** The route/pattern and UI were not observed. A rebuild must design it explicitly and mark
it as an unobserved design, not claim it as verified. Minimum: edit name/instructions, manage memory,
open the folder, list sessions, and delete the project without deleting unrelated files.

## Memory

- Project memory is separate from global account memory.
- The creation form shows **Memory is on**.
- Memory content is personal; never publish it.
- Project deletion must state whether memory is deleted. The shipped behavior was not exercised; the
  rebuild must define it.

## Scheduled tasks with projects

The `coworkScheduledTaskProjects` gate is supported. Scheduled tasks can target a project context. The
selector/behavior was not exercised; the scheduler stores a project reference, resolves it at run
time, and fails clearly if the project is missing/archived.

## States

| State | Behavior |
|---|---|
| Empty | One New project button + centered explanation |
| Choice dialog | Two cards; close/back |
| Form | Name/instructions/folder; Create disabled until valid |
| Folder picker open | Native dialog; form unchanged until return |
| Created | Project appears in list; detail route opens (unobserved) |
| Duplicate/error | Inline error; input retained |
| Missing folder | Project shows unavailable; do not silently create a new folder |
| Deleted | Confirmation; tasks/projects/memory handling per policy |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-PROJ-01` | Empty state has exactly one creation entry and no fake projects. |
| `REQ-PROJ-02` | Choice dialog offers start-from-scratch and existing-folder paths. |
| `REQ-PROJ-03` | Name, instructions, and canonical folder persist; Create is disabled until valid. |
| `REQ-PROJ-04` | Memory status is visible, scoped to the project, and persisted. |
| `REQ-PROJ-05` | Project context can be selected in the composer and is recorded in the session. |
| `REQ-PROJ-06` | Missing folders/projects fail clearly; delete explains files/tasks/memory effects. |

**Tests:** `TEST-UX-004`, `TEST-SESSION-005`, `TEST-SEC-002`.
