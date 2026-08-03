# QM-0023 — Candidate resolution surfaced end to end

## Status

Blocked

Unblocks when `QM-0005` reaches `Complete`.

## Phase

Phase 02 — Catalog and NSIR completion

## Objective

Verify that an ambiguous alias returns **candidates, never a silent pick**, from
the resolver through the catalog and the daemon to a documented API contract.

## Repository Evidence

* `crates/q-nsir/src/resolver.rs` —
  `ambiguous_alias_returns_candidates_not_a_silent_pick`. `NSIR-007` verified.
* `crates/q-daemon/src/lib.rs` —
  `an_ambiguous_alias_is_a_409_carrying_its_candidates`. `API-007` verified.
* `crates/q-catalog/src/lib.rs` —
  `role_and_layer_filters_drive_alias_resolution`. `CAT-005` verified.
* `ARCHITECTURE.md` §6.2 — the `Att[10]` example: Q, K, V, O, or attention
  metadata; *"the query must return a list of candidates rather than silently
  picking one tensor."*

## Requirements Covered

`NSIR-007`, `API-007`, `CAT-005`, `MVP-34`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0075`.

## Parallelization

Fully parallel — test and documentation only, no shared source file.

## Program Boundary

`crates/q-nsir`, `crates/q-daemon` (tests); `schemas/weightql/schema.json`
(candidate shape).

## Scope

* End-to-end test: ambiguous alias → resolver → catalog → daemon → 409 with
  candidates.
* Fix the **candidate payload shape** in `schemas/weightql/schema.json` so the UI
  has a contract to build against.
* Cover the `ARCHITECTURE.md` §6.2 cases: `Att[10]`, `MLP[10]`, `Expert[12]`,
  and a bare `Q` with no layer index.
* Assert candidates are **ordered deterministically**, so the UI's first entry is
  stable across runs.

## Out of Scope

The candidate UI (`QM-0075`) · using the current selection to disambiguate
(`QM-0074`) · new aliases.

## Files Expected to Change

* `schemas/weightql/schema.json`
* `crates/q-daemon/src/lib.rs` — response shape only, if it lacks a field

## Files Expected to Add

* `tests/tests/candidate_resolution.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
// 409
{ "error": "ambiguous_alias", "message": "\"Att[10]\" matches 4 tensors",
  "input": "Att[10]",
  "candidates": [
    { "canonical_address": "model.layers[10].self_attention.query_projection.weight",
      "raw_name": "model.layers.10.self_attn.q_proj.weight",
      "tensor_id": "…", "role": "attention_query_projection",
      "shape": [4096, 4096], "dtype": "F32", "layer_index": 10,
      "suggested": false, "suggestion_reason": null }
  ] }
```

`suggested` is present but **always `false` here** — this task establishes the
field; `QM-0074` is what may set it, visibly and with a reason. A candidate list
that pre-selects without saying why is the failure this whole path exists to
avoid.

Ordering: by role in a fixed order (Q, K, V, O, …), then by canonical address.
Deterministic, so the UI is stable.

## Memory and Performance Constraints

Candidate resolution is a catalog filter query, indexed by
`(model_id, role, layer_index)` — the index already exists. Under 20 ms.

## Implementation Plan

1. Add the candidate fields to `schemas/weightql/schema.json`.
2. Confirm the daemon response matches; add any missing field.
3. Implement deterministic ordering in the resolver.
4. Write the cross-crate test covering all four ambiguity cases.
5. Add a test that a **non**-ambiguous alias returns 200, not 409 — the negative
   case matters as much.

## Error Handling

* Zero candidates → **404**, not an empty 409. "Ambiguous with no options" is not
  a meaningful state.
* One candidate → **200**, resolved. A 409 with one option would be noise.
* Many candidates → 409, all of them, no truncation. Truncating would silently
  hide the tensor the user wanted.

## Acceptance Criteria

1. `Att[10]` returns 409 with ≥ 4 candidates.
2. `Q[10]` returns 200 — unambiguous.
3. `NoSuchAlias[10]` returns 404.
4. Candidate order is identical across runs.
5. Every candidate carries all nine documented fields.
6. `suggested` is `false` for every candidate in this task.
7. The response validates against the schema.
8. `NSIR-007` and `API-007` still pass.

## Verification Plan

**Automated** — `tests/tests/candidate_resolution.rs`; schema validation of the
response.
**Manual** — `curl` an ambiguous alias against the running daemon.

## Suggested Commands

```bash
cargo test -p q-nsir -p q-daemon                         # verified today
cargo test --test candidate_resolution                    # introduced here
curl -s "localhost:PORT/v1/query" -d '{"expression":"show tensor(\"Att[10]\")"}' | jq
```

## Test Cases

| Input | Expected |
| --- | --- |
| `Att[10]` | 409, ≥ 4 candidates, ordered Q, K, V, O |
| `MLP[10]` | 409 with gate, up, down |
| `Expert[12]` | 409 across that expert's projections |
| `Q` (no layer) | 409 with one candidate per layer |
| `Q[10]` | 200 |
| `NoSuchAlias[10]` | 404 |
| Same query twice | Byte-identical candidate order |
| Response vs schema | Valid |

## Risks

| Risk | Mitigation |
| --- | --- |
| A candidate list is truncated for UI convenience | No truncation; asserted |
| Ordering becomes implementation-defined | Fixed role order, then address; asserted across runs |
| `suggested` is set silently later | The field requires `suggestion_reason` whenever true; enforced by schema |

## Completion Evidence

* Test output for all eight cases.
* A `curl` response body for `Att[10]`.
* Schema validation output.
