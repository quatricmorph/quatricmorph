# Testing Strategy

**Authority:** [`../ARCHITECTURE.md`](../ARCHITECTURE.md). Visualization and query results must distinguish exact, sampled, and approximate paths.

## Goals

1. Protect deterministic addressing, range reads, and scalar equality vs SafeTensors reference.
2. Keep active package builds green (Rust crates and/or Cesium viewer).
3. Defer GPU/visual regression until fixtures and headless strategy exist.
4. Never download large model weights in default CI without an allowlisted fixture policy.

## Phase 0 focus

| Layer | Tool | What |
| --- | --- | --- |
| Unit | Rust / TypeScript tests | Header parse, byte ranges, canonical addresses, LOD metadata |
| Reference | Python SafeTensors | Exact scalar / slice equality for clicked indices |
| Build | `cargo` / viewer bundler | Module graph and type checks |
| Manual smoke | CesiumJS | Zoom LOD; click cell → address + value; zoom-out does not fetch exact bytes |
| Cache | Integration | Reopen session hits content-addressed cache |

## Later phases

| Layer | What |
| --- | --- |
| WeightQL | Planner shape checks; reject mismatches before execute |
| Expression | Plan cost estimates; block-mode matmul fixtures |
| Platform | Sharded ingest, NSIR invariants, catalog determinism |

## Conventions

- Name tests after requirement IDs when applicable (`TILE-07 exact scalar matches reference`).
- Prefer pure functions and fixture bytes; avoid requiring GPU in unit tests.
- No network in default unit tests.
- Legacy `quatricmorph/` Vitest suites may remain for historical code but do not define product architecture acceptance.

## Commands (evolve with repo layout)

```bash
# When Rust workspace exists:
cargo test

# Cesium / web viewer (path TBD: apps/web or successor):
npm test
npm run build

# Legacy tree only (not architecture target):
cd quatricmorph && npm test && npm run build
```
