# QM-0074 — Chat → WeightQL plan builder

## Status

Blocked

Unblocks when `QM-0073` reaches `Complete`.

## Phase

Phase 07 — WeightQL and chat

## Objective

Turn a natural-language request into a WeightQL plan — and make it
**structurally impossible** for chat to read checkpoint bytes.

## Repository Evidence

* `STATUS.md` `CHAT-001` — **Not Started**; *"Deliberately not built; see
  `apps/web/query-interface/README.md`."*
* `apps/web/query-interface/src/weightql.ts` — the browser parser, 17 tests
  (`CHAT-002`, `SEC-003` Verified).
* `katex-preview.ts` — `renders_the_grouping_the_parser_chose_not_the_source_order`
  (`CHAT-003`).
* `app.ts` — `reports_a_caret_under_the_offending_character` (`CHAT-004`).
* `ARCHITECTURE.md` §15: *"Chat must not read weight bytes directly. It calls the
  WeightQL planner instead."* §19: *"Do not let chat freely execute
  terabyte-scale expressions."*
* `ADR-CANDIDATE-012` — two hand-written parsers plus a shared corpus.

## Requirements Covered

`CHAT-001`, `MVP-40`.

## Dependencies

`QM-0073`, `QM-0023`.

## Blocks

`QM-0080`.

## Parallelization

Parallel with `QM-0075` — different modules, coordinate on the shell.

## Program Boundary

`apps/web/query-interface`.

## Scope

* Intent classification **by parse attempt**, in order: canonical address → alias
  → WeightQL → natural language.
* A rule-based natural-language → WeightQL mapper covering the seven §21
  examples.
* Query history with plan IDs, costs, and whether each executed.
* Suggested selectors derived from a **catalog query**, not a guess.
* Current-selection context, shown persistently.
* The shared parser conformance corpus (`ADR-CANDIDATE-012`).

## Out of Scope

A language model · free-form conversation · executing anything without an
explicit act · candidate UI (`QM-0075`).

## Files Expected to Change

* `apps/web/query-interface/src/app.ts`
* `apps/web/query-interface/src/weightql.ts`

## Files Expected to Add

* `apps/web/query-interface/src/chat/{intent,mapper,history,context}.ts`
* `schemas/weightql/conformance.json`
* `apps/web/query-interface/src/__tests__/chat.test.ts`
* `crates/q-weightql/tests/conformance_corpus.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The shared corpus is asserted by **both** parsers:

```jsonc
[ { "input": "show tensor(\"Q[10]\") @ transpose(tensor(\"K[10]\"))",
    "ast": { "MatMul": [ {"TensorRef":"Q[10]"},
                         {"Transpose": {"TensorRef":"K[10]"}} ] } },
  { "input": "show eval(\"x\")", "error": "unknown_function" },
  { "input": "show tensor(\"Q[10]\"", "error": "unexpected_eof", "caret": 21 } ]
```

**The chat module has no `fetch` to any byte route.** This is asserted by a test
that inspects its imports and its network calls — architecture enforced by a
test, not by review.

## Memory and Performance Constraints

Classification and mapping are local and synchronous; < 10 ms. Suggested
selectors come from a cached catalog query, refreshed on model change.

## Implementation Plan

1. `intent.ts`: classify by parse attempt in the stated order.
2. `mapper.ts`: rule-based mapping for the seven §21 forms.
3. Post the produced WeightQL to `/v1/query` with `mode: "plan"`.
4. `history.ts`: plan ID, cost, executed flag, re-runnable.
5. `context.ts`: show the current viewer/workspace selection persistently.
6. Write the corpus; wire it into both suites.
7. Add the no-byte-access test.

## Error Handling

* Text no parser accepts and no rule maps → say so and **suggest the closest
  selector**; never invent a query.
* A request needing an unbuilt feature → report the requirement ID. The §21
  example *"show the L2 norm of every query projection"* needs `WQL-007`; before
  `QM-0072` lands, the honest answer names it.
* An ambiguous alias → the 409 candidates surface (`QM-0075` renders them).
* A daemon error → shown with its status; nothing is retried automatically.

## Acceptance Criteria

1. All seven §21 examples produce the expected WeightQL.
2. Classification order is canonical → alias → WeightQL → natural language.
3. **The chat module makes no request to `value`, `blocks`, or any byte route** —
   asserted structurally.
4. Every produced query goes through `/v1/query` with `mode: "plan"` first.
5. Nothing executes without an explicit act.
6. History records plan ID, cost, and execution state.
7. Suggested selectors come from the catalog.
8. Unmappable text says so and suggests alternatives.
9. **Both parsers agree on the full conformance corpus.**
10. A request needing an unbuilt feature names its requirement ID.

## Verification Plan

**Automated** — `chat.test.ts` for all seven examples and classification; the
corpus in both suites; the no-byte-access assertion.
**Manual** — type each §21 example and read the produced WeightQL.

## Suggested Commands

```bash
cd apps/web && npx vitest run chat                            # introduced here
cargo test -p q-weightql --test conformance_corpus             # introduced here
```

## Test Cases

| Input | Expected WeightQL |
| --- | --- |
| `Show Q[10].` | `show tensor("Q[10]")` |
| `Show layer[10].attention.Q.` | Contextual resolved → `show tensor("Q[10]")` |
| `Open model.layers[10]….weight.` | `show tensor("model.layers[10]…")` |
| `Show Q[10][100, :].` | `SELECT slice FROM tensor("Q[10]") ROWS 100:101` |
| `Compare Q[10][100,:] with Q[20][100,:].` | `show compare(…) by cosine_similarity` |
| `Visualize Q[10][0:128,:] @ K[10][:,0:128].` | The matmul expression |
| `Show the L2 norm of every query projection.` | The `GROUP BY` query, or `WQL-007` named |
| Chat's network calls | **No byte route** |
| Corpus, both languages | Identical ASTs and error classes |
| Unmappable text | Says so; suggests |

## Risks

| Risk | Mitigation |
| --- | --- |
| Chat acquires byte access by convenience | Structurally asserted, not reviewed |
| Rule-based mapping is brittle | Seven examples are the specification; anything else is honestly reported as unmapped |
| The two parsers drift | The shared corpus, run by both suites |

## Completion Evidence

* Produced WeightQL for all seven examples.
* The no-byte-access test output.
* Corpus results from both suites.
* A history screenshot showing plan IDs and costs.
