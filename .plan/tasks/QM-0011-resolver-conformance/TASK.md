# QM-0011 — Cross-family resolver conformance suite

## Status

Blocked

Unblocks when `QM-0010` reaches `Complete`.

## Phase

Phase 01 — SafeTensors ingestion completion

## Objective

One table-driven suite asserting that every architecture plugin resolves what it
claims and returns `unknown` for everything else.

## Repository Evidence

* `crates/q-nsir/src/resolver.rs` — per-family tests today, not a shared corpus.
* `crates/q-architecture/src/lib.rs` — `unimplemented_plugins_are_declared_and_never_claim`.
* `architectures/{generic,llama,qwen,kimi,deepseek}/plugin.toml`.
* `NSIR-001` — `generic_resolver_returns_unknown_for_names_it_was_not_taught`.

## Requirements Covered

`NSIR-001`, `NSIR-006`, `NSIR-008`, `MVP-08`.

## Dependencies

`QM-0010`.

## Blocks

None.

## Parallelization

Parallel with `QM-0012`, `QM-0013`. Test-only.

## Program Boundary

`crates/q-nsir`, `crates/q-architecture` — tests only.

## Scope

* A corpus at `architectures/conformance.json`: name → expected canonical address
  or `unknown`, per family.
* Assert: correct resolution; `unknown` for untaught names; **no shape is ever an
  input**; unimplemented plugins never claim; priority ordering; generic fallback.
* Include adversarial names: near-misses, wrong layer syntax, extra segments.

## Out of Scope

New resolvers · changing resolution behaviour · Kimi/DeepSeek implementation.

## Files Expected to Change

* `crates/q-nsir/src/resolver.rs` — test module only.

## Files Expected to Add

* `architectures/conformance.json`
* `crates/q-nsir/tests/resolver_conformance.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "llama": [ { "raw": "model.layers.10.self_attn.q_proj.weight",
               "canonical": "model.layers[10].self_attention.query_projection.weight",
               "role": "attention_query_projection", "layer": 10 },
             { "raw": "model.layers.10.self_attn.q_proj_v2.weight", "canonical": null } ],
  "qwen":  [ … ],
  "generic": [ { "raw": "some.unknown.tensor", "canonical": null } ] }
```

`canonical: null` means **must resolve to `unknown`** — the negative cases are
first-class rows, not an afterthought.

## Memory and Performance Constraints

Table-driven; sub-second.

## Implementation Plan

1. Extract existing per-family test cases into the corpus.
2. Add ≥ 10 negative cases per family, including near-misses.
3. Add a shape-independence test: two identical-shape tensors with different
   names must get different roles, and one untaught name must stay `unknown`
   regardless of its shape.
4. Add registry selection cases per family.
5. Assert the corpus covers every pattern each `plugin.toml` declares — a
   declared pattern with no test row fails the suite.

## Error Handling

* An uncovered declared pattern → suite failure naming the pattern.
* A corpus row referencing a non-existent family → failure naming it.

## Acceptance Criteria

1. Every family's declared patterns have ≥ 1 corpus row.
2. ≥ 10 negative rows per implemented family, all resolving to `unknown`.
3. Shape independence asserted explicitly.
4. Kimi and DeepSeek claim nothing.
5. Priority and generic fallback asserted.
6. Removing a `plugin.toml` pattern makes the suite fail.

## Verification Plan

**Automated** — `cargo test -p q-nsir --test resolver_conformance`.
**Manual** — delete one Qwen pattern; confirm the suite names it.

## Suggested Commands

```bash
cargo test -p q-nsir --test resolver_conformance      # introduced here
cargo test -p q-architecture                          # verified today
```

## Test Cases

| Input | Expected |
| --- | --- |
| Every corpus row | Matches its expected canonical or `unknown` |
| `model.layers.10.self_attn.q_proj_v2.weight` | `unknown` |
| `model.layers.abc.self_attn.q_proj.weight` | `unknown` (bad layer syntax) |
| Two `[4096,4096]` tensors, names `q_proj` and `o_proj` | Different roles |
| An untaught `[4096,4096]` name | `unknown`, despite the familiar shape |
| A `plugin.toml` pattern with no corpus row | Suite fails naming it |

## Risks

| Risk | Mitigation |
| --- | --- |
| The corpus is written from the implementation, so it tests nothing | Rows are derived from public naming conventions and the fixtures, then checked |
| Corpus grows unmaintainable | One file, one shape, additive |

## Completion Evidence

* Suite output with the row count.
* The deliberate-removal demonstration.
* Negative-case count per family.
