# Screen — Composer Menus

**Screen:** `UI-SCREEN-25`
**Triggers:** `+` button, `/` at the start of an empty composer, **Project or folder** chip
**Features:** `FEAT-CHAT-03`–`FEAT-CHAT-07`, `FEAT-MCP-*`, `FEAT-MULTI-*`
**Semantics:** [chat-and-composer](../features/chat-and-composer.md), [mcp-skills-tools](../features/mcp-skills-tools.md)

The captured menus were opened with model-picker and other popovers already mounted. The screenshots
therefore show overlapping layers. That layering is an **automation artifact**, not evidence that the
product intentionally allows several composer popovers at once. A rebuild should close the previous
popover when another trigger opens.

## 1. Add menu (`+`)

Anchored above the `+` control, 274 px wide, ~140 px tall.

| Row | Trailing | Behaviour |
|---|---|---|
| **Add files or photos** | `Ctrl+U` | Native file picker; images become attachments |
| **Skills** | `›` | Opens Skills flyout |
| **Connectors** | `›` | Opens Connectors flyout |
| **Add plugins** | — | Navigate to Customize → Plugins / install flow |

- Rows are 40 px high with a leading glyph and 14 px text.
- The menu has no header and no destructive actions.
- Files/images are added to the current draft; they are not uploaded to Anthropic in 3P mode.
- `Ctrl+U` is the same action when the composer is focused.
- If a capability is disabled (`nativeQuickEntry`, attachments, MCP), hide the row or show a reason;
  never open a dead picker.

### Files and photos

- Multi-select; each attachment is shown as a removable chip/thumbnail in the composer.
- Image MIME/size limits are provider-dependent; error before send.
- Browser/screenshot capture is **not** in this menu in the captured build.
- Drag/drop and paste are separate attachment paths.

## 2. Skills flyout

320 px wide, scrollable, ~525 px tall in the capture. Rows have a document/skill glyph and the skill
name only; no descriptions.

Observed entries include `consolidate-memory`, `context`, `docx`, `explain-usage`, `find-skills`,
`frontend-design`, `pdf`, `pdf-reading`, and more below the fold.

Footer actions separated by a hairline:

- **Manage skills** — navigate to Customize → Skills.
- **+ Browse skills** — open the skill browser/install surface (unobserved).

Behaviour:

- Clicking a skill inserts/activates it for the draft or opens its parameter prompt depending on the
  skill. The exact insertion mechanism for parameterless skills is **unknown**.
- The menu stays searchable only by scrolling in this capture; no search field is visible.
- Disabled/missing skills are omitted; a failed catalog should show an inline retry, not an empty
  “no skills” message.
- Skill content/tools remain subject to the same permission model.

## 3. Connectors flyout

320 px wide, scrollable, ~526 px tall in the capture.

- Header action: **Manage connectors** (navigate to Customize → Connectors).
- Rows: connector icon, name, and a blue toggle at the right.
- Toggling enables/disables the connector for **this session/draft**, not globally. This is the observed
  switch placement; persistence scope must be verified in a rebuild.
- Toggling a connector that is not authenticated should start its setup rather than appear on.
- A Connectors badge/error in setup indicates a configuration problem; this menu does not explain it.

Observed connectors are environment-specific. Do not publish the list, server URLs, or tokens.

## 4. Project or folder menu

Anchored under the composer’s **Project or folder** chip; 330 px wide, ~190 px tall.

```text
┌ Search projects ──────────────┐
│ 📁 Study        /home/...     │
│ 📁 PRM          /home/...     │
│ 📁 …                          │
├───────────────────────────────┤
│ 📁 Add folder                 │
└───────────────────────────────┘
```

- Search filters by project/folder name and path.
- Each recent row shows folder icon, name, and full path (second line, muted); paths are personal data.
- **Add folder** opens the native directory picker.
- Selecting a folder sets the session working directory; selecting a project also applies project
  instructions/memory.
