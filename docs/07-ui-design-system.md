# 07 — UI Design System

This document describes the visual and interaction system used by the shipped SPA. Tokens and
component measurements were read from the live renderer; personal content is omitted.

## 1. Foundations

### 1.1 Typography

| Role | Family | Observed size |
|---|---|---|
| Interface/body | `anthropic-sans`, then system UI and broad script fallbacks | 13–14 px |
| Supporting/meta | same | 12 px, secondary color |
| Sidebar | same | 13 px / 19.5 px line |
| Code/terminal | `anthropic-mono`, `ui-monospace`, `SF Mono`, Menlo, Consolas | code-specific |
| Display/greeting | serif (Anthropic display serif) | large, ~32 px |
| Chat transcript | configurable: Anthropic Serif or system | transcript size Small/Medium/Large |
| Code UI | configurable interface font | Code-specific appearance |

The sans is a variable font. Computed weights include 400, 433.25, 500, 533.25, 577.75, and 700.
Rebuilders should use a variable family and support intermediate weights rather than snapping to
only regular/bold.

The Settings → Claude Code section exposes:

- Code appearance: **Claude Light** / **Claude Dark**.
- Code font: custom monospace.
- Interface font: **Anthropic Sans** / **System** / **OpenDyslexic**.
- Transcript size: **Small** / **Medium** / **Large**.
- Transcript width: **Narrow** / **Medium** / **Wide**.
- Default transcript view: **Normal** / **Thinking** / **Verbose**.

**Verified** from the settings UI and computed styles.

### 1.2 Color tokens

Resolved core tokens in the light theme:

| Token | Light | Dark |
|---|---|---|
| Page background | `#fcfcfb` | `#151515` |
| Surface 0 | `#f9f9f7` | `#0b0b0b` |
| Surface 1 | `#fcfcfb` | `#151515` |
| Surface 2 | `#ffffff` | `#1a1a19` |
| Surface 3 | `#ffffff` | `#20201f` |
| Text primary | `#0b0b0b` | `#f0efec` |
| Text secondary | `#52514e` | `#c3c2b7` |
| Text tertiary/icon | `#898781` | — |
| Strong border | `hsl(from #0b0b0b h s l / 20%)` | `hsl(from #fff h s l / 20%)` |
| Chat surface | `#fcfcfb` with white cards | `#151515` with elevated `#1a1a19` cards |

Additional observed accents:

- Claude orange mark: approximately `#c6613f`.
- Informational blue background: `#cde2fb`; informational text: `#184f95`.
- Code dashboard accent: `#2a78d6`.
- Warning toast: warm cream background with brown text.
- Selection/active row: warm neutral fill, distinct from hover.

These accent values are **verified samples**, not a complete semantic palette. The complete
`--cds-*` scale exists in CSS but was not exhaustively enumerated. Do not invent additional tokens;
extend this table only from computed values.

### 1.3 Spacing, radius, borders

- 4 px base unit; common gaps are 4, 8, 12, 16, 24, and 32 px.
- Control radii: 5 px (small), 6 px (inputs/rows), 7 px (icon buttons), 8 px (cards/popovers),
  20 px (pills).
- The CSS radius token resolves to `calc(.5rem * 1)`, i.e. 8 px at default scale.
- Borders are hairline, low-contrast; separators are neutral, not pure black.
- Shadows are soft and used sparingly for popovers/modal; the settings scrim is dark and translucent.

**Strong inference** for the spacing scale from Tailwind utility classes and measured gaps. Radius
and border samples are **verified**.

### 1.4 Layout

| Element | Measurement |
|---|---|
| Title bar | 36 px high |
| Sidebar | ~22.2vw fluid: ~321 px at default window width, ~425 px at 1920 (2026-10-21 reference re-attest) |
| Sidebar item | 26–34 px high, 5–7 px radius |
| Composer | ~720–735 px wide on new task (fluid); radius large — 16–20 px (2026-10-21 reference reads ~16 px) |
| Main content column | ~720 px centered, with generous vertical whitespace |
| Settings modal | ~1400×960; left nav ~208 px; internal content column ~800 px |
| Code dashboard | centered overview card ~520–570 px wide; composer below |
| Task transcript | responsive max width; user/assistant blocks separated by whitespace |
| Window controls | home surface shows a single round close at the top-right corner (2026-10-21 reference); no min/max at this size |

