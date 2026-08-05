# `schemas/diagnostics/` — the manifest a diagnostic run emits

| File | `$id` | Describes |
| --- | --- | --- |
| `manifest.v1.json` | `…/diagnostics/manifest/v1` | The versioned JSON manifest of a quantisation-error diagnostic run (`REP-001`) |

The fifth schema alongside `nsir`, `qtile`, `weightql` and `visualization`, and
the same discipline: draft-07, an `$id` that carries an explicit version
(`SCHEMA-001`), a `description` on every field, and a reader that refuses an
unknown version rather than guessing (`SCHEMA-002`).

## Why it exists before its producers do

[`REPORT_ARCHITECTURE.md`](../../.plan/REPORT_ARCHITECTURE.md) §1: the manifest
is the **only** serialization the Markdown report (`REP-002`), the daemon routes
(`API-012`) and the heat-map surface derive from. Four consumers each inventing
a shape is the drift this file prevents, so it is written against the contracts
in [`DIAGNOSTIC_ARCHITECTURE.md`](../../.plan/DIAGNOSTIC_ARCHITECTURE.md) rather
than waiting for the engine that fills them in. A number that appears in the
report and not here is a bug.

## Schema-to-code map

| Schema | Rust | Asserted by |
| --- | --- | --- |
| `manifest.v1.json` | `q-report` — `Manifest`, `Run`, `Model`, `QuantConfigRecord`, `ErrorAggregate`, `TensorEntry`, `RankingEntry`, `Frontier`, `Refusal` | `crates/q-report/tests/schema_conformance.rs` |

The schema is the contract and the Rust types mirror it. Two hand-written
descriptions of one shape will drift, so the test suite asserts rather than
trusts: a produced manifest is validated against this file, the `role` and
`dtype` enumerations are compared against `q_source::TensorRole::as_str` and
`q_source::DType::as_safetensors_str`, and `shape.maxItems` is compared against
`q_report::MAX_IMPLEMENTED_RANK`. Change one side and a test turns red.

## Departures from `REPORT_ARCHITECTURE.md` §2's sketch

§2 gives an illustrative `jsonc` outline. Where this schema differs, it differs
deliberately:

| §2 sketch | This schema | Why |
| --- | --- | --- |
| `"frontier": [ { … } ]` | `"frontier": { "method", "claim", "steps": [ … ] }` | `QM-0140`'s data-contract table requires a `frontier.method` field, which an array cannot carry. `steps` keeps the ordering rule ("frontier by cumulative `added_bytes`"), and `claim` is a `const` so that no consumer can render a frontier without the sentence saying it is greedy. |
| `"granularity": { "per_group": 128 }` | `"granularity": { "kind": "per_group", "group_size": 128 }` | A single-key form spells `per_tensor` and `per_output_channel` as bare strings and `per_group` as an object, which needs a `oneOf` over two shapes on the schema side and a mixed enum representation on the serde side. One shape with an `if`/`then` is simpler on both. |
| `"fidelity": "exact" \| "sampled"` | `"exact" \| "sampled" \| "approximate"` | The vocabulary `q_source::ResultFidelity` already types end to end, and `AGENTS.md` rule 4 names all three. |
| *(no discriminator)* | `"projection": "full" \| "summary"` | Without it, a summary projection and a full run that examined nothing are the same document. Those are different facts — the second is valid and `refusals` explains it — and a consumer that cannot tell them apart will report one as the other. |
| `"tensors"` unconditional | required iff `projection` is `full`, forbidden otherwise | The `--summary` projection exists from day one, because at `CAT-006`'s 47 278 tensors the per-tensor array is tens of megabytes and `ARCHITECTURE.md` §19 forbids pushing that into a browser wholesale. |
| `"totals": { SumsOfSquares + derived metrics }` | partials only; `relative_error` appears in `ranking` alone | Sums of squares compose across blocks and finished metrics do not (`DIAGNOSTIC_ARCHITECTURE.md` §4.1). Storing `relative_error` at every level as well would give one quantity two definitions, and two definitions drift. `ranking` carries it because it *is* the ordering key. |

