# QM-0013 — Trillion-scale manifest generator as a tool

## Status

Blocked

Unblocks when `QM-0005` reaches `Complete`.

## Phase

Phase 01 — SafeTensors ingestion completion

## Objective

Promote the synthetic trillion-parameter manifest from a test fixture to a
reusable tool, and express its bounded-memory assertion against a **named
budget** rather than a literal.

## Repository Evidence

* `crates/q-catalog/tests/trillion_scale_manifest.rs` (400 lines) —
  `trillion_parameter_manifest_indexes_and_queries_within_a_bounded_budget`:
  47 278 tensors, 1.048×10¹² parameters, 2.10 TB described, **35.7 MB peak**
  (56 040:1), opening no artifact.
* `crates/q-source/src/budget.rs` (101) — named, enforced budgets;
  `a_tight_metadata_budget_is_enforced`.
* `STATUS.md` `CAT-006` — `Verified`; the strongest evidence in the repository.

## Requirements Covered

`CAT-006`, `MVP-05`; supports `PERF-004`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0072` (statistical queries at scale), `QM-0084` (scaling benchmarks).

## Parallelization

Parallel with `QM-0010`…`QM-0012`. Touches a test and adds a tool.

## Program Boundary

`crates/q-cli` (new subcommand), `crates/q-catalog` (test refactor).

## Scope

* Extract manifest generation into `q_catalog::synthetic` or a small tool crate.
* Add `q-cli synth-manifest --tensors N --parameters P --out DIR`.
* Parameterize by tensor count so `QM-0084` can measure 10³, 10⁴, 10⁵.
* Replace the literal memory assertion with `MAX_METADATA_BYTES`, so an
  **intentional** budget change is visible in review rather than silently passing.
* Keep the existing test's exact scenario as one parameterization.

## Out of Scope

Generating real weight bytes · a manifest that any reader could open as a
checkpoint · changing the catalog schema.

## Files Expected to Change

* `crates/q-catalog/tests/trillion_scale_manifest.rs`
* `crates/q-cli/src/main.rs`
* `crates/q-source/src/budget.rs` — if a named budget needs adding

## Files Expected to Add

* `crates/q-catalog/src/synthetic.rs`

## Files Expected to Remove or Deprecate

None. **The existing test's scenario and its assertion must survive**, since it is
the evidence behind `CAT-006`.

## Data Contracts

The generator emits `model.safetensors.index.json` plus per-shard headers with
**no payload**. Files describe byte ranges that do not exist, which is exactly
the point: **indexing must never open them.**

A guard is required so this cannot be mistaken for a real checkpoint: the
generated directory carries a `SYNTHETIC` marker file, and any attempt to read a
payload range from it fails with a message naming the marker.

## Memory and Performance Constraints

```text
peak_allocation ≤ MAX_METADATA_BYTES          default 256 MiB
observed today  = 35.7 MB at 47 278 tensors
scaling         = O(tensor_count), not O(described bytes)
```

Generation of 47 278 headers under 30 s.

## Implementation Plan

1. Extract the generator, parameterized by tensor count, layer count, and shape
   distribution.
2. Add the `SYNTHETIC` marker and the payload-read guard.
3. Add the CLI subcommand.
4. Rewrite the assertion against `MAX_METADATA_BYTES`; keep the observed value in
   a comment as the historical measurement.
5. Add parameterizations at 10³, 10⁴, 10⁵ for `QM-0084`.

## Error Handling

* Requested parameters exceeding `u64` → refused before generating.
* Insufficient disk for the header files → fail before writing.
* A payload read against a synthetic model → refuse, naming the marker.
* Exceeding the budget during indexing → fail naming the budget and the observed
  peak.

## Acceptance Criteria

1. `q-cli synth-manifest --tensors 47278` reproduces the existing scenario.
2. The existing test still passes with the same scenario, now asserting against
   the named budget.
3. Peak allocation is measured and reported, not hard-coded.
4. Parameterizations at 10³, 10⁴, 10⁵ all index under budget.
5. A payload read against synthetic data is refused.
6. `CAT-006` does not regress; the described-to-peak ratio is reported.
7. Indexing opens **no** artifact — asserted, not assumed.

## Verification Plan

**Automated** — the existing test, parameterized; a new test that a synthetic
payload read is refused.
**Manual** — run the CLI at 10⁵ and observe the reported peak.

## Suggested Commands

```bash
cargo test -p q-catalog --test trillion_scale_manifest          # verified today
cargo run -p q-cli -- synth-manifest --tensors 47278 --out /tmp/synth   # new
cargo run -p q-cli -- inspect /tmp/synth                                 # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| 47 278 tensors, 1.048×10¹² params | Indexes; peak ≤ `MAX_METADATA_BYTES`; ratio reported |
| 10³ tensors | Indexes; peak roughly 47× smaller |
| 10⁵ tensors | Indexes under budget; time linear in count |
| A payload range read on synthetic data | Refused, naming `SYNTHETIC` |
| Budget lowered to 1 MB | Indexing fails naming budget and observed peak |
| Hierarchy query on the synthetic model | Returns one summary per layer |

## Risks

| Risk | Mitigation |
| --- | --- |
| Synthetic data mistaken for a real checkpoint | The `SYNTHETIC` marker and the read guard |
| Weakening the strongest evidence in the repository | The original scenario is retained verbatim as one parameterization |
| A named budget is quietly raised to pass | Budget changes are visible in review; the observed peak is reported in output |

## Completion Evidence

* Test output with tensor count, parameter count, described bytes, peak
  allocation, and ratio.
* CLI output at 10³, 10⁴, 10⁵ with timings.
* The payload-refusal test output.
