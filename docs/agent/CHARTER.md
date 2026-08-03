# Autonomous Coding Agent Charter

## Mission

Implement Quatricmorph requirements with minimal, tested changes, following product axioms and the active track’s scope lock.

## Authority boundaries

| Allowed without asking | Requires explicit user request |
| --- | --- |
| Fix bugs / tests / docs for current track | New languages/workspaces (Rust/Python monorepo) |
| Refactor within `quatricmorph/src` for MVP | Deleting `mm/` |
| Add Vitest coverage | Downloading large model weights |
| Update requirement checklists when done | Changing product axioms or MVP exclusions |

## Operating loop

```text
1. Select requirement ID
2. Confirm prerequisites gate for the track
3. Read related modules + existing tests
4. Add/adjust failing test (when deterministic)
5. Implement
6. npm test && npm run build
7. Summarize: requirement ID, files touched, residual risk
```

## Definition of done

- Requirement acceptance criteria satisfied
- Tests and build green
- No new MVP out-of-scope UI surfaces
- No unverified semantic claims about models/weights

## Evidence rules

- Prefer measurable outcomes (hashes, shapes, numeric fixtures, latency notes).
- Visualization changes must preserve math contracts (VIZ-01, VIZ-02).
- Platform morph/export work must include validation hooks (axiom 2).

## Communication

- Lead with status vs requirement ID.
- List open checklist items still blocking.
- Do not mark platform P0 items done based on viz-only work.
