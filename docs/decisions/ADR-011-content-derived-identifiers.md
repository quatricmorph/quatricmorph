# ADR-011 — Content-derived 128-bit identifiers for every catalog subject

**Status:** Accepted
**Date:** 2026-08-04
**Promoted from:** `.plan/decisions/ADR-CANDIDATE-018-tensor-id.md`

## Context

Identifiers appear in catalog primary keys, cache keys, URLs, GLB feature
metadata, and log lines. They must be stable across reopen, stable across
machines, and collision-resistant, because they are the join key between the
catalog, the tile plane, the cache (ARCHITECTURE.md §13.2), and every canonical
address quoted in a report or an annotation.

Three ID kinds already ship and are tested:

* `q_source::ids::{ModelId, TensorId}` — 16 bytes, derived by `digest16`
  (`crates/q-source/src/ids.rs:81`). `SRC-006` **Verified**, including
  `tensor_ids_are_stable_across_reopen` and
  `length_prefixing_prevents_concatenation_collisions`.
* `q_tensor_runtime::TileId` (`crates/q-tensor-runtime/src/lib.rs:261`) — 16
  bytes, `TileId::for_block(tensor, lod, extent)`. `TILE-003` **Verified**
  (`tile_ids_are_stable_and_sensitive_to_extent_and_lod`).
* `q_cache::CacheKey::digest` (`crates/q-cache/src/lib.rs`) — not an ID, but the
  same construction, with the same length-prefixing rationale.

`schemas/visualization/schema.json:39,41` constrains `tile_id` and `model_id` to
`^[0-9a-f]{32}$`, and `schemas/weightql/schema.json:36` says "32-character hex
model_id". Those patterns are frozen.

Two ID kinds do **not** yet exist and are needed immediately: `StatisticsId`
(`QM-0020`) and `JobId` (`QM-0033`). `.plan/DATA_ARCHITECTURE.md` §5 names both
and gives their inputs, but not their byte-level construction — and a byte-level
construction chosen incidentally by the first implementer is frozen into
persisted rows the moment it runs.

## Decision

**Every catalog subject is identified by the leading 16 bytes of a
domain-separated BLAKE3 digest over its defining inputs.** This is the scheme
already shipped, extended uniformly rather than replaced.

The construction rule, stated at the byte level so it cannot be re-derived
differently:

```text
id = blake3( ID_SCHEME_VERSION ‖ domain ‖ 0x00 ‖ component* ).as_bytes()[..16]

where each component is appended as:
  variable-length (strings, byte slices)  →  u64 little-endian length, then bytes
  fixed-width     (u8, u32, i64, [u8;16]) →  its little-endian bytes, no prefix
```

Length-prefixing the variable-length components is what stops `("ab","c")` and
`("a","bc")` hashing alike. Fixed-width components need no prefix because their
width is not data-dependent, which is why the shipped `TileId` and `CacheKey`
constructions append `lod` and `algorithm_version` bare.

Per-kind domains and inputs:

| ID | Domain | Components |
| --- | --- | --- |
| `ModelId` | `quatricmorph/model/v1` | `source_key`, `revision`, `content_fingerprint` |
| `TensorId` | `quatricmorph/tensor/v1` | `model_id`, `raw_name` |
| `TileId` | `quatricmorph/tile/v1` | `tensor_id`, `lod`, `extent` (4 × `u32`) |
| `BlockId` | `quatricmorph/tile/v1` | **identical to `TileId`** |
| `StatisticsId` | `quatricmorph/statistics/v1` | `subject_id` (`[u8;16]`), `algorithm_version` (`u32`) |
| `JobId` | `quatricmorph/job/v1` | `model_id` (`[u8;16]`), `configuration_hash` (string), `created_at` (`i64`) |

The first three are as shipped. The last three are bound here.

Four points the formula alone does not settle, decided now:

1. **`StatisticsId` carries `algorithm_version` deliberately.** Changing the
   algorithm **mints new rows rather than overwriting history**, so two
   algorithm versions coexist for one subject and can be compared. `QM-0020`
   acceptance criterion 4 already requires exactly this.
2. **`StatisticsId` does not hash `subject_kind`.** A `TensorId` and a `TileId`
   are already separated by their own domain strings, so the 16 bytes of
   `subject_id` carry the kind implicitly. `subject_kind` remains a column for
   readability, not an identity component.
3. **`JobId` includes a timestamp, and it is the only ID here that is not purely
   content-derived.** Two jobs with identical configuration are genuinely
   distinct events. The timestamp component is the existing
   `q_catalog::job::ConversionJob::created_at` (`i64`, `schema::now_unix()`), not
   a newly invented field. **A resumed job keeps its persisted `JobId`** and
   never re-derives it — re-deriving would mint a new ID on every resume and
   defeat `QM-0033`'s resume and crash-recovery criteria.
4. **The persisted form is bare lowercase 32-character hex.** `.plan/API_CONTRACTS.md`
   §3's `"job_id": "job:…"` and `TileId::content_hash`'s `b3:` prefix are
   *display and artifact* forms layered over that; the frozen schema patterns
   `^[0-9a-f]{32}$` describe what is stored and what appears in a URL path. Where
   a prose example and a frozen schema disagree, the schema wins.

