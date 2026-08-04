# QM-0012 — Model-level metadata from `config.json`

## Status

Ready

No longer waits on `QM-0005` (deferred). Model-level metadata feeds the manifest's `model` block.

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 01 — SafeTensors ingestion completion

## Objective

Populate `models.hidden_size`, `layer_count`, and `parameter_count` from
`config.json` and the tensor manifest, so LOD 0 and LOD 1 tiles have something
to summarize.

## Repository Evidence

* `crates/q-catalog/src/schema.rs:35` — `models(model_id, source_uri,
  source_revision, source_hash, architecture, parameter_count, layer_count,
  hidden_size, imported_at)`. The columns exist.
* `fixtures/tiny-llama-2shard/config.json` — `hidden_size: 48`,
  `num_hidden_layers: 12`, `vocab_size: 64`, `torch_dtype: "float32"`.
* `crates/q-architecture/src/lib.rs` already reads `config.json` for
  `model_type` and `architectures` when selecting a plugin.
* `ARCHITECTURE.md` §9.2 — LOD 0 carries *"parameter count, bytes, global
  distributions"*.

## Requirements Covered

`NSIR-010`, `CAT-011`; enables `MVP-06`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0020`, `QM-0021`, `QM-0040`.

## Parallelization

Parallel with `QM-0010`, `QM-0011`, `QM-0013`. Touches `q-catalog` and
`q-architecture`.

## Program Boundary

`crates/q-architecture` (config parsing), `crates/q-catalog` (persistence),
`crates/q-safetensors` (ingestion wiring).

## Scope

* Parse `hidden_size`, `num_hidden_layers`, `intermediate_size`,
  `num_attention_heads`, `num_key_value_heads`, `vocab_size`, `torch_dtype`.
* Compute `parameter_count` by **summing tensor element counts from the
  manifest** — not from config arithmetic, which would be an estimate.
* Compute `total_bytes` likewise.
* Persist; expose through `GET /v1/models/{id}` and `q-cli inspect`.
* Handle a missing or partial `config.json` without failing ingestion.

## Out of Scope

Tokenizer parsing · derived architecture statistics · anything requiring a weight
read.

## Files Expected to Change

* `crates/q-architecture/src/lib.rs`
* `crates/q-catalog/src/lib.rs`
* `crates/q-safetensors/src/ingest.rs`
* `crates/q-daemon/src/lib.rs`
* `crates/q-cli/src/main.rs`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
// GET /v1/models/{id}
{ "model_id": "…", "architecture": "llama", "source_uri": "…",
  "hidden_size": 48, "layer_count": 12,
  "parameter_count": 299184, "total_bytes": 1196736,
  "tensor_count": 111, "shard_count": 2,
  "fidelity": "metadata" }
```

`fidelity: "metadata"` is mandatory — **no weight byte was read to produce any of
this.**

## Memory and Performance Constraints

`parameter_count` is a sum over descriptors already in memory. **Nothing may be
read from a shard payload.** `SRC-007` and `SRC-018` must keep passing.

## Implementation Plan

1. Extend the config struct in `q-architecture` with the fields, all `Option`.
2. During ingest, after descriptors are built, fold element counts and byte
   lengths.
3. Write the columns; extend the model row struct.
4. Surface in the daemon route and the CLI.
5. Test against the fixture's known values.

## Error Handling

* Missing `config.json` → ingestion proceeds; the fields are `NULL`; the API
  reports `null`, never `0`. **Zero would be a lie.**
* Malformed JSON → refuse with context, matching `corrupt_json_is_rejected_with_context`.
* A field of the wrong type → that field is `NULL`; the rest still load.
* `parameter_count` overflowing `u64` → impossible at 10¹² (≈2⁴⁰), but checked.

## Acceptance Criteria

1. `hidden_size = 48`, `layer_count = 12` for `tiny-llama-2shard`.
2. `parameter_count` equals the sum over `golden.json`'s tensor shapes.
3. `total_bytes = 1 196 736`, matching `golden.json`.
4. A model with no `config.json` ingests, with `NULL` fields.
5. `GET /v1/models/{id}` includes the fields and `fidelity: "metadata"`.
6. `q-cli inspect` prints them.
7. `ingestion_reads_only_headers_not_payload` still passes.

## Verification Plan

**Automated** — catalog and daemon tests against fixture values.
**Manual** — `q-cli inspect` output compared against `config.json` by eye.

## Suggested Commands

```bash
cargo test -p q-catalog -p q-architecture -p q-daemon   # verified today
cargo run -p q-cli -- inspect fixtures/tiny-llama-2shard
curl -s localhost:PORT/v1/models | jq                    # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| `tiny-llama-2shard` | `hidden_size 48`, `layer_count 12`, `total_bytes 1196736` |
| `parameter_count` | Sum of `golden.json` shapes, exactly |
| `config.json` deleted | Ingests; fields `NULL`, **not `0`** |
| `config.json` with `hidden_size: "big"` | That field `NULL`; others load |
| Corrupt `config.json` | Refused with context |
| Ingest with the metadata budget | No payload byte read |

## Risks

| Risk | Mitigation |
| --- | --- |
| `parameter_count` computed from config instead of the manifest | Sum descriptors; the test compares against `golden.json`, which config arithmetic would not match |
| `NULL` rendered as `0` in the UI | The API returns `null`; a test asserts it |

## Completion Evidence

* Test output with fixture values.
* `q-cli inspect` output.
* `curl` output showing the fields and the fidelity label.
