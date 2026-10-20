# Model Selection

**Features:** `FEAT-MOD-*`
**Primary docs:** [12 — AI Architecture](../12-ai-architecture.md), [screens/model-picker.md](../screens/model-picker.md)

## Purpose

The model picker chooses the exact provider/model used for a turn. It also exposes effort, thinking,
1M context, Fast mode (Code), and credential readiness. It is one of the most important trust
surfaces: a wrong or silent selection changes cost, privacy, and output.

## Layout

A popover anchored above the composer model label:

- Current model row with checkmark.
- Provider-qualified model names.
- Info glyph with the full provider/native-id tooltip; capability details were not observed.
- **More models** flyout/browser when the list is long.
- No search field in the captured picker or flyout. Searchable/grouped presentation is a proposed
  improvement ([19](../19-improvement-register.md)), not observed behavior.
- Row label showing “needs API key” for unavailable providers.
- Effort/1M/thinking controls in the composer or model details, depending on surface.

The observed browser can exceed the viewport and scroll; keep the selected row visible.

## Model identity

A selection is not just a string. Persist:

```text
providerId + nativeModelId + variant (1m/fast) + profile/config revision
```

Display at least provider + model name; show the native id in details. Do not treat a catalog display
name as a unique key.

## Sources

1. Explicit `inferenceModels` (ordered; first is default).
2. Gateway/provider-native discovery.
3. Signed published catalog (names/descriptions/capabilities).
4. Bundled fallback catalog.

Merge without losing provider labels. The same model id from two providers is two entries.

## Behavior

| Action | Result |
|---|---|
| Click model label | Open picker |
| Select model | Update selection; close; persist only if allowed |
| Select “needs API key” | Prompt configure or block; never choose another |
| Open More models | Flyout/browser with scroll; no search in the captured build |
| Effort | Choose supported level; default from profile/provider |
| 1M | Choose explicit 1M variant if supported; do not silently upgrade |
| Thinking | Enable/disable if supported; preserve reasoning in transcript |
| Fast mode | Code-only capability; does not change model |
| Escape/outside | Close and restore focus |
| Discovery failed | Show error/retry; keep current selection if still valid |

## Effort

Source supports five levels: `low`, `medium`, `high`, `xhigh`, `max`. Shipped profile default was
`medium`.

Rules:

- Show only levels the model advertises; unknown → unsupported.
- If configured effort is unsupported, fail/warn visibly; do not clamp silently.
- `alwaysStartWithDefaultModel` controls whether per-user model/effort changes persist as defaults.
- A per-session effort override must be recorded with the session.

## Thinking / reasoning

- Extended thinking is model/provider-specific.
- Preserve thinking/redacted-thinking blocks and signatures; never drop them in a translation shim.
- Transcript can show Normal/Thinking/Verbose views.
- A visible “Thinking” indicator is not a substitute for preserving native reasoning content.
- Do not expose private chain-of-thought that the provider marks as hidden/redacted.

## 1M context

- `supports_1m`, `prefer_1m`, and `[1m]` identity are explicit.
- “Use 1M wherever available” can collapse base+variant into one option that requests 1M.
- Without the preference, a 1M variant is a distinct choice.
- Context-limit validation uses the variant’s actual limit.
- Do not claim 1M support from a model name alone.

## “Needs API key” and errors

The observed current profile shows many NVIDIA NIM entries as “needs API key.” This is a valid
provider-credential state:

- Row is visible only if the provider is configured/discoverable.
- Selecting it prompts configuration; it does not send.
- The model remains labeled with its provider.
- A provider with no credentials must not be silently removed from an existing session; mark the
  session model unavailable.

Provider errors (auth, quota, region, model-not-found, context) retain their native meaning.

## Current state versus shipped behavior

The live install is patched:

- It bypasses the shipped Anthropic-only model policy (`So → true`).
- Its model list includes non-Anthropic provider-prefixed IDs.
- Its default is a MiniMax free model through a local translation adapter.

Those facts are evidence for the current environment, **not** for the shipped product. The rebuild
may keep MiniMax M3.1 Flash (free) as a preferred default only through a native MiniMax provider
adapter with its own model identity and credentials.

## No silent substitution

- Never fall back to a different model/provider.
- Never “repair” a missing model by choosing the first available.
- Never hide a model change in an alias.
- Resolve aliases explicitly and show the resolved target.
- A model change is visible in the transcript/turn metadata.
- If a profile changes and a session’s model disappears, block send and ask.

## Requirements

| ID | Requirement |
|---|---|
| `REQ-MODEL-01` | Picker displays provider + model + variant and preserves focus/keyboard behavior. |
| `REQ-MODEL-02` | Model identity includes provider/model/variant/profile revision. |
| `REQ-MODEL-03` | Missing credential/model blocks send; no substitution. |
| `REQ-MODEL-04` | Effort/thinking/1M/fast capabilities are detected, not assumed. |
| `REQ-MODEL-05` | Discovery/catalog failures degrade visibly with retry/fallback metadata. |
| `REQ-MODEL-06` | Thinking/caching/tool support is preserved end-to-end. |
| `REQ-MODEL-07` | Current patched model list is not documented as shipped behavior. |

**Tests:** `TEST-UX-002`, `TEST-AI-007..010`, `TEST-AI-017..018`, `TEST-CFG-011/012`.
