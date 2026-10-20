# Screen — Account Menu and Language

**Screens:** `UI-SCREEN-19`, `UI-SCREEN-20`
**Trigger:** account row at bottom of sidebar
**Features:** `FEAT-SHELL-06`, `FEAT-SHELL-07`, `FEAT-SET-10`

## Account menu

Anchored above/beside the account row at the bottom left; ~300 px wide, white surface, rounded,
subtle shadow.

Observed header:

```text
<display name>
<provider label>          (e.g. “Gateway”)
```

The display name/provider are local/deployment identity, not an Anthropic account. Do not publish the
captured values.

Items:

| Item | Shortcut | Behavior |
|---|---|---|
| Settings | `Ctrl+,` | Open Settings modal |
| Language | — | Open language dialog |
| Inference configuration | — | Focus/open `setup-desktop-3p` window |
| — separator — | | |
| View changelog | `›` | Open changelog (submenu/window; unobserved) |
| Learn more | — | Open product/help link |
| Sign out | — | Destructive; not exercised |

Notes:

- “Inference configuration” is the bridge between the user app and the admin/provider control plane.
  It must open the separate setup window, not a Settings tab.
- Sign out scope is **unknown**. It must clear the documented credential scope and not pretend to
  sign out while leaving tokens active.
- The menu closes on outside click/Escape; focus returns to the account row.

## Language dialog

Centered modal over the current route, ~780×260 px:

- Title **Choose your language**; close X.
- Three-column grid of language options.
- Each option shows the autonym (e.g. `Français`) and the English label (`French (France)`).
- Active option has a tinted rounded background and a checkmark at the right.
- No Apply button; selection applies immediately (inference from checked state) and may require a
  restart for full localization. Exact restart behavior is **unknown**.

Observed languages (11):

1. English (United States)
2. Français (France)
3. Deutsch (Deutschland)
4. हिन्दी (भारत) — Hindi (India)
5. Indonesia (Indonesia)
6. Italiano (Italia)
7. 日本語 (日本)
8. 한국어 (대한민국)
9. Português (Brasil)
10. Español (Latinoamérica)
11. Español (España)

The locale catalog also contains other product locales; this dialog’s list is the authority for
what is offered here.

## States

| State | Behavior |
|---|---|
| Menu closed | Only account row visible |
| Menu open | Header + items; focus moves into menu |
| Disabled item | Not shown/disabled with reason (e.g. sign-in disabled) |
| Language open | Modal + scrim; active locale checked |
| Locale applying | Busy/disabled options; no duplicate requests |
| Locale error | Error; previous locale remains |
| Locale applied | UI updates; restart prompt if needed |
| Sign-out confirm | Explicit confirmation; unobserved |

## Accessibility

- Account row is a button with expanded state.
- Menu uses menu/menuitem semantics; arrow keys navigate; Escape closes.
- Header is non-interactive text.
- Language dialog is a labelled dialog; options are radio-like with selected state.
- Autonyms have correct `lang` attributes for screen readers.
- Focus restores to the trigger after close.

## Data

- Locale is persisted in app config; native locale may be updated separately.
- Language changes do not change provider/model or credentials.
- Account display name/provider come from local config; no account secret is in this menu.

## Evidence

- `25-account-menu.png`.
- `34-language.png`.
- i18n catalog `i18n/en-US.json`.

Account name/provider in the screenshot are personal and must be masked.

## Test mapping

`TEST-UX-010`, `TEST-I18N-001..004`, `TEST-AUTH-005`.

## Acceptance checks

1. Menu opens/closes with keyboard and restores focus.
2. Settings shortcut works.
3. Inference configuration opens the setup window.
4. Language selection updates the UI and persists.
5. Restart requirement, if any, is explained.
6. Sign out requires confirmation and clears the documented scope.
7. No account identity/secret is published.
