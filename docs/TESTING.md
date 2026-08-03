# Testing Strategy

## Goals

1. Protect deterministic math and param serialization (fast unit tests).
2. Keep Vite production build green.
3. Defer GPU/WebGL visual regression until fixtures and headless strategy exist.
4. For platform work later: fixture checkpoints, checksums, and golden WeightQL (see architecture §26).

## Commands (`quatricmorph/`)

```bash
cd quatricmorph
npm test           # vitest run
npm run test:watch # vitest
npm run build      # tsc && vite build
```

## Layers

| Layer | Tool | What |
| --- | --- | --- |
| Unit | Vitest | `Array2D`, expr helpers, URL compress/round-trip, pure layout math |
| Build | `tsc` + Vite | Module graph, TypeScript surface |
| Manual smoke | Browser | Orbit, hover, URL restore (checklist in VIZ_MVP) |
| Future platform | Rust/Python test harnesses | SafeTensors fixtures, NSIR invariants, export hashes |

## Conventions

- Colocate tests under `src/**/__tests__/*.test.ts` or `*.test.ts` next to modules.
- Prefer pure functions under test; avoid constructing full Three.js scenes in unit tests.
- Name tests after requirement IDs when applicable: `VIZ-02 multiply deterministic`.
- No network in default unit tests.

## Seed coverage (initial)

- `src/viz/__tests__/array2d.test.ts`
- `src/viz/__tests__/expr.test.ts`

Expand before large refactors of `Mat` / `MatMul`.
