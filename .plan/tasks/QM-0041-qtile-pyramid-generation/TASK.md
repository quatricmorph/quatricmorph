# QM-0041 — `.qtile` pyramid generation

## Status

Blocked

Unblocks when `QM-0040` reaches `Complete`.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Turn a pyramid plan into `.qtile` files on disk — the first artifact
Quatricmorph has ever produced.

## Repository Evidence

* `STATUS.md` `TILE-004` — *"Tile pyramid generation (building `.qtile` files for
  a model)"*, **Not Started**; the daemon returns 501.
* `crates/q-tiles/src/lib.rs` — the format is complete: `QTileHeader::for_block`,
  `QTile::from_f32`, `from_f32_quantized`, `encode`, `decode`, `to_f32`.
  `TILE-005`…`TILE-008` Verified.
* `BlockEncoding::{RawF32, QuantizedI16, MortonSparseI16}` with
  `bytes_per_cell()` 4 / 2 / 8.
* `QM-0030`'s `BlockStream`; `QM-0031`'s statistics; `QM-0032`'s cache.

## Requirements Covered

`TILE-004`, `MVP-13`.

## Dependencies

`QM-0040`, `QM-0033`, `QM-0032`.

## Blocks

`QM-0042`, `QM-0044`, `QM-0045`.

## Parallelization

Lane A, sequential after `QM-0040`.

## Program Boundary

`crates/q-tiles` (new `pyramid.rs`), `crates/q-daemon` (route + job phase).

## Scope

* Generate LOD 4 block tiles from streamed blocks.
* Generate LOD 3 tensor tiles: histogram, norms, and a coarse downsample.
* Generate LOD 2/1/0 aggregate tiles by **merging children's statistics**, not by
  re-reading.
* Encoding per LOD: `QuantizedI16` at 3–4, `MortonSparseI16` where measured
  density justifies it, `RawF32` only at LOD 5 on demand.
* Serve `GET /v1/visualizations/{modelId}/tiles/{tileId}.qtile`.

## Out of Scope

GLB (`QM-0042`) · `tileset.json` (`QM-0044`) · atomic writes and resume
(`QM-0045`) · LOD 5, which is generated on demand, never pre-built.

## Files Expected to Change

* `crates/q-tiles/src/lib.rs`
* `crates/q-daemon/src/lib.rs`
* `crates/q-daemon/src/jobs.rs`

## Files Expected to Add

* `crates/q-tiles/src/pyramid.rs`
* `crates/q-tiles/tests/pyramid_generation.rs`

## Files Expected to Remove or Deprecate

* `q_daemon::qtile_501` — replaced; the shared 501 test is narrowed, not deleted.

## Data Contracts

Output layout, content-addressed so a URL is immutable
(`ADR-CANDIDATE-019`):

```text
<out>/<model_id>/tiles/<tile_id>.qtile
```

`MortonSparseI16` is chosen **per block, by measurement**: it costs 8 B/cell
against `QuantizedI16`'s 2, so it pays only below roughly 25 % density. The
decision and the measured density are recorded on the block row.

## Memory and Performance Constraints

* One tile in memory at a time. Peak adds ≈ 512 KiB to the stream's budget.
* `MAX_QTILE_PAYLOAD_BYTES = 256 MiB` refuses anything absurd.
* < 1 ms to encode a quantized 256×256 tile.
* LOD 0–3 tiles are built by merging, so **the tensor is read exactly once**.

## Implementation Plan

1. Walk the plan bottom-up: LOD 4 first, then merge upward.
2. Per LOD-4 block: stream → statistics → choose encoding by measured density →
   `QTile` → write.
3. Per LOD-3 tensor: histogram and norms from merged block statistics; downsample
   by block-mean, which is exact given the block statistics already computed.
4. LOD 2/1/0: merge upward, no reads.
5. Cache lookup keyed by `CacheKey` before each tile.
6. Serve the route; add the job phase.

## Error Handling

* A block read failure → the tile is not written; the block is recorded failed;
  the pyramid continues.
* An encode producing an unexpected payload length → refuse (the format already
  checks `expected_payload_len`).
* A merge over zero children → error, not an empty tile: an empty tile would
  render as "no data" where data exists.
* Disk full → fail the job; nothing published (`QM-0045` makes this atomic).

## Acceptance Criteria

1. A 4096×4096 f32 tensor produces 256 LOD-4 tiles plus 1 LOD-3 tile.
2. Every tile decodes and round-trips byte-exactly.
3. LOD-3 statistics equal the merged block statistics to `1e-9`.
4. Encoding choice per block is recorded with its measured density.
5. The tensor's payload is read **exactly once** — asserted with a read counter.
6. A second run is served from cache with no recompute.
7. `GET …/{tileId}.qtile` returns the bytes with correct headers.
8. `TILE-005`…`TILE-008` still pass.

## Verification Plan

**Automated** — `pyramid_generation.rs`: counts, round trips, merge equality,
the read counter, cache reuse.
**Manual** — `xxd` the first 72 bytes of a generated tile and check the magic and
version.

## Suggested Commands

```bash
cargo test -p q-tiles                                            # verified today
cargo run -p q-cli -- convert fixtures/tiny-llama-large --lod 0-4 # introduced here
xxd -l 72 out/<model>/tiles/<tile>.qtile
curl -s localhost:PORT/v1/visualizations/<m>/tiles/<t>.qtile -o /tmp/t.qtile
```

## Test Cases

| Input | Expected |
| --- | --- |
| 4096² f32 tensor | 256 LOD-4 + 1 LOD-3 tile |
| Each tile decoded | Round-trips byte-exactly |
| LOD-3 statistics | Equal merged block statistics to `1e-9` |
| A 90 %-zero block | `MortonSparseI16` selected; density recorded |
| A dense block | `QuantizedI16` selected |
| Read counter after generation | Exactly one pass over the payload |
| Second run | Cache hits; no recompute |
| First 8 bytes of any tile | `QTILE\0\0\0` |

## Risks

| Risk | Mitigation |
| --- | --- |
| Upper LODs re-read the tensor | Merging asserted by the read counter |
| Encoding chosen by guess | Density measured per block and recorded |
| Downsampling introduces error | Block-mean downsampling is exact given the block statistics already computed; asserted |

## Completion Evidence

* Tile counts per LOD, file sizes, and total output bytes.
* Round-trip test output.
* The read-counter assertion.
* `xxd` output of a tile header.
* Encoding-choice table with measured densities.
