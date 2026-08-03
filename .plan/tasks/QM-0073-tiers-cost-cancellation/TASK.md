# QM-0073 — Execution tiers, cost preview, and cancellation

## Status

Blocked

Unblocks when `QM-0072` reaches `Complete`.

## Phase

Phase 07 — WeightQL and chat

## Objective

Record which tier will run, require explicit execution for expensive queries, and
make cancellation real and acknowledged.

## Repository Evidence

* `WQL-010` Verified — every plan carries an I/O cost estimate.
* `WQL-011` Verified — `whole_tensor_reads_are_refused_with_an_explanation`.
* `WQL-012` Verified — deterministic, quotable plan IDs.
* `q_expression::Expr::needs_backend()` — the read-versus-compute distinction.
* `crates/q-source/src/cancel.rs` — the token used by ingestion;
  `cancellation_stops_at_a_shard_boundary`.
* `QM-0033`'s SSE progress transport (`ADR-CANDIDATE-011`).

## Requirements Covered

`WQL-013`, `API-011`, `MVP-38`, `MVP-39`.

## Dependencies

`QM-0072`, `QM-0070`, `QM-0033`.

## Blocks

`QM-0074`, `QM-0080`.

## Parallelization

Sequential after `QM-0072` — shared `plan.rs`.

## Program Boundary

`crates/q-weightql`, `crates/q-daemon`.

## Scope

* Tier selection: `metadata | catalog | exact-read | cpu-block | gpu-block |
  sampled`, recorded in the plan.
* Cost thresholds: warn at 64 MiB, refuse at 4 GiB, refuse whole-tensor reads
  categorically.
* `requires_confirmation` on the plan; `execute` requires a matching `plan_id`.
* `POST /v1/query/{planId}/cancel`, acknowledged.
* Cancellation checked between blocks; partial results labelled partial.

## Out of Scope

The cost UI (`QM-0075`) · chat (`QM-0074`) · new execution capability.

## Files Expected to Change

* `crates/q-weightql/src/plan.rs`
* `crates/q-weightql/src/execute.rs`
* `crates/q-daemon/src/lib.rs`
* `schemas/weightql/schema.json`

## Files Expected to Add

* `crates/q-weightql/tests/tiers_and_cancellation.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "plan_id": "plan:b3:…", "execution_tier": "cpu-block",
  "tier_reason": "expression requires a compute backend; 512 KiB read is below the GPU threshold",
  "estimated_read_bytes": 524288, "estimated_gpu_bytes": 0,
  "requires_confirmation": false, "fidelity": "exact" }
```

**Rules:** prefer the cheapest tier meeting the requested fidelity; **never
silently downgrade fidelity to save cost** — refuse and explain instead; a GPU
tier requires a *verified* backend, so an unverified CUDA build never silently
becomes the executor.

## Memory and Performance Constraints

* Tier selection is metadata arithmetic; < 10 ms.
* Cancellation latency bounded by **one block**.
* Partial results are returned, not discarded — the work is already paid for.

## Implementation Plan

1. Implement `select_tier(plan) -> (Tier, reason)` with the rules above.
2. Add `requires_confirmation` when cost exceeds `WARN_READ_BYTES`.
3. Reject `execute` without a matching `plan_id`.
4. Add a cancellation token to execution, checked between blocks.
5. Add the cancel route; return 202 on acknowledgement, 404 for an unknown plan.
6. Return partial results with `status: "cancelled"` and a coverage note.
7. Update the schema.

## Error Handling

* Cost above `MAX_READ_BYTES` → 413 naming the threshold and suggesting a
  narrower slice.
* Whole-tensor read → refused categorically, at any size, with an explanation.
* `execute` with a stale `plan_id` → 400. A cost the user never saw cannot be
  paid.
* Cancel on an unknown plan → 404.
* Cancel on a completed plan → 200, no-op.
* **A cancel that leaves the UI spinning is worse than no cancel** — the
  acknowledgement is part of the contract.

## Acceptance Criteria

1. Every plan reports its tier and a human-readable reason.
2. A metadata query selects the metadata tier and reads nothing.
3. A slice query selects exact-read.
4. A matmul selects cpu-block, or gpu-block only when a **verified** backend
   exists and the size threshold is met.
5. Cost above 64 MiB sets `requires_confirmation: true`.
6. Cost above 4 GiB returns 413.
7. `execute` with a stale plan ID returns 400.
8. Cancellation is acknowledged within one block.
9. Partial results are labelled `cancelled` with coverage stated.
10. Fidelity is never silently downgraded — a refusal is issued instead.

## Verification Plan

**Automated** — `tiers_and_cancellation.rs` for tier selection, thresholds, and
cancellation timing; daemon tests for the routes.
**Manual** — start a long query, cancel it, confirm the response and the timing.

## Suggested Commands

```bash
cargo test -p q-weightql -p q-daemon                            # verified today
curl -X POST localhost:PORT/v1/query -d '{"expression":"…","mode":"plan"}'
curl -X POST localhost:PORT/v1/query/plan:b3:…/cancel           # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| `tensor("Q[10]")` metadata only | Tier `metadata`; zero bytes read |
| Slice query | Tier `exact-read` |
| Matmul, CPU only | Tier `cpu-block` |
| Matmul, unverified CUDA present | **`cpu-block`**, reason recorded |
| 100 MiB read | `requires_confirmation: true` |
| 8 GiB read | 413 naming the threshold |
| Whole-tensor read | Refused categorically |
| Execute with a stale plan ID | 400 |
| Cancel mid-execution | 202; stops within one block |
| Cancelled query result | `status: cancelled`, coverage stated |
| Cancel unknown plan | 404 |

## Risks

| Risk | Mitigation |
| --- | --- |
| Cancellation checked too coarsely to feel responsive | Bounded by one block; the timing is an acceptance criterion |
| Fidelity silently downgraded to fit a budget | Refusal instead; asserted |
| Unverified GPU becomes the default executor | `select_tier` requires `verified`; asserted |

## Completion Evidence

* Tier-selection output for each query kind.
* Threshold-refusal responses.
* Cancellation timing measurements.
* A partial-result payload showing coverage.
