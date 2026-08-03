# Phase 07 — WeightQL and chat

## Goal

```text
Selector or natural-language request → validated query plan → cost preview
→ explicit execution → viewer or matrix action, labelled by fidelity
```

## What is already done

WeightQL is the most complete subsystem in the repository: 10 of 12 requirements
`Verified`. Lexer, parser, resolver, shape checker, cost estimator, and plan-ID
generator all work, and **scalar and slice queries execute**.

On the browser side, `CHAT-002` (a parser matching the Rust grammar, 17 tests),
`CHAT-003` (KaTeX rendering the parser's grouping, not the source order), and
`CHAT-004` (an error caret under the offending character) are all `Verified`.

## The gaps

| Gap | Requirement |
| --- | --- |
| Matmul plans but does not execute | `WQL-006` |
| No statistical `SELECT … GROUP BY` | `WQL-007` |
| No stacked slice composition | `WQL-008` |
| No execution-tier selection recorded in the plan | `WQL-013` |
| No query cancellation | `API-011` |
| **No chat layer at all** — deliberately not built | `CHAT-001` |
| No candidate-resolution UI | `CHAT-005` |
| No stated KaTeX sanitization contract | `SEC-006` |

## Entry conditions

* `QM-0020` complete (statistics persisted), for `QM-0072`.
* `QM-0031` complete (a compute path exists), for `QM-0070`.
* `ADR-CANDIDATE-011` (SSE transport) and `012` (parser strategy) decided.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0070` | Matrix-multiplication execution | Implementation | `WQL-006`, `MVP-28`, `MVP-36` |
| `QM-0071` | Stacked slice composition | Implementation | `WQL-008` |
| `QM-0072` | Statistical `SELECT … GROUP BY layer_index` | Implementation | `WQL-007` |
| `QM-0073` | Execution tiers, cost preview, cancellation | Implementation | `WQL-013`, `API-011`, `MVP-38`, `MVP-39` |
| `QM-0074` | Chat → WeightQL plan builder | Implementation | `CHAT-001`, `MVP-40` |
| `QM-0075` | Candidate UI, KaTeX sanitization, origin policy | Implementation | `CHAT-005`, `SEC-006`, `SEC-007`, `MVP-34`, `MVP-37` |

## Design constraints — the boundary that defines this phase

> `ARCHITECTURE.md` §15: *"Chat must not read weight bytes directly. It calls the
> WeightQL planner instead."*
> §19: *"Do not let chat freely execute terabyte-scale expressions."*

* **Chat's only output is a WeightQL string.** It has no `ModelSource`, no file
  access, and no execution authority. `QM-0074` asserts this structurally, with a
  test that chat cannot reach a byte route.
* **Shape checking happens before anything can read**, by construction:
  `infer_shape` is pure and has no `ModelSource`, so it *cannot* reach a disk or
  a GPU. Already `Verified`.
* **Every plan carries a cost.** Above `WARN_READ_BYTES` (64 MiB), execution
  needs a second confirmation. Above `MAX_READ_BYTES` (4 GiB), it is refused.
  A whole-tensor read is refused **categorically, at any size**.
* **`execute` without a matching `plan_id` is rejected**, so a cost the user
  never saw cannot be paid.
* **An ambiguous alias returns candidates.** The UI presents them; the current
  selection may be *offered* as a default, visibly, with the reason stated — never
  applied silently.
* **KaTeX renders LaTeX generated from the validated AST, never the user's raw
  string.** A string the parser rejected never reaches the renderer at all, which
  is stronger than escaping.
* **No arbitrary code execution**, enforced by a closed enum rather than by
  filtering.
* Cancellation is **acknowledged**, and its latency is bounded by one block. A
  cancel that leaves the UI spinning is worse than no cancel.

## Exit conditions

1. `Q[10][0:256,0:256] @ transpose(K[10][0:256,0:256])` executes and matches the
   CPU reference.
2. `A[0:64][0:8]` composes to `A[0:8]` rather than returning `NotImplemented`.
3. `SELECT layer_index, l2_norm(weight) … GROUP BY layer_index` returns one row
   per layer, in under 1 s on the 47 278-tensor synthetic manifest.
4. Every plan reports its execution tier and its fidelity.
5. A running query cancels within one block, and partial results are labelled
   partial.
6. Chat turns each of the seven §21 example requests into the expected WeightQL,
   including honestly reporting the one that needs `WQL-007`.
7. An ambiguous alias shows candidates in the UI and does not choose.
8. The KaTeX configuration is asserted by a test: `trust: false`,
   `strict: "error"`, bounded `maxExpand`.
9. The daemon binds to `127.0.0.1`, enforces a CORS allowlist, and never `*`.
10. The Rust and TypeScript parsers agree on the shared conformance corpus.

## Parallelization

`QM-0070`, `QM-0071`, `QM-0072` all touch `crates/q-weightql/src/plan.rs` —
**sequential**. `QM-0073` follows them. `QM-0074` and `QM-0075` touch
`apps/web/query-interface/` and run in parallel with the Rust work, merging after
`QM-0073`.

## Risks

| Risk | Mitigation |
| --- | --- |
| R6 — two parsers drift | `QM-0074` adds a shared conformance corpus; drift becomes a red test |
| Chat acquires capabilities by convenience | `QM-0074` asserts the absence structurally, not by review |
| `WQL-007` slow on SQLite | Measured; `ADR-CANDIDATE-005` reopens only above 1 s |
