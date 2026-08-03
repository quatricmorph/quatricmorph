# QM-0032 — Wire the cache into the block and statistics paths

## Status

Blocked

Unblocks when `QM-0031` reaches `Complete`.

## Phase

Phase 03 — Block runtime and compute

## Objective

Make the cache actually used. Today L1 and L2 work and are tested, and **nothing
calls them**.

## Repository Evidence

* `STATUS.md` `CACHE-008` — *"The cache works; nothing calls it yet."* Not Started.
* `crates/q-cache/src/lib.rs` — `CacheKey` (`:47`) with length prefixing,
  `L1Cache` (`:110`), `L2Cache` (`:214`) content-addressed with eviction,
  `LayeredCache` (`:405`) with `HitLevel`. `CACHE-001`…`CACHE-004` Verified,
  including `l2_is_reused_after_reopen`.
* `ARCHITECTURE.md` §13.2 — the key excludes the palette, because colour is
  applied in the shader.

## Requirements Covered

`CACHE-008`, `API-012`, `MVP-17`, `AC-008`.

## Dependencies

`QM-0031`.

## Blocks

`QM-0033`, `QM-0081`.

## Parallelization

Lane A, sequential after `QM-0031`.

## Program Boundary

`crates/q-cache`, `crates/q-gpu`, `crates/q-tensor-runtime`, `crates/q-daemon`.

## Scope

* Look up before computing block statistics; store after.
* Same for decoded blocks, where the decode cost justifies it.
* `LayeredCache` wired into the daemon's state, configured by CLI flags.
* `GET /v1/cache` (stats) and `DELETE /v1/cache` (clear).
* Report `HitLevel` in job records and API responses.

## Out of Scope

L0 GPU, L3 browser, L4 remote — extension points that keep refusing ·
caching query results (`QM-0073`) · caching generated tiles (`QM-0045`).

## Files Expected to Change

* `crates/q-gpu/src/lib.rs`
* `crates/q-tensor-runtime/src/stream.rs`
* `crates/q-daemon/src/lib.rs`
* `crates/q-cli/src/main.rs`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None. `l3_and_l4_refuse_rather_than_missing_silently` **stays passing** — the
extension points must keep being honest about not existing.

## Data Contracts

```text
key = blake3( len‖source_model_hash ‖ len‖tensor_id ‖ len‖logical_slice
            ‖ len‖lod ‖ len‖statistics_algorithm ‖ len‖algorithm_version
            ‖ len‖quantization_encoding ‖ len‖visualization_encoding )
```

**Excluded deliberately:** palette, normalization range, and any purely visual
parameter the shader applies. Including them would multiply the cache by the
number of palettes for no benefit.

```jsonc
// GET /v1/cache
{ "l1": { "entries": 412, "bytes": 105906176, "hits": 8821, "misses": 1204 },
  "l2": { "entries": 9832, "bytes": 4294967296, "root": "…", "max_bytes": 8589934592 },
  "l3": { "status": "not_implemented", "requirement": "CACHE-006" },
  "l4": { "status": "not_implemented", "requirement": "CACHE-007" } }
```

## Memory and Performance Constraints

* L1 defaults: 512 entries, 256 MiB, evicting by **both** count and bytes.
* L2 default 8 GiB with eviction.
* A cache hit must be **strictly cheaper** than recomputation — measured, not
  assumed. If a lookup costs more than the compute for small blocks, the
  threshold is recorded and applied.

## Implementation Plan

1. Build `CacheKey` at the statistics call site.
2. `get_with_level` before compute; on miss, compute and `put`.
3. Same in the block decode path, behind a size threshold.
4. Wire `LayeredCache` into daemon state; add `--cache-dir`, `--cache-max-bytes`.
5. Add the two cache routes.
6. Thread `HitLevel` into job records and responses.
7. Measure hit versus miss cost; set the threshold from the measurement.

## Error Handling

* A corrupt L2 entry → **treated as a miss**, the entry deleted, recompute. Never
  served.
* An unwritable cache directory → warn once, run uncached, do not fail.
* A cache full and uneviactable → warn, run uncached.
* `DELETE /v1/cache` during an active job → refused with 409.

## Acceptance Criteria

1. A second statistics pass over the same tensor reports L1 or L2 hits and
   **skips compute** — asserted by timing and by a compute counter.
2. Restarting the daemon and re-running hits L2 (`AC-008`).
3. Changing `algorithm_version` invalidates cleanly — a miss, not a stale hit.
4. A corrupt L2 entry becomes a miss and is deleted.
5. `GET /v1/cache` reports all four tiers; L3 and L4 report their requirement IDs.
6. `DELETE /v1/cache` empties L1 and L2.
7. Hit-versus-miss cost measured; the threshold recorded.
8. `CACHE-001`…`CACHE-004` still pass.

## Verification Plan

**Automated** — hit/miss tests with a compute counter; a corruption test; a
reopen test.
**Manual** — run the pass twice, compare wall time; `curl` the cache route.

## Suggested Commands

```bash
cargo test -p q-cache -p q-gpu -p q-daemon        # verified today
curl -s localhost:PORT/v1/cache | jq               # introduced here
curl -X DELETE localhost:PORT/v1/cache
```

## Test Cases

| Input | Expected |
| --- | --- |
| Same tensor twice | Second run: hits, compute counter unchanged |
| Restart, re-run | L2 hit |
| `algorithm_version` bumped | Miss, recompute, new entry |
| L2 entry corrupted on disk | Miss; entry deleted; correct result |
| Cache dir read-only | Warning; runs uncached; no failure |
| `DELETE /v1/cache` mid-job | 409 |
| `GET /v1/cache` | Four tiers; L3/L4 name their requirements |
| Palette changed | **Same key** — no invalidation |

## Risks

| Risk | Mitigation |
| --- | --- |
| A stale hit returns wrong data | The key includes `algorithm_version` and encoding; corruption is a miss |
| Caching costs more than it saves for small blocks | Measured; a size threshold applied |
| The cache masks a correctness bug in recompute | Tests run with the cache disabled as well as enabled |

## Completion Evidence

* Timing for run 1 versus run 2, plus compute-counter values.
* `curl` output of both cache routes.
* The corruption-recovery test output.
* The measured hit/miss cost and the chosen threshold.