New ID kinds are added through the `define_id!` macro in `q_source::ids`, which
supplies `ID_SCHEME_VERSION`, `to_hex`, and `from_hex` for free.

## Alternatives considered

**UUIDv4 for new IDs.** Trivially collision-free and requires no thought about
inputs. Rejected, and disqualified rather than merely outscored: it is **not
deterministic**. Re-importing the same checkpoint would mint new IDs, breaking
cache reuse and the already-passing `reimporting_the_same_model_is_idempotent`.
An ID that changes when nothing changed is not an identity.

**Sequential integers from the catalog.** Compact, and natural database keys.
Rejected: not stable across machines. Two users opening the same checkpoint
would number its tensors differently, so a shared canonical URL would resolve to
different data for each of them — the precise property `SRC-006` exists to
guarantee.

**Content hashes of the actual tensor bytes.** Deduplication for free; identical
blocks would share an ID. Rejected because it inverts the architecture: an ID is
needed *before* the bytes are read, precisely so that the read can be planned and
budgeted. `content_fingerprint` (`ids.rs:139`) already makes this trade-off
explicitly at model scale — it hashes the manifest, not the weights, because
hashing 600 GB to open a model would violate the bounded-IO contract.

**A fresh scheme for the new IDs, leaving the shipped three alone.** Rejected as
the quiet failure: it would leave the repository with two ID grammars and no
statement of which applies to the next kind, which is the situation this ADR
exists to end.

## Why the domain string and the version byte are load-bearing

The domain string is what makes `TensorId` and `TileId` inhabit different spaces
even when their inputs coincide, which is what lets point 2 above drop
`subject_kind` safely.

`ID_SCHEME_VERSION` (`ids.rs:16`) is the escape hatch. Bumping it invalidates
every previously persisted ID *loudly and completely* rather than allowing a
changed construction to collide with old data. That is the intended cost of ever
changing this scheme, and it is cheap only because it is stated in advance.

**Collision probability is arithmetic, not a measurement.** At 128 bits, the
birthday bound over 10⁶ IDs is on the order of 10⁻²⁷. No collision has been
observed because none has been searched for; the number is a calculation.

## Consequences

* `QM-0020` implements
  `StatisticsId = blake3("quatricmorph/statistics/v1" ‖ subject_id ‖ algorithm_version_le)[..16]`,
  which is the formula its own `Scope` section already commits to, now with the
  byte layout fixed. Its acceptance criterion 4 (two algorithm versions coexist)
  follows from the scheme rather than being bolted onto it.
* `QM-0033` derives `JobId` once at job creation and persists it;
  `ConversionJob::job_id` stays a `String` holding the 32-hex form.
* `QM-0021` (`visual_tiles`) and `QM-0022` (`tensor_blocks`) use `TileId`
  unchanged. `BlockId = TileId` means the catalog row and the artifact address
  are two renderings of one identity, not two identities to keep in sync.
* **`TileId::for_block` is not modified.** It predates `digest16` and omits the
  `ID_SCHEME_VERSION` byte. It satisfies the construction rule above in every
  other respect, it is frozen by `TILE-003`, and tile IDs are already persisted —
  so changing it would invalidate stored data to gain nothing. New kinds go
  through `define_id!`; this one stays as it is, and this paragraph is why.
* `schemas/visualization/schema.json` and `schemas/weightql/schema.json` need no
  change: 32-hex is what this scheme produces.
* Raising `ID_SCHEME_VERSION` is a catalog-invalidating migration. It is
  designed to be, so that a scheme change is never quiet.

### What this does not unblock

`QM-0020`'s `## Dependencies` section names `QM-0012` only, and `QM-0012` merged
at `4e0e85c`. This ADR removes the decision risk that its `Scope` and
`Implementation Plan` carried by citing an unpromoted candidate; it does not
change `QM-0020`'s dependency edges. The same holds for `QM-0021`, `QM-0022`, and
`QM-0033`, none of which list `ADR-CANDIDATE-018 (decision required)` in
`Dependencies`.

## Research

* **BLAKE3 Rust crate documentation** — https://docs.rs/blake3/latest/blake3/,
  retrieved 2026-08-04. Current published version 1.8.5; the workspace pins
  `blake3 = "1"` (`Cargo.toml:53`), which is compatible. Confirms
  `Hasher::new`/`update`/`finalize`, `Hash::as_bytes`, and the existence of
  `derive_key` and `keyed_hash` as the crate's own domain-separation
  primitives. *Credibility: the crate's own generated API documentation — the
  authoritative source for its surface.*

  This **did not change the decision.** The repository's construction achieves
  domain separation by hashing an explicit domain string and a scheme-version
  byte as the first input bytes, which is already shipped and tested across
  three ID kinds. Switching to `derive_key` would alter every previously
  persisted `ModelId` and `TensorId` for no behavioural gain, so the
  repository's own convention was retained.

  The crate documentation gives no prescriptive guidance on truncating output
  below 32 bytes. The 16-byte truncation is therefore justified by the
  repository's own frozen schema patterns (`^[0-9a-f]{32}$`) and by the
  arithmetic above, not by an external recommendation.

No other external research was required. The construction, the collision
analysis, and every input this ADR binds are settled by code and schemas already
in this repository.
