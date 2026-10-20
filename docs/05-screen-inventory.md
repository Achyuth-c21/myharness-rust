# 05 — Screen Inventory

The main renderer is a single-page app. “Screens” are client routes plus modal/popover surfaces.
Screenshots were taken at 1920×1080, X11, 1× scale. Geometry below is measured from the captured
viewport and is descriptive, not a hard specification. Capture identifiers refer to the private
archive outside this repository; no raw screenshot is included here.

## 1. Route and surface map

| ID | Surface | Route / trigger | Status | Evidence | Detail spec |
|---|---|---|---|---|---|
| UI-SCREEN-01 | Main shell | any route | Available | screenshots 50/60/61/72 | [06](06-navigation-map.md) |
| UI-SCREEN-02 | New task / Cowork home | `/new` | Available | `50-new-task`, `54-new-clean`, `60-new-home` | [screens/new-task.md](screens/new-task.md) |
| UI-SCREEN-03 | Cowork task detail | `/cowork/<session-id>` | Available | `65-task` | [screens/task-detail.md](screens/task-detail.md) |
| UI-SCREEN-04 | Code dashboard | `/epitaxy` | Available | `61-code` | [screens/code-dashboard.md](screens/code-dashboard.md) |
| UI-SCREEN-05 | Code session | `/epitaxy/<session-id>` | Unknown | route pattern and session APIs | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-06 | Projects empty state | `/projects` | Available | `10`, `66` | [screens/projects.md](screens/projects.md) |
| UI-SCREEN-07 | New project dialog | “Create new project” | Available | `11`, `67` | [screens/projects.md](screens/projects.md) |
| UI-SCREEN-08 | New project form | “Start from scratch” | Available | `12`, `68` | [screens/projects.md](screens/projects.md) |
| UI-SCREEN-09 | Existing project detail | `/projects/<id>` (strong inference) | Unknown | not created | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-10 | Artifacts library | `/cowork-artifact` | Available | `20`, `69` | [screens/artifacts.md](screens/artifacts.md) |
| UI-SCREEN-11 | New live artifact task | “New artifact” | Available | `70` | [screens/new-artifact.md](screens/new-artifact.md) |
| UI-SCREEN-12 | Artifact detail/viewer | artifact selection | Unknown / pane unavailable | `artifactsPane` gate | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-13 | Scheduled tasks | `/scheduled-task` | Available | `21` | [screens/scheduled-tasks.md](screens/scheduled-tasks.md) |
| UI-SCREEN-14 | Scheduled task editor | “New task” | Unknown | not exercised | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-15 | Customize — Skills | sidebar “Customize” | Available | `22` | [screens/customize.md](screens/customize.md) |
| UI-SCREEN-16 | Customize — Connectors | Customize tab | Available | `23` | [screens/customize.md](screens/customize.md) |
| UI-SCREEN-17 | Customize — Plugins | Customize tab | Available | `24` | [screens/customize.md](screens/customize.md) |
| UI-SCREEN-18 | Settings modal | `#settings/<section>` | Available | `26`–`33`, `71`, `72`, `73` | [screens/settings.md](screens/settings.md) |
| UI-SCREEN-19 | Account menu | sidebar account | Available | `25` | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) |
| UI-SCREEN-20 | Language dialog | Account → Language | Available | `34` | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) |
| UI-SCREEN-21 | 3P setup window | `/setup-desktop-3p` | Available | `35`–`43` | [screens/setup-desktop-3p.md](screens/setup-desktop-3p.md) |
| UI-SCREEN-22 | Search palette | `Ctrl+K` / search icon | Available | `63` | [screens/search-palette.md](screens/search-palette.md) |
| UI-SCREEN-23 | Filter menu | task-list filter icon | Available | `64` | [screens/search-palette.md](screens/search-palette.md) |
| UI-SCREEN-24 | Model picker | composer model label | Available | `51`, `52` | [screens/model-picker.md](screens/model-picker.md) |
| UI-SCREEN-25 | Composer menus | `+`, Skills, Connectors, `/`, folder | Available | `55`–`59` | [screens/composer-menus.md](screens/composer-menus.md) |
| UI-SCREEN-26 | Permission prompt | tool call requiring consent | Unknown | APIs/events only | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-27 | Update prompt/banner | update available | Partial | banner only | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-28 | Error overlay / recovery | renderer load failure | Unknown | source only | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-29 | Trusted folders / allowed sites manager | Settings → Cowork → Manage | Unknown | buttons seen, dialogs not opened | [14](14-gaps-and-unknowns.md) |
| UI-SCREEN-30 | Skill/plugin/connector detail | row actions | Unknown | not opened | [14](14-gaps-and-unknowns.md) |

