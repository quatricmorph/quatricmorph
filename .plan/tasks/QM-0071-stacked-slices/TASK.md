# QM-0071 — Stacked slice composition

## Status

Blocked

Unblocks when `QM-0070` reaches `Complete`.

## Phase

Phase 07 — WeightQL and chat

## Objective

Compose `A[0:64][0:8]` into `A[0:8]` instead of returning `NotImplemented`.

## Repository Evidence

* `STATUS.md` `WQL-008` — **Stub**; *"Returns `NotImplemented` rather than
  approximating."*
* `crates/q-weightql/src/parser.rs:7-24` — the grammar already allows
  `postfix := primary ('[' selector ']')*`, so **stacking parses**; only
  composition is missing.
* `q_expression::Expr::Slice { selector, .. }` — nested slices form naturally.
* `q_nsir::{ElementSelector, IndexTerm}` — the selector types.
* `alias::tests::parses_the_five_architecture_md_forms` — including
  `Expert[12,37].up[0:128,:]`.

## Requirements Covered

`WQL-008`.

## Dependencies

`QM-0070`.

## Blocks

`QM-0073`.

## Parallelization

**Sequential after `QM-0070`** — shared file `plan.rs`.

## Program Boundary

`crates/q-weightql`, `crates/q-expression`.

## Scope

* Compose nested slices into one effective selector.
* Handle ranges, single indices, full-axis `:`, and mixtures.
* Validate that each level is within the previous level's bounds.
* Reduce rank when a single index is used, matching numpy semantics.

## Out of Scope

Negative indices · strided slices · `reshape` · slicing a computed intermediate,
which is a different composition problem.

## Files Expected to Change

* `crates/q-weightql/src/plan.rs`
* `crates/q-expression/src/lib.rs`

## Files Expected to Add

* `crates/q-weightql/tests/slice_composition.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Composition rules, stated so the semantics are unambiguous:

```text
A[a:b][c:d]      → A[a+c : a+d]        requires a+d ≤ b
A[a:b][i]        → A[a+i]              requires a+i < b   (rank reduces by 1)
A[a:b][:]        → A[a:b]
A[i][j]          → error: A[i] has rank-1, so [j] indexes the remaining axis
A[a:b, c:d][e:f, g:h] → A[a+e : a+f, c+g : c+h]
```

**Out-of-bounds composition is an error, not a clamp.** `A[0:64][0:100]` refers
to elements that are not in `A[0:64]`, and clamping would silently return
different data than the user asked for.

## Memory and Performance Constraints

Pure index arithmetic; no reads. Composition happens at planning time, so the
executed read is the composed extent — `A[0:64][0:8]` reads **8 rows, not 64**.

## Implementation Plan

1. Add `ElementSelector::compose(outer, inner) -> Result<ElementSelector>`.
2. Handle each combination in the table, including rank reduction.
3. Bounds-check every level against the previous.
4. Apply composition during planning, before byte-range resolution.
5. Assert the composed read touches only the final extent.
6. Tests over every combination, including the error cases.

## Error Handling

* Inner range exceeding the outer extent → error naming both.
* Indexing beyond the reduced rank → error stating the current rank.
* An empty result (`a:a`) → an empty tensor, which is legal, not an error.
* Deep stacks (> 8 levels) → refused, to bound planning cost.

## Acceptance Criteria

1. `A[0:64][0:8]` composes to `A[0:8]`.
2. `A[10:20][2]` composes to `A[12]`, with rank reduced.
3. `A[0:64, 0:64][0:8, 0:8]` composes to `A[0:8, 0:8]`.
4. `A[0:64][0:100]` is an error naming both bounds — **not a clamp**.
5. `A[i][j]` on a rank-2 tensor errors, stating the rank after the first index.
6. `A[0:64][:]` composes to `A[0:64]`.
7. The composed read touches only the final extent — asserted by bytes read.
8. Composition is applied before byte-range resolution.
9. `WQL-002` and `WQL-005` still pass.

## Verification Plan

**Automated** — `slice_composition.rs` over every table row and error case, plus
a bytes-read assertion.
**Manual** — `q-cli query` with a stacked slice; check the reported bytes.

## Suggested Commands

```bash
cargo test -p q-weightql                                     # verified today
cargo run -p q-cli -- query fixtures/tiny-llama-2shard \
  'SELECT slice FROM tensor("Q[10]") ROWS 0:64'              # works today
cargo run -p q-cli -- query … 'show tensor("Q[10]")[0:64][0:8]'   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| `A[0:64][0:8]` | `A[0:8]` |
| `A[10:20][2]` | `A[12]`, rank − 1 |
| `A[0:64,0:64][0:8,0:8]` | `A[0:8,0:8]` |
| `A[0:64][:]` | `A[0:64]` |
| `A[0:64][0:100]` | Error, both bounds named |
| `A[5][3]` on rank-2 | Error naming the rank |
| `A[0:0]` | Empty tensor, legal |
| 9 stacked levels | Refused |
| Bytes read for `A[0:64][0:8]` | Equals `A[0:8]`'s cost |

## Risks

| Risk | Mitigation |
| --- | --- |
| Off-by-one in composition | Every table row is a test case |
| Out-of-bounds silently clamped | Error asserted explicitly |
| Composition applied after byte planning, reading too much | Bytes-read assertion |

## Completion Evidence

* Test output for every composition case.
* The bytes-read comparison proving the composed extent is what is read.
* Error messages for the out-of-bounds and rank cases.
