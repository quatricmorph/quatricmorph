# EXECUTION_ORDER — v1

## 1. How to use this document

An agent picks the earliest `Ready` task on the critical path. If none is
`Ready`, it takes any `Ready` task from a parallel lane whose `Parallelization`
section does not name a file another in-progress task is editing
([`DEPENDENCY_GRAPH.md`](DEPENDENCY_GRAPH.md) §4).

**Read [`../STATUS.md`](../STATUS.md) first.** It, not this plan, is the record of
what is built.

**Never start a `Deferred` task.** Phases 04–07 are deferred wholesale for v1
(Cesium viewer, tile pyramid, matrix workspace, chat). They are correct work, in
the wrong release. See [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md).

## 2. Wave order

Tasks within a wave may run concurrently. A wave starts when its predecessor
completes, except where a gate says otherwise.

### Wave 0 — start the clock on the long lead times (4 tasks)

```text
immediately: QM-0100  acquire a real ≥24 GB checkpoint    ← hours of download; start FIRST
parallel:    QM-0001  baseline verification               (minutes)
             QM-0002  divergence register + path drift
             QM-0160  design-partner outreach             ← no code; runs for the whole plan
```

`QM-0100` is first in the entire plan because it is the only task whose lead time
is measured in hours of wall clock the agent cannot compress, and because
`QM-0101`, `QM-0122`, `QM-0125`, `QM-0161` and every performance number in the
release all depend on the artifact it produces.

`QM-0160` starts now and never finishes until v1 ships. The strategy's Days 0–30
instruction is partner conversations, not code, and this is where a solo founder
most reliably loses two months.

### Wave 1 — residency proof and the streaming spine (4 tasks)

```text
Lane P:  QM-0101  bounded-residency proof on the real checkpoint   ══ GATE G1
Lane P:  QM-0030  bounded streaming block reader
Lane T:  QM-0012  model metadata from config.json
Lane R:  QM-0140  manifest schema v1                (schema only — no data needed yet)
```

`QM-0140` is scheduled here, far ahead of the engine that fills it, because the
report schema is the contract between the engine, the CLI, the daemon, and the
surface. Writing it late means writing it four times.

### Wave 2 — the engine (7 tasks)

```text
Lane Q:  QM-0120 → QM-0121 → QM-0122                 ══ GATE G2 at QM-0122
Lane T:  QM-0020  persist statistics
Lane T:  QM-0010 → QM-0011  Qwen resolver + conformance   (v1 targets are Qwen-family)
Lane S:  QM-0150  heat-map surface against the schema, with synthetic data
```

