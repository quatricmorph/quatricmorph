# Phase 10 — Out-of-core proof on a real checkpoint

## Goal

```text
A real open-weight SafeTensors checkpoint, ≥ 24 GB on disk
→ indexed from headers alone
→ streamed end to end, block by block
→ peak resident bytes ≤ 1.25 × a configured ceiling of ≤ 2 GB
→ measured, not asserted
```

## Why this phase is first

The strategy document's central technical claim is out-of-core diagnosis of a
checkpoint that does not fit in a 24 GB-class GPU. **No test in this repository
currently exercises that claim on real data.** `CAT-006` indexes a synthetic
10¹²-parameter *manifest* in 35.7 MB peak — that proves metadata scale and is
silent about streaming bytes. The largest real artifact in the tree is
`models/distilbert-distilgpt2` at 339 MB.

Until this phase closes, every performance statement in the product is
unsupported. It is also the phase with the longest wall-clock lead time in the
entire plan, and none of it is compressible by working harder.

## Restating "out-of-core" for the hardware that exists

The development machine is an Apple M3 Pro with 36 GB unified memory and 21 GB
free disk. There is no discrete VRAM to overflow. The property is therefore
stated as something the code enforces and `/usr/bin/time -l` can measure:

> Peak resident bytes stays under a configured ceiling `C` while streaming a
> checkpoint `N ×` larger than `C`, with `N ≥ 100`.

`crates/q-source/src/budget.rs` already implements named, enforced budgets. This
phase points that mechanism at the streaming path and measures the result.

## Entry conditions

* `QM-0001` baseline verified (or running in parallel — `QM-0100` does not depend
  on it).
* At least 45 GB free disk. **Check before starting**; the machine had 21 GB at
  planning time and the checkpoint is the largest single artifact this repository
  will ever hold.

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0100` | Acquire and verify a real ≥ 24 GB checkpoint | Data | P | `SRC-020`, `V1-01`, `V1-02` |
| `QM-0101` | Bounded-residency proof, measured | Verification | P | `PERF-002`, `V1-03`…`V1-05` |
| `QM-0102` | Scaling benchmarks across three checkpoint sizes | Verification | P | `PERF-003` |

`QM-0030` (bounded streaming block reader) belongs to Phase 03 by numbering and
to this phase by function; it is the code `QM-0101` measures.

## Exit conditions — Gate G1

1. A checkpoint ≥ 24 GB is on disk, its source URL and revision hash recorded.
2. `q-cli inspect` lists its tensors having read < 0.1 % of its bytes.
3. A full streaming pass completes with measured peak RSS ≤ 1.25 × `C`, `C ≤ 2 GB`.
4. The same ceiling holds at three checkpoint sizes — the claim is structural.
5. The numbers are recorded in the task files, not in prose.

**If G1 fails, halt the engine lane.** Every downstream claim inherits from it.

## If the checkpoint will not fit

Fall back to the largest that does, record the actual size in
[`../../DEFINITION_OF_DONE.md`](../../DEFINITION_OF_DONE.md) §1, and state the
limitation in the report. Do **not** substitute the synthetic manifest and call
the claim proven — that is the one substitution this plan explicitly forbids.