- No “None” row is visible; clearing context may be done by re-opening or a separate action (unknown).
- If no recents exist, show the search + **Add folder** only.
- The chip label updates to the selected name; hovering exposes the full path.

## 5. Slash menu (`/`)

Triggered when `/` is typed at the start of an empty composer. Anchored above the composer, 280 px wide,
max-height ~310 px, scrollable.

Rows:

1. **Add files** (paperclip icon; selected by default)
2. **Usage**
3. Skill/command rows with a document glyph: `consolidate-memory`, `context`, `docx`, `explain-usage`,
   `find-skills`, `frontend-design`, … below the fold.

Behaviour:

- Filtering as more characters are typed is expected; the captured state is only `/`.
- `ArrowUp/Down` moves the highlight; `Enter`/`Tab` accepts; `Escape` closes and leaves the typed text.
- Accepting **Add files** opens the file picker and removes the slash.
- Accepting a skill inserts the command token or activates the skill; the exact syntax is **unknown**.
- **Usage** opens the Usage view/report; it is not a model call.
- `/schedule` is documented in the Scheduled Tasks screen and is the supported way to create a
  schedule from a task.
- Unrecognised slash text sends literally; it must not be silently interpreted as a command.

## 6. Menu coordination and states

| State | Required behavior |
|---|---|
| One menu open | Clicking another trigger closes the first |
| Escape | Closes the topmost layer only |
| Outside click | Closes all composer popovers |
| Scroll | Menu scrolls; composer stays anchored |
| Resize | Reposition or close; never detach off-screen |
| Loading | Skeleton/inline spinner, not empty |
| Empty | Explain configuration, not “no skills” for an error |
| Disabled | Hidden or disabled with reason; no dead click |
| Keyboard | Full arrow/Enter/Escape support; focus restored |
| Mobile/narrow | Menus stay within viewport; may become sheets |

## 7. Accessibility

- Trigger buttons expose `aria-expanded` and `aria-haspopup`.
- Menus use menu/menuitem (or listbox) semantics consistently.
- Submenus open on ArrowRight/Enter and close on ArrowLeft/Escape.
- Toggle rows in Connectors expose switch semantics and checked state.
- File input has an accessible label; attachment chips have remove buttons with names.
- Paths are not the only label; accessible names include the project/folder name.
- Focus returns to the originating trigger after close.

## 8. Data and security

- Attachments are local file references until encoded for the selected provider. Provider upload
  limits and retention differ; never claim local-only if the request sends bytes.
- Connector toggles change the enabled MCP tool snapshot for the next request.
- Project/folder selection changes `cwd` and context; it must not grant read access outside allowed
  roots.
- Menu content can include user-installed skills/MCP servers; treat names/descriptions as data, not
  instructions.
- Do not publish absolute paths, connector names, or tokens from captures.

## 9. Evidence

- `55-attach-menu.png`, `56-attach-skills.png`, `57-attach-connectors.png`,
  `58-slash-menu.png`, `59-project-folder-menu.png`.
- DOM menu roles and labels captured through CDP.
- `runtime-api-surface.json` for attachment/MCP/skill APIs.

All these screenshots contain personal task titles and paths; they are in the private archive and
not included.

## 10. Test mapping

`TEST-UX-001`, `TEST-UX-013`, `TEST-MCP-001..010`, `TEST-AI-013`, `TEST-SEC-002`.

## Acceptance checks

1. Only one composer popover is open at a time.
2. Add files opens a native picker and attaches selected files exactly once.
3. Skills flyout lists enabled skills and footer actions work.
4. Connector toggles reflect and persist the intended scope; unauthenticated connectors prompt.
5. Project search + Add folder work without exposing paths in logs.
6. Slash filtering, acceptance, and Escape behave correctly.
7. Unknown slash text sends literally.
8. Keyboard/focus restoration works for every menu.
9. Disabled capabilities show truthful reasons; no empty state masks an error.