Lane S runs against the `QM-0140` schema with synthetic input, so the surface is
built and reviewable before the engine produces anything. If the surface turns
out not to be what drives adoption, this is the cheapest possible moment to learn
it — see [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §5.

### Wave 3 — aggregation, jobs, decisions (6 tasks)

```text
Lane Q:  QM-0123 → QM-0125                           (aggregation → ranking + frontier)
Lane Q:  QM-0124                                     (outlier attribution; parallel to QM-0125)
Lane P:  QM-0033  job runner                         (checkpoint, atomic, resume, cancel)
Lane T:  QM-0032  wire the cache into the diagnostic path
Lane U:  QM-0126  Metal backend build                [Apple GPU — blocks nothing]
```

### Wave 4 — the artifact and the interface (6 tasks)

```text
Lane R:  QM-0141  deterministic Markdown report      ══ GATE G3
Lane R:  QM-0142  golden report + diff test
Lane R:  QM-0143  CLI exit codes and daemon routes
Lane S:  QM-0151  legibility review with real data   ══ GATE G4
Lane S:  QM-0152  surface reads a real manifest
Lane U:  QM-0127  Metal differential verification vs CPU
```

### Wave 5 — validation (8 tasks, mostly not code)

```text
QM-0161  design-partner run on a checkpoint the founder did not choose
QM-0162  documented decision-change case             ══ GATE G5
QM-0163  price probe
QM-0164  repeated-use log                            (runs over weeks, in the background)
parallel: QM-0102 scaling benchmarks
          QM-0081 cache/resume failure injection
          QM-0082 browser soak
          QM-0085 runtime error and security audit
```

`QM-0164` spans this wave and the next: repeated use cannot be observed inside a
single wave, and prompting a partner to generate the signal invalidates it.

### Wave 6 — release (6 tasks)

```text
parallel:  QM-0090  documentation update
           QM-0092  limitations
           QM-0093  attribution and licensing
           QM-0167  root-document amendment          (see Phase 14)
then:      QM-0091  regenerate STATUS.md
then:      QM-0165  v1 release audit                 ══ RELEASE
after:     QM-0166  technical write-up               (not a release blocker)
```

`QM-0166` follows the release rather than gating it. It is the outcome that
survives either result — including the one where `VALIDATION_PLAN.md` §4's kill
criteria fire.

## 3. Critical path

```text
QM-0100 → QM-0101 → QM-0030 → QM-0120 → QM-0121 → QM-0122 → QM-0123
        → QM-0125 → QM-0141 → QM-0150 → QM-0161 → QM-0162 → QM-0165
```

**Thirteen tasks. Not one requires an NVIDIA GPU, a renderer, or a chat
interface.**

`QM-0140` (manifest schema) and `QM-0033` (job runner) are **prerequisites, not
serial links**: `QM-0140` gates `QM-0141`, `QM-0143` and `QM-0150`, and `QM-0033`
gates cancellation and resume (`V1-06`). Both are scheduled early — `QM-0140` in
Wave 1, `QM-0033` in Wave 3 — and neither extends the path.

This string is canonical. [`MASTER_PLAN.md`](MASTER_PLAN.md) §7 states the same
sequence with its task titles; if the two ever disagree, `MASTER_PLAN.md` wins
and this file is corrected.

### Why the shared spatial contract is no longer first

The previous critical path began `QM-0001 → QM-0004 → QM-0005` — a shared spatial
contract and its conformance tests, so that the Rust tiler, the Cesium viewer and
the matrix workspace could not drift apart on grid parameters and the LOD ladder.
That was correct when three consumers existed. **v1 has none of them**: no tiler,
no Cesium viewer, no workspace. Building the contract now would be building a
schema for three deferred consumers, and it would be stale by the time they
arrive.

`QM-0004` and `QM-0005` are `Deferred`, not cancelled, and they remain the first
two tasks of the post-v1 visualization release.

### Why the Metal lane is off the path

`q_gpu::CpuBackend` is `GPU-002 Verified` and is the numerical reference for every
metric v1 computes (`DIAGNOSTIC_ARCHITECTURE.md` §4.3). A Metal backend makes the
headline run faster; it changes no output. If Metal slips, v1 ships on CPU with a
slower benchmark and an honest note. If Metal were on the path, a shader bug
would block a business milestone.

## 4. Lanes

| Lane | Tasks | Owns | Unblocked by |
| --- | --- | --- | --- |
| **P — proof** (critical) | `QM-0100`, `QM-0101`, `QM-0102`, `QM-0030`, `QM-0033` | `crates/q-source`, `q-tensor-runtime`, `fixtures/`, `models/` | — |
| **Q — engine** (critical) | `QM-0120`…`QM-0127` | `crates/q-quant`, `crates/q-diagnostics`, `crates/q-gpu` | `QM-0030` |
| **R — report** | `QM-0140`…`QM-0143` | `crates/q-report`, `q-cli`, `q-daemon` | schema: nothing; data: `QM-0123` |
| **S — surface** | `QM-0150`…`QM-0153` | `apps/web/diagnostics` | `QM-0140` schema |
| **T — catalog** | `QM-0010`, `QM-0011`, `QM-0012`, `QM-0020`, `QM-0031`, `QM-0032` | `crates/q-catalog` | sequential among themselves |
| **U — Metal** | `QM-0126`, `QM-0127` | `crates/q-gpu`, `gpu/metal/` | `QM-0121`. **Blocks nothing** |
| **V — validation** | `QM-0160`…`QM-0167` | no code | **nothing — starts on day 1** |

## 5. Runnable on the development machine

**Every v1 task.** The machine is an Apple M3 Pro, 36 GB unified memory, 21 GB
free disk. No task in Phases 10–14 requires NVIDIA hardware, and the Metal lane
targets the GPU that is present.

The binding constraint is **disk, not compute**: `QM-0100` must fit its checkpoint
in 21 GB, which caps v1's headline model at roughly 30–40 GB. That cap is
recorded as a waiver in [`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md), not
papered over.

## 6. Where concurrency is forbidden

| Sequence | Reason |
| --- | --- |
| `QM-0012` → `QM-0020` → `QM-0032` | All edit `crates/q-catalog/src/lib.rs` |
| `QM-0120` → `QM-0121` → `QM-0122` → `QM-0123` → `QM-0125` | Each consumes the previous stage's output |
| `QM-0140` before `QM-0141`, `QM-0143`, `QM-0150` | All four consume the manifest schema |
| `QM-0100` before anything measuring throughput | There is nothing to measure until it lands |
| `QM-0162` alone | It is the business gate, and it needs a person, not a lane |

## 7. If a gate fails

| Gate | Failure | Response |
| --- | --- | --- |
| **G1** | Peak RSS exceeds the ceiling on a real checkpoint | Something in the path materialises more than a block. **Halt the engine lane** — every downstream claim depends on this. Bisect with `/usr/bin/time -l` per stage |
| **G1** | The checkpoint will not fit on disk | Fall back to the largest that fits, record the actual size in `DEFINITION_OF_DONE.md`, and state the limitation in the report. Do **not** substitute the synthetic manifest and call it proven |
| **G2** | A metric disagrees with the Python reference | Highest-severity finding available. The engine is the product; a wrong number is worse than no number. Halt and bisect against the golden tensors |
| **G3** | Two runs produce different bytes | Non-determinism — usually floating-point reduction order or a timestamp in the body. Fix the ordering; timestamps belong in the run-metadata block only (`REPORT_ARCHITECTURE.md` §3.2) |
| **G4** | No reader can find the fragile layers unaided | The surface is not the differentiator it was assumed to be. Do not add features; take [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §5's headless pivot seriously |
| **G5** | No design partner changes a decision | This is the strategy's kill signal, not an engineering bug. Do not respond by building the next module. Re-read `VALIDATION_PLAN.md` §4 |

## 8. Estimated shape

Not a schedule — this plan has no calendar. A shape, for sequencing intuition,
against the strategy's 90-day frame:

| Wave | Tasks | Longest chain | Strategy window |
| --- | --- | --- | --- |
| 0 | 4 | 1 (`QM-0100`, wall-clock bound) | Days 0–30 |
| 1 | 4 | 2 (`0101`→`0030`) | Days 0–30 |
| 2 | 6 | 3 (`0120`→`0121`→`0122`) | Days 30–60 |
| 3 | 6 | 2 (`0123`→`0125`) | Days 30–60 |
| 4 | 6 | 2 (`0141`→`0142`) | Days 30–60 |
| 5 | 5 | 2 (`0161`→`0162`) | Days 60–90 |
| 6 | 5 | 3 (`0090`→`0091`→`0165`) | Days 60–90 |

Wave 2 is the centre of gravity and the only wave with genuinely novel code in
it. Everything before it is plumbing that already mostly exists; everything after
it is presentation and proof.

## 9. Deferred waves

The previous plan's Waves 3–4 (tile pyramid, GLB, tileset, Cesium viewer, matrix
workspace, chat) are preserved in `phases/phase-04-*` through `phases/phase-07-*`
and become the post-v1 visualization release. Their execution order is unchanged
and still correct; only their start condition has moved, from "after G1" to
"after v1 ships or `VALIDATION_PLAN.md` §5 says to pivot."

## 10. v1 dependency rewiring

Thirteen tasks from Phases 00–09 are in v1 but had dependency edges into tasks
that are now `Deferred`. Their `## Status` blocks carry the v1 unblock condition;
their `## Dependencies` sections still record the original edges, which return
with the platform release. This table is the reconciliation.

| Task | Original edge | v1 edge | Why |
| --- | --- | --- | --- |
| `QM-0010` Qwen resolver | `QM-0005` | none — **Ready** | The spatial contract has no bearing on a name resolver |
| `QM-0012` model metadata | `QM-0005` | none — **Ready** | Feeds the manifest's `model` block |
| `QM-0030` streaming block reader | `QM-0003` | `QM-0100` | v1 streams the real checkpoint, not an LOD fixture |
| `QM-0031` CPU statistics pass | `QM-0030`, `QM-0020`, `QM-0022` | `QM-0030`, `QM-0020` | `QM-0022` (block registry) is deferred |
| `QM-0033` job runner | `QM-0032`, `QM-0022` | `QM-0032` | same |
| `QM-0037` backend selection | `QM-0034` (CUDA) | `QM-0126` (Metal) | v1's GPU lane is Metal |
| `QM-0081` failure injection | `QM-0080` | `QM-0033`, `QM-0143` | The v1 pipeline replaces the platform demo |
| `QM-0082` browser soak | `QM-0080`, `QM-0056`, `QM-0067` | `QM-0152` | Scope narrows to the diagnostics surface |
| `QM-0085` error/security audit | `QM-0080`, `QM-0075` | `QM-0152` | same |
| `QM-0090` documentation | `QM-0080` | `QM-0141`, `QM-0152` | Audits the report and the surface |
| `QM-0091` regenerate STATUS | `QM-0090`, `QM-0080`, `QM-0084` | `QM-0090`, `QM-0102` | `QM-0102` replaces `QM-0084` |
| `QM-0092` limitations | `QM-0084`, `QM-0035` | `QM-0102`, `QM-0127`/waiver | Metal replaces CUDA as the lane to describe |
| `QM-0093` licensing | `QM-0080` | none — **Ready** | A licence audit needs no pipeline |

**Forty-four tasks are stamped `Deferred`**: Phases 04–07 in full (30), the CUDA
lane (`QM-0034`…`QM-0036`, `QM-0083` — 4), and ten others whose purpose v1 either
does not need or covers elsewhere (`QM-0003`, `QM-0004`, `QM-0005`, `QM-0013`,
`QM-0021`, `QM-0022`, `QM-0023`, `QM-0080`, `QM-0084`, `QM-0094`). Each names the
v1 task that covers its purpose, where one exists.

## 11. The first three actions

For an agent starting now, with nothing in progress:

1. **`QM-0100`** — start the checkpoint download. It is hours long, it blocks the
   entire proof lane, and nothing else in the plan gets faster by doing it later.
2. **`QM-0001`** — while that runs, verify the baseline. `cargo test --workspace`
   and `npx vitest run`. Nothing may be built on an unverified baseline, and it
   takes minutes.
3. **`QM-0002`** — validate the plan's own citations. The web-workspace path
   drift is closed here: `QM-0006` renamed the directory, so
   `apps/web/matrix-workspace` no longer exists, and `QM-0002` corrected the
   `.plan/` prose that still carried the old name to
   `apps/web/quatricmorph-workspace`.

Then `QM-0101`, and the lanes open.

**And, in parallel with all three, not after them: `QM-0160`.** Send the first
design-partner message before the first line of engine code. The strategy is
explicit that this is the ordering solo founders get wrong.
