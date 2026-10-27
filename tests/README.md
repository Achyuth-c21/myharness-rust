# Test layout

Layers and tooling follow [docs/13-implementation-roadmap.md](../docs/13-implementation-roadmap.md) section 6.1.

| Directory   | Scope                                                                 |
|-------------|-----------------------------------------------------------------------|
| `unit/`     | schemas, policy, token/usage math, redaction, path validation          |
| `integration/` | config apply, DB migrations, MCP lifecycle, retention (temp profiles) |
| `e2e/`      | routes, composer, setup, settings, menus, permissions                  |
| `wire/`     | native provider request/response conformance against mock servers      |
| `fixtures/` | synthetic model/MCP/session/catalog/UI/security fixtures               |

`e2e/user-journey.spec.mjs` is the whole-app campaign's phase-5 suite (`docs/17` §4.24):
multi-step user journeys — first message, follow-up, rename/archive/delete, drafts,
projects, scheduled tasks, artifacts, Code, usage, palette — driven against a STATEFUL
fake bridge whose sessions/turns/projects/tasks mutate per command like the real core,
so a journey's later steps see its earlier steps' effects. Run: `node
tests/e2e/user-journey.spec.mjs`.

`e2e/a11y.spec.mjs` is the axe-core WCAG 2.0/2.1/2.2 A+AA scan (`npm run a11y`). It is green
as of `IMP-39` option (c) — tertiary text uses computed AA derivatives while icons keep the
observed `#898781` — and fails on any new serious/critical finding. It is not part of the
default `npm run e2e` gate. See `docs/17` §4.20 and `docs/07` §1.2.

Fixture policy (roadmap section 7): no real credentials, no content from the inspected user's
sessions, no provider secrets. Every fixture must be safe to publish in an open repository.
