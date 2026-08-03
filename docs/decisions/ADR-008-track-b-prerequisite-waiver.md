# ADR-008 — Proceeding with Track B work ahead of its prerequisite gate

**Status:** Accepted (waiver, granted by explicit user instruction)
**Date:** 2026-08-03
**Waives:** `docs/requirements/PREREQUISITES.md:67`

## Context

`docs/requirements/PREREQUISITES.md:67` states:

> Autonomous agents must **not** start Track B until G3 Track B prerequisites
> are complete.

Track B is "Phase 1 Dense Model Browser", and its listed prerequisites — Rust
workspace scaffold, architecture plugin layout, frozen NSIR/catalog schema
draft, daemon/CLI interface sketch — were all unchecked. The document recommends
Track A (the Phase 0 tiling spike) as the next sprint.

The work commissioned for this pass is squarely Track B: SafeTensors ingestion,
NSIR, the catalog, WeightQL, and the local daemon.

## Decision

Proceed, and record the waiver rather than walking past the gate silently.

The instruction to build these subsystems was explicit and detailed, down to
the crate list and the acceptance test. `PREREQUISITES.md:65` allows for exactly
this: gates may be *"explicitly waived by the user for a spike"*.

## What this pass actually completed of the G3 Track B checklist

| Prerequisite | Status after this pass | Evidence |
| --- | --- | --- |
| Rust workspace scaffold exists (`q-safetensors`, `q-architecture`, `q-nsir`, `q-catalog`, …) | **Done** | 17 crates, all building and tested |
| Architecture plugin layout under `architectures/` | **Done** | `architectures/*/plugin.toml`, loaded by `q_architecture::Registry` |
| NSIR / catalog schema draft frozen | **Done** | `schemas/nsir/schema.json`, `crates/q-catalog/src/schema.rs` migration 1 |
| Local daemon / CLI interface sketched (`q-daemon`, `q-cli`) | **Done** | Both implemented, not merely sketched |

The G2 test-baseline items are also satisfied: a fixture policy with no network
downloads (`fixtures/`, checked in), seed tests for header parsing, byte-range
reads, and scalar equality against a Python `safetensors` reference
(`tests/tests/end_to_end_scalar_slice.rs`), and canonical-address round-trip
tests (`crates/q-nsir`).

## What this pass did **not** complete

Track A (Phase 0 tiling) remains open. `TILE-04`, `TILE-05`, `TILE-06`,
`TILE-08`, and `TILE-11` are unmet: there is no tile pyramid, no `tileset.json`,
no CesiumJS viewer, and no click-to-address path. `TILE-07` — the exact scalar
matching a Python reference — **is** met, by the Section 7 slice.

## Consequences

* `docs/requirements/PREREQUISITES.md` should have its G3 Track B boxes ticked
  to reflect the table above. That file was left unmodified in this pass because
  it is a checklist owned by the project, not by this ADR.
* The next sprint has a clear shape: Track A's tiling spike, now sitting on top
  of a real catalog and a real query layer rather than on scaffolding.
