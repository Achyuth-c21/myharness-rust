# Screen — Model Picker and Model Browser

**Screen:** `UI-SCREEN-24`
**Trigger:** composer model label (`data-testid="model-selector-dropdown"`), or the model row in Settings/Code
**Features:** `FEAT-MOD-01`–`FEAT-MOD-08`
**Semantics:** [model-selection](../features/model-selection.md), [12](../12-ai-architecture.md)

## Purpose

Select the exact provider/model used by the next turn. The picker must expose provider and native model
identity, never substitute a different model silently, and route missing credentials to explicit setup.

## Popover

Observed on `/new` at 1920×1080:

- Anchored above the composer model label; 416 px wide.
- Height is viewport-constrained (~519 px in the capture); it scrolls.
- White surface, rounded ~12 px, thin border/shadow.
- No title, search field, provider headers, filter chips, or grouping in this popover.
- Rows are flat, sorted by display label in the captured catalog.
- A bottom **More models** row with a right chevron opens a second flyout.
- The current model is checked with a blue checkmark at the row’s right.
- A small info glyph appears next to truncatable names; hovering/focusing the row reveals the full
  provider/model id in a tooltip.
- Row height ~50 px for two-line rows; single-line “needs API key” rows are denser.

```text
┌ model popover 416 px ─────────────────────┐
│ MiniMax: MiniMax M3.1 Flash (free) (…  ⓘ ✓│
│ OpenRouter · minimax/minimax-m3.1-flash:free│
│ …                                          │
│ 01-ai/yi-large                          ⓘ │
│ NVIDIA NIM · 01-ai/yi-large · needs API key│
│ …                                          │
│ More models                             › │
└────────────────────────────────────────────┘
                                      ┌ More models 320 px ┐
                                      │ … 432 rows …       │
                                      │ scroll             │
                                      └────────────────────┘
```

The provider names and model IDs in the captured deployment (`OpenRouter`, `NVIDIA NIM`, hundreds of
non-Anthropic IDs) come from the local Gateway catalog and the live patch. They are **current-state
evidence, not the shipped provider list** and not a recommendation. The pristine build accepts only
Anthropic-policy model IDs; see [12](../12-ai-architecture.md).

## Row anatomy

| Part | Behavior |
|---|---|
| Display label | Provider-prefixed human name; middle-truncated with ellipsis |
| Info glyph | Tooltip with complete native id/provider |
| Secondary line | `Provider · native-model-id` |
| Missing credential | Appends `needs API key`; selection requires setup |
| Current model | Blue check; `aria-checked="true"` |
| Hover/focus | Neutral highlight; keyboard highlight is distinct |
| Disabled/unavailable | Not observed as `aria-disabled`; do not infer |

The flat catalog in this capture exposed 522 menu items and 432 items in **More models**. This volume is
not a usable shipped design. A production picker needs search and grouping or a curated primary list;
those are proposed improvements, not observed behavior.

## More models flyout

- Opens to the right of **More models**; 320 px wide, ~493 px tall, independently scrollable.
- Contains the full alias list (432 rows in this capture), flat and ungrouped.
- No search field and no filter in this surface, despite the i18n catalog containing the strings
  “Search models” / “Search models…”. That copy belongs to a model-management surface that was not
  reached.
- Keyboard focus can move into the flyout; Escape closes the outermost layer first (expected).
- The flyout can extend above the viewport when opened near the top; the capture starts at `y=0`.

## Selection behavior

- Choosing a row updates the composer label and the next request’s model id.
- It must not rewrite the current session’s model or prior turns.
- If the row needs credentials, show an explicit credential action; do not fall back to another model.
- If the provider is unavailable, keep the row visible with an actionable error.
- The chosen native id must be sent unchanged; no display-name-to-model guessing.
- Mid-session change is permitted only with a visible model change marker.

Selection of a non-default/credential-less row was not exercised in this investigation. That flow is a
**gap**, not a verified success path.

## Credentials and policy

- The printed catalog includes many credential-less NIM rows. A rebuild should disable rows whose
  provider has no credential, with the reason in text.
- Shipped policy rejects vendor names such as `minimax`, `gpt`, `gemini`, `deepseek` and accepts only
  `sonnet|opus|haiku|fable|mythos` or IDs containing `claude`/`anthropic` (see [12](../12-ai-architecture.md)).
  The live patch’s unconditional policy bypass is noncompliant and must not be reproduced.
- Bedrock/Vertex/Foundry have stricter ID validation. The picker shows discovered ids but the host
  validates before sending.

## States

| State | Behavior |
|---|---|
| Closed | Composer shows selected label |
| Loading catalog | Spinner/skeleton; keep current model visible |
| Ready | Flat list; current checked |
| Catalog error | Inline error + retry; no empty fake list |
| No models | Explain configuration; link to setup |
| Missing key | Row label; explicit setup path |
| Unavailable provider | Disabled with reason |
| Very long list | Scroll/virtualize; no layout shift |
| Keyboard | Arrow/Home/End/Enter/Escape |
| Selection pending | Keep popover/label stable; no optimistic wrong id |
| Session streaming | Model changes disabled or applied next turn with warning |

## Accessibility

- Trigger is a button with `aria-haspopup="menu"` and current model in its name.
- Popover uses `role="menu"`; rows are `menuitemradio` with `aria-checked`.
- `aria-activedescendant`/focus follows the highlighted row and scrolls it into view.
- Tooltips are available by focus, not hover only.
- Escape closes and restores focus to the trigger.
- The list has an accessible name (e.g., “Model”); submenu has one too.
- Provider/model IDs are read as text, not split into ambiguous fragments.

## Data and APIs

- Catalog source is provider discovery (`GET /v1/models?limit=1000`, up to 3 pages) or a signed static
  catalog; see [12](../12-ai-architecture.md).
- Selection persists in session/composer state; it is not a global default rewrite.
- Credential state comes from the provider config, never from the display label.
- Effort/1M/Fast capabilities attach to the selected model and are shown in the composer menus.

## Evidence

- `51-model-picker.png`, `52-model-browser.png`, `53-new-after-model-browser.png`.
- Live DOM: `role=menu`, `menuitemradio`, `data-model-id`, `aria-checked`; 522/432 observed rows.
- Catalog/validation rules in [12](../12-ai-architecture.md).
- Do not publish the model catalog entries as product recommendations or expose any gateway key.

## Test mapping

`TEST-AI-001..012`, `TEST-UX-002`, `TEST-UX-013`, `TEST-SEC-003`.

## Acceptance checks

1. Current model is checked and reflected in the composer.
2. Choosing a model sends its exact native id.
3. Missing credentials produce an explicit setup path, never substitution.
4. Blocked vendor ids are rejected before request dispatch.
5. No silent model change occurs between turns or on retry.
6. Long catalogs scroll and remain keyboard-navigable.
7. Search/grouping, if added, is a documented improvement and does not alter provider identity.
8. Screenshots contain no keys, gateway URLs, or personal profile names.
