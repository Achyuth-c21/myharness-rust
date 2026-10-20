# Screen — Customize (Skills, Connectors, Plugins)

**Screens:** `UI-SCREEN-15`, `UI-SCREEN-16`, `UI-SCREEN-17`
**Route:** `/customize` (tab state in UI; exact URL hash not confirmed)
**Features:** `FEAT-CUST-01`–`FEAT-CUST-06`

## Shell

- Serif page title **Customize**.
- Tabs: **Skills**, **Connectors**, **Plugins**.
- Right-side controls vary by tab.
- Content column uses most of the main width; rows span ~1050 px.
- No modal wrapper; this is a normal route.
- Search/filter/sort are immediate; no Apply button.

## 1. Skills tab

Observed:

- Tabs at top; **Add** black split button at far right.
- Secondary icons: search, filter, sort.
- Sections:
  - **Created by you · <count>**
  - **From Anthropic & Partners · <count>**
- Row layout (not cards):
  - document/skill icon tile;
  - name (monospace/semibold);
  - author line (`by you` or `from Anthropic`);
  - description (truncated);
  - date at right;
  - overflow `⋮`.

Observed rows include `find-skills`, `vercel-composition-patterns` under “Created by you”, and
`consolidate-memory`, `context`, `docx`, `explain-usage`, `frontend-design`, `pdf`, `pdf-reading`,
`pptx`, `schedule`, plus more under “From Anthropic & Partners”. Counts and ownership vary by
profile; do not hard-code.

Add menu (unobserved) should include create/upload and manage. `skillCreationEnabled` gates creation.
The skill detail/editor was not opened.

### Interactions

| Action | Result |
|---|---|
| Add | Create/upload/manage menu |
| Search | Filter names/descriptions |
| Filter | Filter source/ownership/status |
| Sort | Name/date/author order |
| Row | Open detail/edit (unobserved) |
| Overflow | Enable/delete/details (unobserved) |

## 2. Connectors tab

Observed:

- Tabs; search icon at far right. No Add button in this state.
- Filter chips: **All**, **Connected**, **Not connected**.
- Table header: **Connector**, **Type**, **Status**.
- Row:
  - icon (generic server or connector logo);
  - name;
  - type + tag: `Desktop` + `Local dev`, or `Web` + `Custom`;
  - status: check mark for connected, **Connect** button for not connected.
- At least one row shows **Connect** while others are connected. Names are redacted.
- A persistent MCP error toast may appear at bottom-right.

Connector names are profile-dependent. The type/tag vocabulary is the evidence; use it consistently.

### Interactions

| Action | Result |
|---|---|
| Filter chips | Narrow rows immediately |
| Search | Filter by name/type |
| Connect | Start connector auth/setup |
| Connected row | Open details/disconnect (unobserved) |
| Error toast | Open developer settings; dismiss |

OAuth, disconnect, and error-detail flows were not exercised. Do not invent a multi-step wizard.

## 3. Plugins tab

Observed empty state:

- Tabs; search; **Discover** secondary button; **Add** split button.
- A light-gray panel:
  - **Add your first plugins**
  - “Give Claude role-level expertise with plugins.”
  - black outlined plugin/puzzle illustration at right.
- No plugin rows and no fake marketplace content.

Marketplaces are configured, but the Directory/install flow was not opened.

Expected (design, unobserved):

- Installed list with enable/disable/update/remove.
- Discover/directory with marketplace filter.
- Install review with skills/MCP/permissions.
- Empty/error/update states.

## Shared states

| State | Behavior |
|---|---|
| Loading | Skeleton rows; no empty flash |
| Empty | Skills/connectors/plugins-specific empty state |
| Populated | Rows with hover/focus/overflow |
| Search no results | Keep query; clear action |
| Policy-disabled | Hide Add/upload with explanation |
| MCP error | Toast; non-blocking |
| Install/connect pending | Progress; safe cancel |
| Unsupported | Reason surfaced; no fake control |

## Accessibility

- Tabs implement a tablist with arrow-key navigation and selected state.
- Add/Discover/search/filter/sort have accessible names.
- Table has header associations; status is text, not color-only.
- Row overflow menu is keyboard-accessible and restores focus.
- The MCP toast has a close button and a link, not just color.

## Evidence

- `22-customize.png`, `23-customize-connectors.png`, `24-customize-plugins.png`.
- Bundled skill manifest.
- `runtime-api-surface.json` for skill/MCP/plugin APIs.

Personal skill names/descriptions in the Skills capture are product/user content; sanitize before
publication.

## Test mapping

`TEST-UX-007`, `TEST-MCP-001..010`, `TEST-CFG-006`.

## Acceptance checks

1. Tab state and keyboard behavior work.
2. Skills show ownership/source and counts without hard-coded data.
3. Connector filters compose and status is accurate.
4. Plugins empty state matches; Add/Discover are distinct.
5. MCP error is actionable and redacted.
6. Policy-disabled actions are hidden/explained.
7. No personal skill/connector data in fixtures.
