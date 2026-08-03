# Autonomous Coding Agent Charter

## Mission

Implement Quatricmorph requirements with minimal, tested changes, following product axioms and [`ARCHITECTURE.md`](../../ARCHITECTURE.md).

## Authority boundaries

| Allowed without asking | Requires explicit user request |
| --- | --- |
| Fix bugs / tests / docs for current track | Deleting `mm/` or inventing alternate product architectures |
| Scaffold Phase 0 paths consistent with architecture §16 | Downloading large model weights |
| Add unit/integration coverage for tiles, addresses, range reads | Changing product axioms or architecture §19 non-goals |
| Update requirement checklists when done | Marking later phases done based only on Phase 0 work |

**Conflicts:** Root `ARCHITECTURE.md` overrides every other doc.

## Operating loop

```text
1. Select requirement ID (TILE-* or PLAT-*)
2. Confirm prerequisites gate for the track
3. Read ARCHITECTURE.md sections relevant to the change
4. Add/adjust failing test (when deterministic)
5. Implement
6. Run package tests / build
7. Summarize: requirement ID, files touched, residual risk
```

## Definition of done

- Requirement acceptance criteria satisfied
- Tests and build green for touched packages
- No new Phase 0 out-of-scope surfaces (cube-per-weight, full-RAM load, Cesium compute, etc.)
- No unverified semantic claims about models/weights
- UI/API results labeled exact / sampled / approximate when applicable

## Evidence rules

- Prefer measurable outcomes (hashes, shapes, byte-range metrics, numeric fixtures).
- Visualization changes must preserve addressing and scalar contracts (`TILE-06`, `TILE-07`).
- Platform morph/export work (when tasked) must include validation hooks (axiom: validation before success).

## Communication

- Lead with status vs requirement ID.
- List open checklist items still blocking.
- Do not mark Phase 1+ or morph/export complete based on Phase 0-only work.
