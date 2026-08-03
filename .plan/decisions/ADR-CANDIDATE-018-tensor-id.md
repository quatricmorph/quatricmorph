# ADR-CANDIDATE-018 — Canonical tensor ID generation

## Status

`Open` — the implementation is settled; the question is whether tile and block
IDs should follow the same scheme as the pipeline extends.

## Context

IDs appear in catalog keys, cache keys, URLs, GLB feature metadata, and log
lines. They must be stable across reopen, stable across machines, and collision-
resistant.

## Repository evidence

* `crates/q-source/src/ids.rs` (202 lines) — `ModelId`, `TensorId`, 16 bytes each.
* `Cargo.toml` — `blake3 = "1"`.
* `SRC-006` **Verified**: `tensor_ids_are_stable_across_reopen`, plus `ids::tests::*`.
* `q_tensor_runtime::TileId` (`:261`) — 16 bytes,
  `TileId::for_block(tensor, lod, extent)`, `to_hex()`, `content_hash()`.
* `TILE-003` **Verified**: `tile_ids_are_stable_and_sensitive_to_extent_and_lod`.
* `q_cache::CacheKey` (`:47`) — **length-prefixed** components;
  `length_prefixing_prevents_field_boundary_collisions`.
* `schemas/visualization/schema.json` — `tile_id` and `model_id` are
  `^[0-9a-f]{32}$`, i.e. 16 bytes hex.

## Decision required

Keep BLAKE3-over-length-prefixed-components for all four ID kinds as the pipeline
extends to blocks, statistics, and jobs?

## Options

| Option | |
| --- | --- |
| **A** | Keep the current scheme everywhere |
| **B** | UUIDv4 for new IDs |
| **C** | Sequential integers from the catalog |
| **D** | Content hashes of the actual bytes |

## Advantages

* **A** — deterministic, so the same input yields the same ID on any machine and
  in any run; already verified for three ID kinds; 128 bits is ample; length
  prefixing closes the boundary-collision hazard.
* **B** — trivially collision-free.
* **C** — compact; natural database keys.
* **D** — deduplication for free; identical blocks share an ID.

## Disadvantages

* **A** — a 32-character hex string in a URL is not friendly. Acceptable:
  canonical addresses are the human-facing identifier; IDs are machine-facing.
* **B** — **not deterministic.** Re-importing the same model would mint new IDs,
  breaking cache reuse and `reimporting_the_same_model_is_idempotent`.
  Disqualifying.
* **C** — not stable across machines; a shared URL would resolve differently for
  two users of the same model.
* **D** — requires reading the bytes to compute the ID, which inverts the
  architecture: IDs are needed *before* reading, precisely so a read can be
  planned.

## Risks

Collision probability at 128 bits with 10⁶ IDs is ~10⁻²⁷. Not a risk.

The real risk is **field-boundary collision** — `("ab","c")` hashing the same as
`("a","bc")` — and it is already closed by length prefixing, with a test named for
it.

## Recommended default

**A**, extended uniformly:

```text
ModelId       = blake3(len ‖ source_uri ‖ len ‖ source_revision ‖ len ‖ source_hash)[..16]
TensorId      = blake3(len ‖ model_id   ‖ len ‖ raw_name)[..16]
TileId        = blake3(len ‖ tensor_id  ‖ len ‖ lod ‖ len ‖ extent)[..16]
BlockId       = TileId                       # same inputs; one is the catalog key, one the artifact key
StatisticsId  = blake3(len ‖ subject_id ‖ len ‖ algorithm_version)[..16]
JobId         = blake3(len ‖ model_id ‖ len ‖ config_hash ‖ len ‖ started_at)[..16]
```

`StatisticsId` including `algorithm_version` is deliberate: changing the algorithm
**mints new rows rather than overwriting history**, so two algorithm versions can
be compared instead of one silently replacing the other.

`JobId` includes a timestamp because two jobs with identical configuration are
genuinely distinct events — the only ID here that is not purely content-derived,
and for a reason worth stating.

## Tasks affected

`QM-0021` (`visual_tiles` rows), `QM-0022` (`tensor_blocks`), `QM-0020`
(statistics), `QM-0033` (jobs).

## Decision deadline

Before `QM-0021`.