## 2. Main shell geometry

| Region | Observation |
|---|---|
| Viewport | 1920×1080 |
| Title bar | ~36 px tall; sidebar toggle, panel toggle, back/forward controls |
| Sidebar | ~321 px wide at default; scrolls independently |
| Main content | remaining width; centered content column |
| Composer | ~720 px wide on new task; rounded card with toolbar row |
| Settings modal | ~1400×960, centered, dark scrim; left nav ~208 px |
| Setup window | separate 900×720 window (content area; see screenshots) |
| Footer | disclaimer at bottom of main content; account/update pinned to sidebar bottom |

These values are **verified measurements at this viewport**. The app supports resizing and split/popout
layouts; those variants were not fully exercised.

## 3. Standard states

A screen specification must account for these states where applicable:

| State | Observed treatment |
|---|---|
| **Loading** | Skeleton/placeholder or spinner; boot placeholder can be rendered before React hydration |
| **Empty** | Centered icon, headline, explanatory copy, primary action; e.g. Projects, Artifacts, Scheduled |
| **Populated** | List/table/cards with hover, selected, active, unread, and pinned variants |
| **Error** | Inline banner or persistent toast; MCP startup error includes a **Developer settings** action |
| **Policy-disabled** | Explanatory copy instead of control; Import & export is the clearest example |
| **Unavailable** | Feature hidden or control disabled with runtime reason; e.g. tray on Linux |
| **Validation** | Inline field error; setup window uses required markers and apply-time validation |
| **Streaming** | Send becomes Stop; response text appears incrementally; transcript announces tools |
| **Permission-required** | Approval dialog/inline prompt; unknown exact layout here |
| **Offline/unreachable** | Provider/gateway error banner, retry/recheck controls |
| **Update available** | Bottom sidebar banner with version/action; not applied |

## 4. Screen-by-screen summary

### 4.1 New task / Cowork home

- Header: mode switch (Cowork selected), new task, navigation.
- Sidebar: task history grouped by recency, search/filter, active tasks.
- Main: greeting with starburst mark, composer, project/folder selector, privacy/info affordance.
- Below composer: active tasks with relative timestamps; “Clear active”.
- Empty history: no task list, but the same composer and active section rules.
- Sending creates a Cowork session and navigates to `/cowork/<session-id>`.

### 4.2 Task detail

- Header: task title dropdown, globe/context action, right-pane toggle. Location/model breadcrumb was
  not visible in the captured task header.
- Transcript: right-aligned user bubble; left assistant text; “Thought process”; tool/file blocks;
  “Working on it…” status.
- Composer remains at the bottom; stop/streaming states apply.
- Overflow/session actions live in the title dropdown and sidebar row menu; copy/open variants were not
  all exercised.
- Permission requests can appear inline or modal; exact rendering not captured.

### 4.3 Code dashboard

- Sidebar switches to code-specific repositories/sessions.
- Overview card: Sessions, Messages, Total tokens, Active days, Peak hour, Favorite model.
- Time range: All / 30 days / 7 days; tabs Overview/Models.
- Activity heatmap and comparison copy.
- Composer with local folder path, permission mode, model, Fast mode, send.
- “What’s up next?” greeting; project picker; repository controls.

