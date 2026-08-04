# QM-0010 — Qwen-family architecture resolver

## Status

Ready

No longer waits on `QM-0005` (deferred). v1's target checkpoints are Qwen-family, so the resolver is needed early.

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 01 — SafeTensors ingestion completion

## Objective

Implement the Qwen-family resolver so Qwen tensor names map to canonical
addresses, **without** the resolver ever guessing a role it was not taught.

## Repository Evidence

* `architectures/qwen/plugin.toml` — declared with `implemented = false`.
* `architectures/llama/plugin.toml` — the working reference;
  `resolver::tests::llama_resolves_the_architecture_md_example` and
  `llama_resolves_moe_expert_tensors` pass.
* `crates/q-nsir/src/resolver.rs` (632 lines) — the resolution engine.
* `crates/q-architecture/src/lib.rs` (412) — the plugin registry;
  `llama_is_selected_by_model_type_and_by_architecture`,
  `unknown_model_falls_back_to_generic`,
  `unimplemented_plugins_are_declared_and_never_claim`.
* `STATUS.md` `NSIR-006` — **Not Started**, covering Qwen, Kimi, and DeepSeek.
* `NSIR-001` — `generic_resolver_returns_unknown_for_names_it_was_not_taught`.

## Requirements Covered

`NSIR-006` (Qwen only), `MVP-08`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0011`.

## Parallelization

Parallel with `QM-0012`, `QM-0013`. Touches `architectures/qwen/` and
`crates/q-nsir/src/resolver.rs`.

## Program Boundary

`crates/q-nsir`, `crates/q-architecture`, `architectures/qwen/`.

## Scope

* Fill in `architectures/qwen/plugin.toml` with Qwen2/Qwen3 naming patterns and
  set `implemented = true`.
* Cover: `q_proj`, `k_proj`, `v_proj`, `o_proj`, `gate_proj`, `up_proj`,
  `down_proj`, `input_layernorm`, `post_attention_layernorm`, `q_norm`, `k_norm`,
  `embed_tokens`, `lm_head`, `model.norm`, and MoE `experts.N.*` plus
  `mlp.gate.weight`.
* A small generated Qwen-shaped fixture — **naming is what is under test**, not
  weights.
* Registry selection by `model_type: "qwen2"` / `"qwen3"` and by
  `architectures: ["Qwen2ForCausalLM", …]`.

## Out of Scope

**Kimi and DeepSeek resolvers** — out of scope per the task specification §8 and
`PRODUCT_SCOPE.md`; their plugins stay `implemented = false`. Vision or audio
towers. Quantized checkpoint formats.

## Files Expected to Change

* `architectures/qwen/plugin.toml`
* `crates/q-nsir/src/resolver.rs` — only if Qwen needs a pattern form Llama does
  not have (e.g. `q_norm`/`k_norm`, which Llama lacks)
* `fixtures/generate_fixtures.py`

## Files Expected to Add

* `fixtures/tiny-qwen-single/{config.json,golden.json}`

## Files Expected to Remove or Deprecate

None. Kimi and DeepSeek plugins **stay** as declared-but-unimplemented; deleting
them would remove the evidence that they are known and deliberately absent.

## Data Contracts

Canonical addresses match the Llama form so downstream consumers need no
knowledge of family:

```text
model.layers.10.self_attn.q_proj.weight
  → model.layers[10].self_attention.query_projection.weight

model.layers.10.mlp.experts.37.up_proj.weight
  → model.layers[10].moe.expert[37].up_projection.weight

model.layers.10.self_attn.q_norm.weight
  → model.layers[10].self_attention.query_norm.weight
```

`schemas/nsir/schema.json` governs the semantic record; **no schema change is
expected**, and needing one would indicate the abstraction is family-specific.

## Memory and Performance Constraints

Resolution is per tensor name, O(patterns). At 47 278 tensors it must stay well
under a second — a linear scan over ~20 patterns per name.

## Implementation Plan

1. Read `architectures/llama/plugin.toml` and mirror its structure.
2. Enumerate Qwen2/Qwen3 names from the fixture config and public naming.
3. Write the patterns; map each to `(component, operation, parameter, axes)`.
4. Add `q_norm`/`k_norm` role variants if `TensorRole` lacks them — an additive
   enum change.
5. Register `model_type` and `architectures` keys with a priority above generic.
6. Add the Qwen fixture.
7. Write tests, **including the negative case**.

## Error Handling

* An unrecognized name → `unknown`. **Never inferred from shape.**
* A name matching two patterns → the higher-priority one, with the ambiguity
  logged; genuinely ambiguous *aliases* still return candidates.
* A malformed `plugin.toml` → refused at registry load, naming the file.

## Acceptance Criteria

1. `qwen/plugin.toml` has `implemented = true` and resolves all 15 name families.
2. A Qwen checkpoint's tensors resolve to canonical addresses identical in form
   to Llama's.
3. **A name the resolver was not taught returns `unknown`** — asserted directly.
4. Registry selects Qwen by `model_type` and by `architectures`.
5. Kimi and DeepSeek still never claim a model
   (`unimplemented_plugins_are_declared_and_never_claim` passes).
6. MoE expert addressing works for Qwen's `experts.N` layout.
7. Canonical names are stable across resolution runs.
8. No `NSIR-*` requirement regresses.

## Verification Plan

**Automated** — new tests in `crates/q-nsir/src/resolver.rs` and
`crates/q-architecture/src/lib.rs`.
**Manual** — `cargo run -p q-cli -- inspect fixtures/tiny-qwen-single`, reviewed
by eye.

## Suggested Commands

Verified today:

```bash
cargo test -p q-nsir
cargo test -p q-architecture
cargo run -p q-cli -- inspect fixtures/tiny-llama-2shard
```

Introduced by this task:

```bash
cargo run -p q-cli -- inspect fixtures/tiny-qwen-single
.venv/bin/python fixtures/generate_fixtures.py --qwen
```

## Test Cases

| Input | Expected |
| --- | --- |
| `model.layers.10.self_attn.q_proj.weight` | `model.layers[10].self_attention.query_projection.weight` |
| `model.layers.10.self_attn.q_norm.weight` | `…self_attention.query_norm.weight` |
| `model.layers.10.mlp.experts.37.up_proj.weight` | `…moe.expert[37].up_projection.weight` |
| `model.layers.5.mlp.gate.weight` | `…moe.router.weight` |
| `model.layers.0.some_future_thing.weight` | **`unknown`** |
| `config.json` with `model_type: "qwen3"` | Qwen plugin selected |
| `config.json` with `model_type: "kimi"` | Generic fallback; Kimi does not claim |
| Two tensors of identical shape, different names | Different roles; **shape never used to infer** |

## Risks

| Risk | Mitigation |
| --- | --- |
| Qwen naming varies across releases | Cover Qwen2 and Qwen3; unknown names return `unknown`, which is safe |
| Over-claiming to raise coverage | The negative test is an acceptance criterion, not an afterthought |
| `TensorRole` enum growth ripples | The change is additive; exhaustive matches are compiler-checked |

## Completion Evidence

* New test output.
* `q-cli inspect` output on the Qwen fixture.
* The negative test asserting `unknown`.
* `cargo test --workspace` count, increased.