The composer is a rounded white card with a toolbar row. The textarea is transparent. The send button
at rest is a low-contrast rounded-square outlined control with an upward arrow (2026-10-21 reference
re-attest; an earlier note read it circular); when text is present it becomes primary. Attachments,
skills, connectors, folder, and model controls share the toolbar; on the empty home the model label
renders as plain text (reads "claude" in the captured build) and the project/folder selector sits in
its own strip section beneath the input card.

### 1.5 Iconography

- Custom variable icon font (`Anthropicons-Variable`) for navigation and controls, 16–20 px.
- The Claude starburst mark uses the orange accent.
- Icons are decorative when paired with a text label and have accessible labels when icon-only.
- Status icons include check, warning triangle, clock, folder, document stack, spinner, and chevrons.

**Verified** for font and observed glyphs. Exact icon codepoints are implementation details and
should be replaced with a documented icon set in the rebuild.

## 2. Themes

### 2.1 Theme modes

- **System** — follows OS `prefers-color-scheme`; persisted as `system`.
- **Light** — forced light.
- **Dark** — forced dark.

The root uses `data-theme="claude"` and a resolved `data-mode="light|dark"`. The renderer also calls
`WindowControl.setThemeMode` so native chrome can match.

### 2.2 Dark theme

Dark mode is a warm neutral dark, not pure black:

- page `#151515`
- lowest surface `#0b0b0b`
- cards `#1a1a19`
- elevated controls `#20201f`
- primary text `#f0efec`
- secondary text `#c3c2b7`

The dark screenshot `73-settings-dark.png` shows the settings modal and underlying app in dark mode.
Code appearance can be independently set to Claude Light/Dark, so the code editor may not match the
shell theme.

### 2.3 Font and motion preferences

- **Interface font** and **Chat font** are independent.
- **Reduce motion** reduces streaming and interface animation.
- **Transcript width** and **text size** affect only the conversation column.
- **Default transcript view** controls Normal/Thinking/Verbose for new sessions; a session menu can
  override it.

These preferences are persisted locally and are not organization policy, except where a policy
locks a related surface.

## 3. Component library

### 3.1 Buttons

| Variant | Use | Visual |
|---|---|---|
| Primary | Create, Apply, Send | Filled dark (light theme) / light (dark theme), white/dark inverse text |
| Secondary | Back, Manage, Change | White surface, hairline border |
| Ghost/icon | Toolbar, close, overflow | Transparent until hover; 28×28 or 26×26; 7 px radius |
| Destructive | Delete, reset, sign out | Red text or filled red, confirmation required |
| Link | Learn more, folder path | Underlined or accent-colored text |
| Disabled | Invalid form or unavailable action | Reduced opacity; no hover |

Hover, active, focus-visible, and disabled states are visually distinct. Focus uses a ring shadow
rather than a browser outline; the ring must remain visible on both themes.

### 3.2 Inputs

- Text fields are either borderless inside a surface or 1 px bordered with 6 px radius.
- Labels sit above fields; descriptions/help text sit below in secondary text.
- Required fields use an asterisk in the setup window.
- Inline validation replaces help text and marks the field invalid.
- The composer is a multiline, auto-growing input with a maximum height and internal scrolling.
- Search fields have a leading search icon and a clear button when non-empty.

### 3.3 Menus and popovers

- Surface-2 background, 8 px radius, hairline border, soft shadow.
- Menu rows ~32–40 px high; icons and keyboard hints align right.
- Hover and selected rows use the neutral hover fill; selected state may also show a check.
- Submenus open to the right and scroll when taller than the viewport.
- Popovers are anchored to their trigger and reposition to stay on-screen.
- Escape and outside click close; focus returns to the trigger.

The model picker and composer menus are the richest examples. The account menu is a compact list.
The setup profile menu is a separate popover.

### 3.4 Dialogs and sheets

- Centered modal with dark scrim; ~1400×960 for Settings.
- Settings uses a two-pane layout: search + section nav left, scrollable content right.
- Language is a compact centered list with a check on the active locale.
- Project creation is a compact modal with field groups and a footer.
- Confirmation dialogs use a title, explanatory copy, and Cancel/Confirm actions.
- Setup window is a full window, not a modal, with tabs across the top.

### 3.5 Cards and rows

- Task rows are full-width, ~32 px high, with title truncation, relative time, status icon, and
  hover/active background.
- Project/artifact/scheduled cards use a soft surface, 8–12 px radius, icon or preview, title,
  supporting copy, and overflow.
- Settings rows use label + description on the left and control on the right, separated by generous
  vertical space; some are divided by a hairline.
- Tables (connectors) use a header row, 1 px separators, status chips, and row actions.

