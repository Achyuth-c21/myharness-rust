# 22 — Branching and commit policy

Working model for this repository (binding for human and AI contributors):

## Branch roles

| Branch | Role | Access |
|---|---|---|
| `main` | **Frozen reviewed baseline.** Never written to by routine work — no checkouts for editing, no merges, no direct pushes. | Manual, maintainer-only |
| `dev` | Integration branch. The "copy" the team iterates on. | All contributors |
| `phase-*` / `feature-*` | Short-lived working lanes cut from `dev`. | All contributors |

## The loop (AI agents and daily work)

```text
1. git switch dev && git pull          # integrate latest
2. git switch -c phase-1 dev           # new lane from dev
3. code + commit locally on the lane
4. git switch dev && git merge --no-ff phase-1   # land the work
5. git push origin dev                 # publish dev
6. git pull                            # confirm clean state
7. repeat from (2) for the next slice
```

- Lanes are disposable; if a lane rots, cut a fresh one from `dev`.
- `main` only ever advances by an explicit maintainer decision, and `dev` may only
  reach `main` through that decision — never by agent automation.
- Sensitive evidence lives outside git (`evidence/app-asar-orig/` is ignored by design).

## Rules for automation

- Agents must never: `git push origin <branch>:main`, `git checkout main` (for edits),
  merge anything into `main`, or force-push any protected branch.
- Every lane commit must build logically (`cargo metadata` at minimum) and keep the
  roadmap traceability comments in place.
