# QM-0011 — Cross-family resolver conformance suite

## Status

Complete

`QM-0010` is `Complete` and merged, so the block is lifted.

**In v1.** `QM-0010`'s Qwen resolver is v1 work — v1's target checkpoints are Qwen-family — so the conformance suite that guards it is too. Lane T, Wave 2.

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

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | `Awaiting Independent Review` |
| Lane | T |
| Wave | 2 |
| Branch | `task/qm-0011-resolver-conformance` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0011` |
| Base commit | **`e49ac24`** — the dispatch named `e82fe98`, and `git reflog` records `e49ac24 HEAD@{2026-08-05 17:03:09}: reset: moving to main`, a reset this agent did not run. See the note below |
| Head commit | the single commit on this branch, subject `test(q-nsir): add resolver conformance suite [QM-0011]`. Authoritative: `git rev-parse task/qm-0011-resolver-conformance`. A commit cannot contain its own hash, which is why no SHA is written here |
| Implementation agent | `impl-agent-16` |
| Evidence record | `.plan/evidence/QM-0011.md` |
| Merge path | L |

**Tests added:** 29 — 28 in the new binary `crates/q-nsir/tests/resolver_conformance.rs`
and 1 in `crates/q-nsir/src/resolver.rs` (test module only; no production line
changed). Corpus: `architectures/conformance.json`, 120 rows over 5 families,
60 positive and 60 negative (generic 12/15, llama 24/18, qwen 24/19, kimi 0/4,
deepseek 0/4). Every one of the 29 was observed failing under a deliberate
break before it passed; the 21-mutation battery and its verbatim red output are
in `.plan/evidence/QM-0011.md` §Validation evidence.

**Base moved mid-task, by something outside this agent.** The dispatch named
`e82fe98`; `git reflog` records `e49ac24 HEAD@{2026-08-05 17:03:09}: reset:
moving to main` about six minutes after work began, so `HEAD^` is `e49ac24`.
Checked rather than assumed harmless: `git diff e82fe98 e49ac24 --stat` is
`.plan/tasks/QM-0121-…/TASK.md` and `CLAUDE.md` only;
`git diff e82fe98 main --name-only -- crates/ architectures/ fixtures/ scripts/ apps/`
is **empty**; and `scripts/baseline.json` reads 677/51/115/13 at `e82fe98`,
`e49ac24` and `main` (`1ea382d`) alike. No rebase was performed. Detail in
`.plan/evidence/QM-0011.md` §Task.

**Floor before → after:** rust 677 over 51 binaries → **706 over 52**;
web 115 over 13 → unchanged (this branch changes no web file). 677 + 29 = 706
reconciles exactly. `scripts/baseline.json` is raised, never lowered. Two other
branches raise the same floor concurrently; the controller reconciles at merge.

**`./scripts/verify-baseline.sh` exits 1** on a pre-existing environment gap, not
on this branch: `three@^0.185.1` is declared at
`apps/web/quatricmorph-workspace/package.json:15` and installed nowhere on this
machine, so one vitest file fails to collect. The identical failure reproduces in
the untouched main checkout. Every Rust, fixture and CLI-golden check passes and
both Rust floors read "at floor". The web floor was **not** lowered. Detail in
`.plan/evidence/QM-0011.md` §Validation evidence.

**Controller action — `MVP-08`.** This file's §Requirements Covered names
`MVP-08`, but `.plan/REQUIREMENT_TRACEABILITY.md:184` maps `MVP-08` to
`QM-0010`, and no per-criterion text for it exists anywhere in the repository —
only the band `MVP-02`…`MVP-09` at `.plan/DEFINITION_OF_DONE.md:181`. This suite
does **not** independently witness `MVP-08`; it guards `QM-0010`'s work, which is
where the criterion is mapped. Neither document was edited — reconciling a plan
document is outside this task's boundary. Detail in `.plan/evidence/QM-0011.md`
§Research and §Claim limits.

**Reported, not fixed:** the unanchored `experts.` marker in
`crates/q-nsir/src/resolver.rs` (`.plan/PLAN_CHANGELOG.md`, 2026-08-05) is a
production defect outside this task's test-only boundary. It is characterized by
`resolver::tests::an_unanchored_expert_marker_files_a_plural_shared_experts_name_as_routed_today`,
which records the behaviour without endorsing it and must be **replaced** when
the marker is anchored.
