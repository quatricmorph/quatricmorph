# QM-0140 — Manifest schema v1 and serialization

## Status

Ready

Depends only on the *shape* of the engine's output, not its data. Scheduled in
Wave 1 deliberately: it is the contract between the engine, the CLI, the daemon,
and the surface, and writing it late means writing it four times.

## Phase

Phase 12 — Report, manifest, and the machine interface

## Objective

Define and implement the versioned JSON manifest that every consumer of a
diagnostic run reads — the single serialization from which the report, the API,
and the heat-map all derive.

## Repository Evidence

* `schemas/` — four existing JSON schemas (`nsir`, `qtile`, `weightql`,
  `visualization`); this adds a fifth alongside them.
* `crates/q-catalog/src/schema.rs` — `a_future_schema_is_refused_rather_than_corrupted`
  (`CAT-002`): the version-refusal idiom this follows.
* `crates/q-tiles/src/lib.rs` — `TILE-005`…`TILE-008`: a versioned binary format
  with corruption rejection; the same discipline in JSON.
* `crates/q-source/src/lib.rs` — `SRC-018`, exactness as a type; the `fidelity`
  field carries it into the manifest.

## Requirements Covered

`REP-001`, `V1-16`.

## Dependencies

None. (Consumes `QM-0123`'s types when they land; the schema may be written
against the contracts in `.plan/DIAGNOSTIC_ARCHITECTURE.md` first.)

## Blocks

`QM-0141`, `QM-0143`, `QM-0150`, `QM-0152`.

## Parallelization

Lane R, alone — four consumers depend on it, so it lands before any of them.

## Program Boundary

`crates/q-report` (new), `schemas/diagnostics/`.

## Scope

* `schemas/diagnostics/manifest.v1.json`.
* Serde types mirroring it exactly, in `q-report`.
* Version refusal: a future `manifest_version` is rejected, not partially parsed.
* Schema validation in tests.
* A total, content-defined ordering for every array.

## Out of Scope

Markdown rendering (`QM-0141`) · the daemon routes (`QM-0143`) · the surface
(`QM-0150`) · persistence in the catalog (`QM-0020`).

## Files Expected to Change

* `Cargo.toml` — workspace member

## Files Expected to Add

* `crates/q-report/Cargo.toml`
* `crates/q-report/src/lib.rs`
* `crates/q-report/src/manifest.rs`
* `schemas/diagnostics/manifest.v1.json`
* `schemas/diagnostics/README.md`

## Data Contracts

Per [`REPORT_ARCHITECTURE.md`](../../REPORT_ARCHITECTURE.md) §2. The parts that
are contract rather than convenience:

| Field | Why it is required |
| --- | --- |
| `manifest_version` | Refused if greater than known — `CAT-002`'s rule |
| `run.backend` | Claiming GPU computation the CPU performed is forbidden (`PRODUCT_SCOPE.md` §5.2) |
| `run.peak_resident_bytes` | The product's central claim, measured, in the artifact |
| `model.revision_hash` | A diagnosis of an unidentified checkpoint is not evidence |
| `model.resolver_confidence` | A `generic`-resolved hierarchy must not read as a known one (`NSIR-001`) |
| `fidelity` | `exact` vs. `sampled`, carried end to end |
| `refusals[]` | Distinguishes "zero" from "not computed" — the failure mode that destroys trust |
| `frontier.method` | Carries "greedy, not proven optimal" into every consumer |

### Ordering

Every array has a total order fixed by content, never by iteration:

```text
layers   by layer_index
experts  by (layer_index, expert_index)
tensors  by canonical address
ranking  by (relative_error desc, parameter_count desc, address asc)
frontier by cumulative added_bytes
refusals by requirement_id
```

Floating-point values serialise with a fixed representation — enough digits to
round-trip f64, and the same digits every time. This is what makes `V1-18`
achievable at all.

## Memory and Performance Constraints

The manifest is `O(tensors)`. At `CAT-006`'s 47 278 tensors, a full per-tensor
manifest is on the order of tens of MB — acceptable on disk, **not** acceptable to
push into a browser wholesale.

The schema therefore supports a `--summary` projection: totals, layers, experts,
ranking, and frontier, without the per-tensor array. `QM-0150` loads the summary
by default and fetches per-tensor detail on demand. That mirrors the same
discipline `assertBlockIsBounded` enforces elsewhere: never send the whole thing
to the browser.

## Implementation Plan

1. Write the JSON schema first, with descriptions on every field.
2. Mirror it in serde types; a test asserts a produced manifest validates.
3. Version refusal: unknown-greater is an error naming both versions; unknown
   fields within a known version are preserved, not dropped, on round trip.
4. Fixed float formatting and the ordering rules.
5. The `--summary` projection as a distinct, also-validated document.
6. Round-trip tests: serialise → deserialise → serialise, byte-identical.

## Error Handling

| Case | Behaviour |
| --- | --- |
| `manifest_version` greater than known | Refuse, naming both versions |
| Missing required field | Refuse, naming it — never default silently |
| NaN or Infinity in a numeric field | Refuse at serialization; JSON has no representation and a `null` would be a lie |
| Duplicate canonical address | Refuse; addresses are unique (`SRC-006`) |
| Empty run (no tensors examined) | Valid, with `refusals` explaining why |

## Acceptance Criteria

1. A produced manifest validates against the schema.
2. Round trip is byte-identical.
3. A future version is refused, naming both versions.
4. Array ordering is deterministic and content-defined; two runs match.
5. Float formatting round-trips f64 exactly and identically across runs.
6. `refusals[]` is present and populated for a run with an unimplemented request
   (e.g. an accuracy estimate).
7. The `--summary` projection validates and omits `tensors`.
8. NaN and Infinity are refused rather than serialised.

## Verification Plan

**Automated** — schema validation, round trip, version refusal, ordering,
formatting.
**Manual** — read the schema descriptions as a stranger would; a field nobody can
explain does not belong in v1.

## Suggested Commands

```bash
cargo test -p q-report
python3 -m jsonschema -i /tmp/manifest.json schemas/diagnostics/manifest.v1.json
```

## Test Cases

| Input | Expected |
| --- | --- |
| A full manifest | Validates |
| Round trip | Byte-identical |
| `manifest_version: 2` | Refused, both versions named |
| Missing `run.backend` | Refused, field named |
| `relative_error: NaN` | Refused at serialization |
| Two runs, same data | Identical bytes |
| Summary projection | Validates; no `tensors` array |
| A run with an `EVAL-001` refusal | `refusals[]` contains it with its ID |

## Risks

| Risk | Mitigation |
| --- | --- |
| The schema drifts from the Rust types | A test validates produced output against the schema on every run |
| Float formatting varies by platform | Fixed representation; a cross-platform check in CI if available, otherwise documented |
| The per-tensor array is pushed into the browser | The summary projection exists from day one, and `QM-0150` uses it by default |
| `refusals` is treated as optional and omitted | It is a required field with a required-array constraint |

## Completion Evidence

* The schema file.
* Validation output for a produced manifest.
* Round-trip byte comparison.
* Version-refusal error text.
* A summary projection alongside its full manifest.