### 4.4 Projects

- Empty state: folder/hand illustration, **“Looking to start a project?”**, “Give Claude a folder you
  already work from”, and a single outlined **New project** button. The header also has **New project**.
- Choice dialog (**Create a new project**): **Start from scratch** and **Use an existing folder** cards.
- Form (**Start a new project**): Name, Instructions, Choose project location (`Select a folder...`),
  **Memory is on**, Back/Create. There is no location radio group and no “What are you working on?”
  label.
- Create is disabled until the name/folder are valid (observed disabled state).
- Existing project detail unknown.

### 4.5 Artifacts

- Empty state: document/folder illustration, **“Create your first artifact”**, and a single
  **What needs my attention** suggestion button. The header has **New artifact**.
- The header also exposes search and a view toggle.
- New artifact immediately creates a task and starts generation; there is no creation modal.
- Library cards, previews, share/star/hide menus exist in the API; exact populated layout was not
  reached.

### 4.6 Scheduled tasks

- Header: title, search, sort (“Next run”), “New task”.
- Information banner: tasks run only while the computer is awake and online; keep-awake switch.
- Empty state: clock icon and “No scheduled tasks yet.”
- Template cards: Daily briefing, Inbox triage, Meeting prep, Weekly review, Content ideas, Monitor a
  topic, each with schedule and description.
- Task row states and editor unknown.

### 4.7 Customize

- Skills: search/filter/sort, Add, “Created by you” and “From Anthropic & Partners” sections. Rows (not
  cards) show icon, name, author/source, description, date, overflow.
- Connectors: All/Connected/Not connected filter; table with Connector, Type, Status; Connect action.
- Plugins: empty state (“Add your first plugins”), Add/Discover; configured marketplaces populate the
  Directory.

### 4.8 Settings

A single modal with left navigation and search. Observed sections: General, Privacy, Usage, Claude
Code, Cowork, Import & export, System, Developer, plus Customize shortcuts. Each is documented in
[screens/settings.md](screens/settings.md); configuration semantics are in
[09-settings-and-configuration.md](09-settings-and-configuration.md).

### 4.9 Third-party setup

A separate window with profile selector, Export menu, nine tabs, and Apply Changes. It is the
authoritative provider/control-plane editor. All nine tabs and six provider forms were captured. No
changes were applied.

### 4.10 Search and filter

Search is a centered palette (~726×470) with **Search chats and projects**, result rows showing
surface icon, title, and relative time. Action rows, snippets, and footer shortcuts were **not**
observed. Filter exposes Type, Status, Last activity, Group by, and Sort by.

### 4.11 Model picker

A popover anchored at the composer model label; 416 px wide and viewport-scrolled. The flat list shows
provider-qualified model names, current checkmark, full-ID tooltip, and a **More models** flyout. A
“needs API key” label is rendered for providers without credentials. The captured patched deployment
shows 522 rows/432 flyout rows with no search or grouping; that is current-state evidence, not shipped
behavior. Searchable/grouped presentation is a documented improvement.

## 5. Unobserved surfaces

The following are known or strongly inferred but not specified visually:

- Existing project detail and file manager.
- Scheduled-task editor, run history, and failure state.
- Artifact viewer/editor, version history, share dialog.
- Permission approval modal.
- Trusted-folder manager, allowed-sites manager, memory manager.
- Skill editor, connector detail, plugin detail/install confirmation.
- Device-code/OIDC sign-in windows.
- About window, changelog window, support-bundle flow.
- Update download/install/restart dialogs.
- Quick Entry panel, tray menu, popouts, split panes.
- Computer-use consent/preview, simulator, WSL/self-hosted/cloud session surfaces.
- macOS/Windows variants.

These are tracked in [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md), not invented here.
