# 06 — Navigation and Route Map

Navigation is a hybrid of client routes (`app://localhost/...`), a settings hash, popovers, and a
separate configuration window. The renderer has its own back/forward controls; these operate on the
SPA history and should not be confused with browser chrome.

## 1. Routes observed or evidenced

| Route | Surface | Notes |
|---|---|---|
| `/new` | New task / Cowork home | Default entry; clean and populated variants |
| `/cowork/<session-id>` | Cowork task | Session id is local (`local_<uuid>`); hash may carry settings |
| `/cowork-artifact` | Artifacts library | “New artifact” creates a task and leaves this route |
| `/projects` | Projects list/empty state | Create dialog overlays the route |
| `/scheduled-task` | Scheduled tasks list | Search/filter/sort in header |
| `/epitaxy` | Code dashboard | Code mode; overview and session controls |
| `/epitaxy/<session-id>` | Code session | Strong inference from route pattern and APIs; not exercised |
| `/setup-desktop-3p` | Third-party setup | Separate window, same `app://localhost` origin |
| `/` or unknown | Redirect | Runtime entered `/new`; exact redirect rule unknown |

The settings modal is not a separate route. It uses a hash on the current route:

```text
#settings/general
#settings/data-privacy-controls
#settings/usage
#settings/claude-code
#settings/cowork
#settings/import
#settings/desktop
```

Developer settings is reached from the settings navigation or from the persistent MCP error toast and
uses the same modal. Closing settings removes the hash and returns to the underlying route without
losing the task/session state.

**Verified** for the listed routes/hashes. **Unknown:** all parameterized project/artifact routes and
redirect behavior from `/`.

## 2. Global navigation actions

| Action | Source | Destination |
|---|---|---|
| New | Sidebar “New” / `Ctrl+N` | `/new` |
| Projects | Sidebar | `/projects` |
| Artifacts | Sidebar | `/cowork-artifact` |
| Scheduled | Sidebar | `/scheduled-task` |
| Customize | Sidebar | In-app Customize view (route not captured) |
| Cowork mode | Segmented switch | Cowork task/home route |
| Code mode | Segmented switch | `/epitaxy` |
| Open task | Sidebar/active list/search | `/cowork/<session-id>` |
| Search | Search icon / `Ctrl+K` | Palette overlay |
| Settings | `Ctrl+,` / account menu | Settings modal at last/default section |
| Inference configuration | Account menu | Separate `/setup-desktop-3p` window |
| Language | Account menu | Language dialog |
| Back/forward | Title bar arrows | SPA history |
| Close window | `Ctrl+W` | Window close; on Linux quits if no tray |
| Close pane/sidebar | `CmdOrCtrl+\` / `CmdOrCtrl+B` | Layout change |

## 3. Creation flows

### New conversation/task

```text
/new → select model/project/folder/attachments → send
     → /cowork/<session-id> (streaming) → transcript/task detail
```

The task appears in the sidebar immediately and in the home “Active” list. The composer remains
available in the task view. If a task is archived/deleted, the sidebar entry is removed; the exact
post-delete destination depends on whether the deleted task is currently open (unknown).

### New project

```text
/projects → New project
          → dialog **Create a new project**: Start from scratch | Use an existing folder
          → form **Start a new project**: name, instructions, folder, Memory is on
          → Create → project (detail route unknown)
```

“Use existing folder” selects a folder path into the form. Back returns to the choice dialog; the exact
dirty-dismiss confirmation is **unknown**.

### New artifact

```text
/cowork-artifact → New artifact
                  → creates a live-artifact task immediately
                  → transcript/streaming view