`run.backend` is `["cpu", "metal"]` and cannot express `cuda`: `q-cuda` is
`Hardware-Unverified` (`CUDA-001`), and a document that can name a backend which
has never run is how the `PRODUCT_SCOPE.md` §5.2 forbidden claim gets made by
accident.

## Ordering

Every array has a total order fixed by content, never by iteration order. This
is what makes byte-identical output across two runs (`V1-18`) achievable.

| Array | Order | Uniqueness |
| --- | --- | --- |
| `layers` | `layer_index` | `layer_index` |
| `experts` | `(layer_index, expert_index)` | the pair |
| `tensors` | canonical address | address (`SRC-006`) |
| `ranking` | `(relative_error ↓, parameter_count ↓, address ↑)` | address |
| `frontier.steps` | cumulative `added_bytes`, then `error_removed_fraction`, then `keep_set` | the whole step |
| `frontier.steps[].keep_set` | ascending address | address |
| `refusals` | `(requirement_id, what, why)` | the triple |

The secondary keys on `frontier.steps` exist only so the order is *total*: sorting
by `added_bytes` alone leaves two steps of equal cost in an undefined order.
`q_report::Manifest::to_json_string` **imposes** this order rather than demanding
it of the producer, and refuses the duplicates that would leave any key
ambiguous.

## Floating point

`serde_json` renders `f64` with the shortest decimal that round-trips (Ryū):
enough digits to recover the exact value, the same digits on every platform, and
the same digits every time. `0.1 + 0.2` therefore appears as
`0.30000000000000004`, not `0.3`.

NaN and ±Infinity have no JSON representation. `serde_json` writes `null` for
them, and a `null` where a measured number belongs reads as an absence rather
than a failure — so they are refused *before* serialization, naming the field.

## Unknown members

Two rules, and neither of them drops data:

* An unrecognised **top-level** member is preserved verbatim by
  `q_report::Manifest` (in `extensions`) and written back out unchanged, so a
  newer producer's addition survives a read-modify-write by an older build.
* An unrecognised member **inside** `run`, `model`, `config` or any array element
  is refused, naming it. Those objects are closed here
  (`additionalProperties: false`), so ignoring a member there would hide a
  producer bug.

The two coexist on purpose: an extension survives the round trip, and validating
against this file still tells you the document is not v1.

## Rank

`tensor_entry.shape` has `maxItems: 3`. ADR-010 implements rank ≤ 3 and
**refuses above it rather than flattening**: a `[32,128,128]` tensor presented as
`[32,16384]` is a confidently wrong picture that invites the reader to see
adjacency between values that are not adjacent. A rank-4 tensor is recorded in
`refusals` under `GRID-007`; it never appears in `tensors` reshaped. The `examples`
entry in the schema and the golden manifest under
`crates/q-report/tests/golden/` both show the shape of that refusal.

## Versioning

`manifest_version` is `const: 1`. Per
[`SCHEMA_PLAN.md`](../../.plan/SCHEMA_PLAN.md) §5 a breaking change writes an
ADR, mints `manifest.v2.json` with an `$id` ending `/v2`, and leaves this file
readable — consumers pin. A reader refuses an unknown version naming both the
version it found and the version it supports; it does not attempt to upgrade a
document in place.

## Validating a manifest

```bash
cargo test -p q-report                    # schema conformance, round trip, refusals

# Optional external corroboration; not a build or test dependency. Run from a
# throwaway virtualenv — nothing here may become something `cargo test` needs.
python3 -m venv /tmp/qm-schema && /tmp/qm-schema/bin/pip install jsonschema
/tmp/qm-schema/bin/python -m jsonschema \
    -i runs/qwen-int4/manifest.json schemas/diagnostics/manifest.v1.json
```

The in-repo check is a validator for the draft-07 subset this schema uses, in
`crates/q-report/tests/schema_conformance.rs`. It **refuses any schema
containing a keyword it does not assert**, so the schema cannot quietly grow a
constraint that nothing checks. An external validator is the belt to that
braces and is recorded as evidence rather than wired into the build — a URL or a
`pip install` must never become something `cargo test` depends on.
