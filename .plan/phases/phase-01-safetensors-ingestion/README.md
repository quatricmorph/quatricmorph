# Phase 01 — SafeTensors ingestion completion

## Goal

```text
Open one SafeTensors file → parse metadata → read one exact tensor slice
```

**Already achieved.** 17 of 18 `SRC-*` requirements are `Verified`, and
`cargo run -p q-cli -- value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42`
returns `0.006408154033124447` — 4 bytes read from a 1.2 MB checkpoint, matching
the Python `safetensors` reference.

This phase closes the **three remaining gaps** rather than rebuilding what works.

## Entry conditions

* Phase 00 complete; gate **G1** passed.
* `QM-0003`'s larger fixture available.

## What is already done — no task needed

| Capability | Requirements |
| --- | --- |
| Single-file and sharded ingestion | `SRC-003`, `SRC-004` |
| Memory-mapped byte-range reads | `SRC-005` |
| Stable IDs across reopen | `SRC-006` |
| Nothing allocated proportional to checkpoint size | `SRC-007` |
| Cancellation and resume | `SRC-009`, `SRC-010` |
| Missing shard, duplicate name, corrupt header, unknown dtype, invalid offset — all refused | `SRC-011`…`SRC-015` |
| Exact f32 / bf16 / f16 decoding including subnormals | `SRC-016` |
| Named, enforced budgets; access scale as a type | `SRC-017`, `SRC-018` |
| Generic and Llama resolvers; MoE addressing; canonical addresses; alias grammar; ambiguity → candidates | `NSIR-001`…`NSIR-005`, `NSIR-007`…`NSIR-009` |

Regression is guarded by `QM-0001`.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0010` | Qwen-family architecture resolver | Implementation | `NSIR-006` (Qwen), `MVP-08` |
| `QM-0011` | Cross-family resolver conformance suite | Verification | `NSIR-001`, `NSIR-006`, `NSIR-008` |
| `QM-0012` | Model-level metadata from `config.json` | Implementation | `NSIR-010`, `CAT-011` |
| `QM-0013` | Trillion-scale manifest generator as a tool | Verification | `CAT-006`, `MVP-05` |

## Scope boundary

**Qwen only.** `architectures/{kimi,deepseek}/plugin.toml` stay declared with
`implemented = false`, and
`q_architecture::tests::unimplemented_plugins_are_declared_and_never_claim`
keeps asserting they never claim a model. The task specification §8 lists generic,
Llama-like, and Qwen-like; additional families are out of scope unless a fixture
requires them, and none does.

`SRC-008` (HTTP Range transport) stays an extension point — the range arithmetic
is verified, the transport is not built, and the MVP reads local checkpoints.

## Exit conditions

1. A Qwen-family checkpoint's tensor names resolve to canonical addresses, and
   names the resolver was not taught still return `unknown`.
2. `models.hidden_size`, `layer_count`, and `parameter_count` are populated from
   `config.json` and asserted against the fixture's known values.
3. The synthetic trillion-scale manifest generator is a reusable tool, and the
   bounded-memory assertion is expressed against a **named budget** rather than a
   literal — so an intentional budget change does not silently pass.
4. No `SRC-*` or `NSIR-*` requirement regresses.

## Parallelization

All four are independent. `QM-0010` and `QM-0011` touch `crates/q-nsir` and
`architectures/`; `QM-0012` touches `q-catalog` and `q-architecture`; `QM-0013`
touches a test and a new tool. `QM-0011` should merge after `QM-0010`.

## Risks

| Risk | Mitigation |
| --- | --- |
| A Qwen resolver over-claims, guessing roles from shape | `QM-0011` asserts the negative case explicitly: names it was not taught return `unknown` |
| No Qwen fixture exists | `QM-0010` adds a small generated Qwen-shaped fixture; the naming, not the weights, is what is under test |