```

There is no artifact-creation modal. This is a destructive navigation surprise worth preserving:
a single click starts a task and model generation.

### New scheduled task

```text
/scheduled-task → New task → editor (unknown) → save
```

Templates may prefill the editor rather than creating immediately; not exercised.

## 4. Mode switching

The Cowork/Code switch is persistent in the sidebar. It does not discard the current session; each
mode has its own home and session list. Code mode adds repository/folder selection, permission mode,
Fast mode, and the overview dashboard. Switching back does not necessarily restore the previously
open Cowork task — the last route per mode is unknown.

## 5. Modal and popover stack

Observed layers, from bottom to top:

1. Main route (`/new`, `/projects`, task, Code, etc.).
2. Settings modal (full-screen scrim; has its own left nav and search).
3. Language dialog over the task/home, or over settings.
4. Composer popovers (attachments, Skills, Connectors, slash, project/folder, model).
5. Task overflow menu; account menu; filter menu.
6. Permission/confirmation prompts (unknown).

Popovers typically close on outside click or Escape, but React state can leave them mounted in the
DOM. For CDP interaction, toggle the trigger rather than assuming Escape removed the layer.

Focus behavior is only partially verified. Expected/observed behavior:

- Opening a menu moves focus into the menu.
- Arrow keys move between items; Enter selects; Escape closes.
- The search palette traps focus and returns it to the trigger on close.
- Settings traps focus behind the scrim.
- Native file pickers suspend renderer focus until the dialog returns.

**Strong inference** for focus details; verify during rebuild.

## 6. Deep links and external entry

- The app registers a `claude://` scheme and has `disableDeepLinkRegistration` policy.
- Second-instance launches route argv/deep links to the primary instance.
- `Open File…` and `Open Folder…` are separate menu accelerators.
- Clicking a link in a task can open the built-in browser or the OS browser depending on settings.
- External artifact/file URLs use dedicated schemes (`cowork-artifact`, `cowork-file`,
  `claude-media`) and are origin-checked.

**Verified** for scheme/registration strings and menu items. Exact deep-link route grammar and
unauthenticated behavior are **unknown**.

## 7. Keyboard navigation and accelerators

Native menu accelerators extracted from the shipped main bundle:

| Accelerator | Action |
|---|---|
| `CmdOrCtrl+N` | New session/task |
| `CmdOrCtrl+,` | Settings |
| `CmdOrCtrl+K` | Search / command palette |
| `CmdOrCtrl+/` | Keyboard shortcuts |
| `CmdOrCtrl+B` | Toggle sidebar |
| `CmdOrCtrl+\` | Close pane |
| `CmdOrCtrl+;` | New side chat |
| `CmdOrCtrl+Shift+T` | Reopen closed session |
| `CmdOrCtrl+W` | Close window |
| `CmdOrCtrl+Shift+O` | Open folder |
| `Ctrl+Alt+Shift+O` | Open file |
| `Ctrl+Q` | Quit |
| `F1` | Help |
| `CmdOrCtrl+Z` / `Shift+Z` | Undo / redo |
| `CmdOrCtrl+X/C/V/A` | Cut/copy/paste/select all |
| `CmdOrCtrl+Alt+Shift+V` | Paste and match style |
| `CmdOrCtrl+F`, `CmdOrCtrl+G`, `Shift+G`, `F3`, `Shift+F3` | Find, next, previous |
| `CmdOrCtrl+0`, `CmdOrCtrl+Plus/=`, `CmdOrCtrl+-` | Actual size, zoom in, zoom out |
| `CmdOrCtrl+Shift+O` | Open folder |

Some accelerators are platform-conditional (macOS Command vs Control). The exact shortcut for Quick
Entry was observed as `Ctrl+Alt+Space`, but the Quick Entry panel is unavailable here.

## 8. Navigation state and restoration

The app persists window size, sidebar placeholder, frame layout, zoom factor, and theme. The boot
sequence can render a placeholder before the React island loads. Boot metrics in logs show a
“main_view_ready_to_show” phase distinct from “window_ready_to_show.”

Session/task route restoration is **partially verified**:

- The app restores a main window and persisted sessions.
- A task opened before restart may be restored; exact last-route restoration is unknown.
- Closing a popout cleans up its window.
- Reopening a closed session is a menu action (`CmdOrCtrl+Shift+T`).

## 9. Navigation requirements

| ID | Requirement |
|---|---|
| `ARCH-NAV-01` | Client routes must be stable, deep-linkable, and restore correctly. |
| `ARCH-NAV-02` | Settings must be a modal over the current route and must not destroy task state. |
| `ARCH-NAV-03` | The setup window must be separate and independently reachable. |
| `ARCH-NAV-04` | Back/forward must map to SPA history, not reload the app. |
| `ARCH-NAV-05` | Creation actions must make side effects explicit; “New artifact” starts generation. |
| `ARCH-NAV-06` | Menus/popovers must manage focus and be dismissible by Escape/outside click. |
| `ARCH-NAV-07` | Mode switching must preserve per-mode context without silently moving sessions. |
| `ARCH-NAV-08` | All keyboard shortcuts must be discoverable and platform-correct. |
| `ARCH-NAV-09` | Deep links must validate origin/authentication before opening local resources. |
