# ADR-CANDIDATE-004 — SafeTensors library selection

## Status

`Decided` — recording an existing decision.

## Context

SafeTensors parsing could use the official `safetensors` Rust crate or a
purpose-written parser. The repository has one already.

## Repository evidence

* `crates/q-safetensors/` — `header.rs` (391), `index.rs` (131), `ingest.rs`
  (435), `read.rs` (361). Own implementation.
* `Cargo.toml` workspace dependencies — **no `safetensors` crate**. Dependencies
  are `serde`, `serde_json`, `thiserror`, `memmap2`, `blake3`, `rusqlite`, `lru`,
  `tempfile`, `toml`, `clap`, `tokio`, `axum`, `tower`, `tracing`.
* 18 `SRC-*` requirements, 17 `Verified`.
* `docs/decisions/ADR-002-crates-rewritten-not-migrated.md`.
* `fixtures/tiny-llama-2shard/golden.json` — reference values from
  `safetensors==0.8.0` (Python), asserted by
  `tests/tests/end_to_end_scalar_slice.rs`.

## Decision required

None. Recording why the own parser stands.

## Options

| Option | |
| --- | --- |
| **A** | Own parser (current) |
| **B** | The official `safetensors` Rust crate |

## Advantages of A

* **Byte-range reads without materializing a tensor.** The official crate's API is
  oriented toward getting a tensor's bytes; the architecture's premise is reading
  4 bytes out of a 2 TB checkpoint. `scalar_read_touches_only_dtype_width_bytes`
  is the requirement, and it is easier to guarantee when you own the read path.
* Cancellation and resume at shard boundaries.
* Named, enforced memory budgets (`SRC-017`) integrated with `q_source::budget`.
* Refusals that carry requirement IDs — the repository-wide idiom.
* `absurd_header_length_is_refused_before_allocating`, which is a property of
  *our* allocation policy.

## Disadvantages of A

Maintenance burden; format changes must be tracked; ~1 300 lines that could have
been a dependency.

## Risks

Divergence from the reference implementation. **Mitigated by the strongest check
available**: golden values produced by the official Python library, asserted in
CI, with the fixtures regenerated and diffed to prove reproducibility.

## Recommended default

**A.** No action. The Python reference in `golden.json` is the correctness
anchor, and it is a better one than sharing a Rust implementation would be —
because it is an *independent* implementation, not the same code twice.

## Tasks affected

None. `QM-0003` extends `golden.json` for the larger fixture.

## Decision deadline

Passed.