### 3.6 Pills and chips

- Used for mode switch, model selector, folder selector, and filters.
- Fully rounded (20 px+) with compact 13 px text.
- Selected pill uses a raised surface and darker text; unselected is transparent/secondary.
- Status chips use semantic color backgrounds and text, not color alone.

### 3.7 Toasts and banners

- Persistent error toast bottom-right (observed MCP failure), warm warning background, close button,
  and an action link.
- Inline information banners span the content width and use a neutral or accent-tinted surface.
- Update banner is pinned above the account row in the sidebar.
- Banners do not block the composer.

## 4. States and feedback

| State | Pattern |
|---|---|
| Loading | Skeleton rows or centered spinner; composer may be disabled; boot placeholder can precede hydration |
| Empty | Centered icon, short headline, one sentence, primary action; no fake data |
| Error | Inline message near the failing control, or persistent toast with action |
| Partial | Show what loaded and a non-blocking warning (“N of M loaded”); model discovery does this |
| Streaming | Send → Stop; incremental text; tool blocks show running/completed |
| Disabled | Opacity + no pointer events; tooltip explains policy if useful |
| Selected | Checkmark or filled row; not color alone |
| Unread | Small dot next to task title |
| Pinned | Pin icon/section at top of the sidebar |
| Active | Highlighted row plus accessible `aria-current`/selected state where applicable |
| Validation | Red border/text, `aria-invalid`, error summary for form-level failures |
| Permission | Explicit approval surface; never silently grants |
| Offline | Connection error banner and retry; cached content remains visible where safe |

## 5. Accessibility

Observed/expected requirements:

- Semantic roles: `dialog`, `menu`, `menuitem`, `tab`, `tabpanel`, `combobox`, `listbox`, `option`,
  `switch`, `radio`, `radiogroup`.
- Icon-only controls have `aria-label`; decorative icons are hidden from assistive technology.
- Focus is visible and trapped in modals; Escape closes the top layer.
- Search results and model lists support arrow-key navigation and Enter selection.
- Toggles expose checked state; radio groups expose the selected option.
- Text and controls meet contrast targets in both themes; semantic colors are not the sole signal.
- Reduced-motion preference is honored.
- Font choices include OpenDyslexic; transcript size/width are adjustable.
- Error messages are associated with fields and announced when they appear.

**Verified** for roles and labels seen in the DOM. **Unknown:** complete screen-reader behavior,
live-region announcements, and native menu accessibility.

## 6. Responsive behavior

- The sidebar can be collapsed (`Ctrl+B`); the main content expands.
- The layout is optimized for desktop widths; the settings modal is nearly full-width at 1920 px.
- Composer has a max width and does not stretch indefinitely.
- Long menus and model lists scroll within the viewport.
- The setup window has its own fixed/minimum size and does not use the main window layout.
- Narrow-window, high-DPI, and multi-monitor behavior were not exhaustively tested.
- The persisted zoom factor is applied at startup; zoom controls use standard Electron roles.

**Strong inference / partial verification.** Do not claim a mobile layout exists.

## 7. Content and tone

- Concise, direct labels; sentence case.
- Descriptions explain consequences and data handling (“Prompts, completions, and your data are never
  sent to Anthropic”).
- Destructive actions are explicit and include risk copy (bypass permissions, delete, reset memory).
- Error copy names the failing subsystem and offers a concrete next step.
- Empty states avoid faux data and explain how to begin.
- Model/provider labels preserve the provider’s native display name and full model id.

## 8. Design-system requirements

| ID | Requirement |
|---|---|
| `UI-DS-01` | Ship light, dark, and system themes with equivalent component coverage. |
| `UI-DS-02` | Use one variable sans, one serif display face, and one mono face with documented fallbacks. |
| `UI-DS-03` | Preserve 4 px spacing, 8 px card radius, 5–7 px control radius, and hairline borders. |
| `UI-DS-04` | Every interactive element has hover, active, focus-visible, and disabled states. |
| `UI-DS-05` | Menus and dialogs trap/restore focus and support Escape. |
| `UI-DS-06` | Status is conveyed by text/icon as well as color. |
| `UI-DS-07` | Streaming, loading, empty, error, disabled, and permission states are designed, not incidental. |
| `UI-DS-08` | Font, transcript size/width, motion, and theme preferences persist and apply before paint where possible. |
| `UI-DS-09` | The composer remains usable with the sidebar collapsed and with long model names. |
| `UI-DS-10` | No private content appears in design fixtures or screenshots. |
