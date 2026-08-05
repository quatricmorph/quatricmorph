# QM-0121 — differential verification against an independent NumPy reference

> **Read this alongside [`QM-0121.md`](QM-0121.md).** Two agents worked `QM-0121`
> concurrently in one worktree. `QM-0121.md` is the **implementation** half,
> written by the other agent and committed as `988d4c6`. This file is the
> **verification** half, written by `impl-agent-14`, and covers the artefacts
> `QM-0121.md` §Not performed explicitly records as absent:
>
> > *"**No Python/NumPy reference and no golden vector file was produced by this
> > task.** … A differential test against a NumPy reference remains available to
> > `QM-0122`, which needs one for **G2**."*
>
> It is a separate file rather than an edit to `QM-0121.md` because that record
> is another agent's truthful account of its own run and destroying or rewriting
> it would be worse than the naming deviation. The precedent for a companion
> evidence file is `.plan/evidence/QM-0140-independent-check.py.md`.

## Task

`QM-0121` — Paired block reduction in `q_gpu::Backend` (`QUANT-002`, `V1-11`),
lane Q, boundary `crates/q-gpu`. Branch `task/qm-0121-metric-kernels`, worktree
`/Users/thanh/Quatricmorph/.qm-worktrees/qm-0121`, base `e82fe98`.

`.plan/EXECUTION_ORDER.md` §7 places gate **G2** immediately behind this task:
*"A metric disagrees with the Python reference → highest-severity finding
available. The engine is the product; a wrong number is worse than no number."*
`.plan/DIAGNOSTIC_ARCHITECTURE.md` §9 requires, at the golden level, *"a
committed Python/NumPy script under `python/`, run in CI-equivalent form"*, and
`.plan/TEST_STRATEGY.md` §0 rule 2 requires expected values to come from a hand
computation or an independent implementation, never from the code under test.
This file records the discharge of exactly that requirement.

## Research

Read in full before writing anything: `ARCHITECTURE.md` §19 · `AGENTS.md` ·
`.plan/DIAGNOSTIC_ARCHITECTURE.md` (all of it; §3.1's degenerate-case table and
its 2026-08-05 amendment, §3.2 axis convention, §4.1 the contract block, §4.2
composition and reduction order, §4.3 CPU-is-the-reference and the
`max_abs_* must be exact` rule, §5 residency, §8 forbidden claims, §9
verification levels) · `.plan/DATA_ARCHITECTURE.md` §8 fidelity vocabulary ·
`.plan/TEST_STRATEGY.md` §0 · `.plan/EXECUTION_ORDER.md` §7 ·
`docs/decisions/ADR-010-tensor-rank-ceiling.md` ·
`.plan/tasks/QM-0121-paired-block-reduction/TASK.md` (the task directory is
`QM-0121-paired-block-reduction`; the dispatch brief cited a path
`.plan/tasks/QM-0121-metric-kernels/TASK.md`, which does not exist — there is
exactly one `QM-0121` task directory and it is the one used here).

Read as the immediate dependency: `crates/q-quant/src/lib.rs`,
`crates/q-quant/src/rtn.rs`, `crates/q-quant/tests/reference_goldens.rs`,
`.plan/evidence/QM-0120.md`, and `python/reference/quantise_reference.py` —
the precedent whose shape this reference deliberately follows: spec quoted
verbatim in the module docstring, idiomatic NumPy, deterministic emission from
an **integer LCG** rather than a NumPy RNG, **no version strings** in the
emitted file so regeneration is byte-reproducible, and the golden's SHA-256
recorded here so a reviewer can regenerate and compare.

Read as the subject: `crates/q-gpu/src/lib.rs` (the pre-existing `Backend`
trait, `check_workload`, `CpuBackend`, and its 7 tests),
`crates/q-tensor-runtime/src/lib.rs` (`BlockData`), `crates/q-source/src/error.rs`
(`QError`), `crates/q-statistics/src/lib.rs` (`StatisticsFidelity`,
`.plan/DATA_ARCHITECTURE.md` §8's vocabulary as already implemented by
`QM-0020`).

`q-quant`'s boundary was respected and is untouched: it takes no Quatricmorph
dependency at all, and this work adds none — `q-gpu` does not depend on
`q-quant`, and the paired reduction never mentions quantisation.

## Summary

An independent NumPy reference for the paired block reduction, a discriminating
golden set emitted from it, and a differential test that checks the committed
`CpuBackend` against that golden **bit for bit** on every field of every case at
both channel axes.

* `python/reference/paired_reduction_reference.py` — written from
  `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.1's contract block, quoted verbatim in
  its docstring, and run **before** any Rust implementation was read.
* `crates/q-gpu/tests/goldens/paired-reduction-goldens.json` — 14 numeric cases,
  3 composition splits, 12 refusals. Byte-reproducible.
* `crates/q-gpu/tests/paired_reference_goldens.rs` — 11 tests, all bit-exact.

**Result: the implementation agrees with the reference exactly.** All 14 cases ×
2 axes × (1 whole-block + every per-channel) × 6 fields match on `to_bits()`,
with **zero** toleranced comparisons anywhere in the differential test.

**One defect was found and fixed.** The implementation **panicked** (index out
of bounds, `crates/q-gpu/src/paired.rs:286`) instead of refusing when a
`BlockData` declares a shape whose product exceeds its buffer. `BlockData`'s
`rows`, `columns` and `values` are all `pub`, so this is reachable from safe
code. Fixed by adding `require_dense`, ordered **after** `check_workload` so
that the pre-existing budget-precedence test (which declares 4096×4096 with an
empty buffer) keeps its more specific refusal. The other agent adopted the fix
into `988d4c6` and added a unit test for it.

### The arithmetic the reference implements

Per element, in flat row-major index order, for both the whole block and the one
channel that element belongs to:

```text
d              = f64(base) − f64(counterpart)      # f32→f64 is exact
sum_sq_base   += f64(base) · f64(base)
sum_sq_delta  += d · d
sum_abs_delta += |d|
max_abs_delta  = max(max_abs_delta, |d|)
max_abs_base   = max(max_abs_base,  |f64(base)|)
count         += 1
```

Channel of element `i`, where `(row, column) = divmod(i, columns)`: `row` under
`ChannelAxis::Rows`, `column` under `ChannelAxis::Columns`. Channel count is
`rows` or `columns` respectively.

### Why the reference sums in a Python loop rather than with `np.sum`

`np.sum` on float64 uses **pairwise** summation — a different and unspecified
grouping. `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2 *fixes* the reduction order
(*"partials accumulate in that order, single-threaded at the accumulation
step"*, because *"`V1-13` requires byte-identical output across runs"*), so
using `np.sum` would have silently defined a different answer from the one the
architecture specifies. The element-wise arithmetic is vectorised NumPy; only
the three ordered sums are a Python loop over `float()`, which is IEEE-754
binary64 with round-to-nearest-even. **The pairwise value is emitted anyway**,
as `discriminators.numpy_pairwise_*`, so the golden records how far a
different-but-reasonable order lands from the specified one instead of leaving a
reviewer to guess. See the measured figures below.

## Acceptance criteria

| # | Criterion | Where verified | Verdict |
| --- | --- | --- | --- |
| 1 | Hand-computed 3×4: every field of `PairedPartials` and every `ChannelPartials` matches values computed by hand in the test | `the_hand_computed_3x4_case_matches_arithmetic_written_out_in_this_test`, plus `every_golden_case_matches_the_numpy_reference_bit_for_bit` case `hand_computed_3x4` | **Met.** Hand arithmetic written out in the doc comment, asserted as literals in the test, and independently reproduced by the NumPy reference — three derivations, one answer |
| 2 | Orientation: a non-square asymmetric fixture where the wrong axis gives a different answer, and the test would fail if it did not | `orientation_reducing_over_the_wrong_axis_gives_a_different_answer` | **Met, and strengthened.** Non-square `asymmetric_2x5` **and** square `square_asymmetric_3x3` — see "how the goldens discriminate" |
| 3 | Partials compose: halves summed equal the whole for every additive field; `max_*` composes by maximum | `partials_compose_when_a_block_is_reduced_in_two_halves`, 3 split fixtures × 2 axes | **Met.** `count` and both `max_abs_*` compose **exactly**; the three sums compose exactly on the dyadic fixture and to the **measured** ULP on the inexact ones |
| 4 | Two runs are bit-identical | `two_runs_over_the_same_blocks_are_bit_identical`, all 14 cases × 2 axes | **Met**, compared through bit patterns |
| 5 | Shape mismatch, empty block, non-finite value and bad axis all refuse before arithmetic, each naming the reason | `every_refusal_in_the_golden_set_is_refused_with_the_reason_named` (11 golden refusals), `a_block_whose_value_count_disagrees_with_its_shape_is_refused` | **Met for shape / empty / non-finite / ragged.** The bad-axis path is covered by the implementation's own unit tests, not by this differential test — see §Not performed |
| 6 | Allocation is proportional to channel count, not element count | `the_partials_held_track_channel_count_and_not_element_count` | **Met** at the contract level (`per_channel.len()`, `size_of::<ChannelPartials>() == 48`). The measured-allocation proof is the implementation's `paired_allocation_bounds.rs` |
| 7 | The signature mentions neither quantisation nor any specific second-operand provenance | `the_signature_is_neutral_about_where_the_counterpart_came_from` | **Met.** Type names carry no quantisation vocabulary, and the kernel is driven with an unrelated second operand (`lcg_independent_counterpart_64x64`, `DIFF-001`'s shape) against the reference's value for it |

## Architecture conformance

* **`.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.1** — the reference implements exactly
  the six declared partials and **no finished metric**. There is no RMSE, no
  relative error and no norm in the golden, for the same reason there is none in
  the type: *"Computing the finished metric per block and averaging is the single
  most likely correctness bug in this engine."*
* **§4.2** — sums accumulate sequentially in flat row-major order in **both**
  implementations, and the golden records what a different order would give.
* **§4.3** — *"`max_abs_*` **exact** — a max reduction has no rounding excuse."*
  Both `max_abs_*` fields are asserted on `to_bits()`, everywhere, including
  under composition. In fact **every** comparison in the differential test is on
  `to_bits()`; there is no tolerance anywhere in it.
* **§3.1's 2026-08-05 amendment** — inherited, not re-derived. The transferable
  rule (*"A golden set needs inputs selected to **discriminate**, not merely to
  cover"*) is implemented mechanically: see below.
* **§8 forbidden claims** — nothing here predicts accuracy, ranks importance, or
  interprets a weight. A metric is arithmetic.
* **`.plan/DATA_ARCHITECTURE.md` §8** — see §Claim limits for the fidelity
  labelling position and the one gap.
* **`ADR-010`** — no rank ceiling is exercised here because `BlockData` is rank-2
  by construction (`BlockExtent` is 2-D only; ADR-010 §Consequences). A rank > 3
  tensor is refused upstream at block planning, not flattened here. The
  block-rank axis refusal lives in the implementation's `ChannelAxis::from_index`.
* **`.plan/TEST_STRATEGY.md` §0** — no test added here touches the network; the
  golden is embedded with `include_str!`, so the differential test performs no
  file I/O at runtime either.

### How the goldens were chosen to discriminate

`QM-0120` shipped a 100 % error behind a passing differential test because its
only constant magnitude was `c = 1`, the single value at which two candidate
formulas agree. The defence here is mechanical rather than editorial: the
reference evaluates **each plausible wrong formula on the same input** and
records whether it lands somewhere else.

| wrong formula ruled out | agrees with the right answer whenever… | ruled out by |
| --- | --- | --- |
| `\|Σ d\|` for `Σ \|d\|` | every delta shares one sign | 6 cases; on `hand_computed_3x4`, 3.75 vs 2.75 |
| `max d` for `max \|d\|` | the extreme delta is positive | 7 cases; `hand_computed_3x4` puts max\|d\| on a **negative** delta at (2,1) |
| `max w` for `max \|w\|` | the extreme weight is positive | 9 cases; `hand_computed_3x4` puts max\|w\| = 8 on a **negative** weight at (1,0), where `max w` = 6 |
| `Σ ŵ²` for `Σ w²` | the two blocks are equal | 11 cases; `hand_computed_3x4` has 137.140625 vs 143.828125 |
| an `f32` accumulator | the block is small or dyadic | `f32_accumulator_would_be_wrong_64x64`: f64 gives 1048576.999755859375, f32 gives exactly 1048576.0 |
| a **different summation order** | the values are dyadic or few | see the ULP table below |
| the two axes **transposed** | the block is symmetric under transposition | `square_asymmetric_3x3` — square, so the channel *count* matches and only the values can catch it |

Two points worth a reviewer's attention:

1. **The reference refuses to emit a golden set that covers without
   discriminating.** `build()` checks that every wrong formula is ruled out by
   at least one case and exits non-zero otherwise. **It fired during this run**:
   the first golden set could not distinguish sequential from pairwise summation
   on `sum_sq_delta` or `sum_abs_delta`, so two cases were added specifically to
   discriminate — `lcg_independent_counterpart_64x64` and
   `summation_order_matters_64x64`. The check is not decorative; it caught a real
   gap in the set before any Rust was run against it.
2. **The Rust asserts the discrimination too.**
   `every_wrong_formula_is_ruled_out_by_a_case_that_can_actually_tell_it_apart`
   checks both that every wrong formula is covered *and* that on each case
   claiming to rule one out, the backend's own answer really does differ from it.
   Without the second half the coverage map would be a claim about the reference
   alone.
3. **What the goldens cannot discriminate, stated plainly.** Every field is even
   in the sign of the delta (`sum_sq`, `sum_abs`, `max_abs`), so **no golden can
   distinguish `base − counterpart` from `counterpart − base`.** That is a
   property of the specified metric, not a gap in the set, and no test here
   claims otherwise.

### Measured floating-point divergence

Every figure below is **measured** by the reference and emitted into the golden;
none is estimated. ULP distances are absolute differences of `f64` bit patterns.

**Summation order — sequential (specified) vs NumPy pairwise:**

| field | fixture | measured |
| --- | --- | --- |
| `sum_sq_base` | `lcg_pseudorandom_64x64`, `lcg_independent_counterpart_64x64` | **27 ULP** |
| `sum_sq_delta` | `lcg_independent_counterpart_64x64` | **28 ULP** |
| `sum_abs_delta` | `summation_order_matters_64x64` | **1020 ULP** |

**Composition — two half-block reductions summed, vs the undivided reduction:**

| fixture | axis | `sum_sq_base` | `sum_sq_delta` | `sum_abs_delta` | `max_abs_*`, `count` |
| --- | --- | --- | --- | --- | --- |
| `hand_computed_3x4_split_at_row_1` | rows, columns | **0** | **0** | **0** | 0 |
| `lcg_pseudorandom_64x64_split_at_row_32` | rows, columns | **43 ULP** | 0 | 0 | 0 |
| `lcg_independent_counterpart_64x64_split_at_row_32` | rows, columns | **43 ULP** | **26 ULP** | 0 | 0 |
| …the same, per channel | columns | 0 | **7 ULP** (max over 64 channels) | 0 | 0 |

**The bound, and why it is what it is.** Composition regroups a sequential sum:
`((a₀+…+a_{k−1}) + (a_k+…+a_{n−1}))` is a different association from
`(((a₀+a₁)+a₂)+…)`, so the two agree bit-for-bit **only when every partial sum is
exact**. On the dyadic fixture they are, and the test asserts `to_bits()`
equality (0 ULP). On the pseudorandom fixtures they are not, and the test asserts
the **exact measured distance** recorded by the reference rather than a guessed
tolerance — a bound that would silently absorb a real regression. 43 ULP at
≈1333.22 is a relative deviation of ≈7.3 × 10⁻¹⁵, four orders inside §4.3's
1 × 10⁻⁶ backend tolerance; that tolerance is quoted for scale only and is a
different comparison (CPU vs GPU), not the one asserted here.

`count` and both `max_abs_*` compose **exactly and always** — integer addition
and a maximum have no rounding — and that is asserted separately from the sums so
the exactness claim is not diluted by them.

## Tests added

| Test | File | Asserts |
| --- | --- | --- |
| `every_golden_case_matches_the_numpy_reference_bit_for_bit` | `crates/q-gpu/tests/paired_reference_goldens.rs` | 14 cases × 2 axes × (whole block + every channel) × 6 fields, on `to_bits()` |
| `every_wrong_formula_is_ruled_out_by_a_case_that_can_actually_tell_it_apart` | same | every wrong formula is ruled out by ≥ 1 case, and the backend really differs from it there |
| `the_hand_computed_3x4_case_matches_arithmetic_written_out_in_this_test` | same | AC 1; hand arithmetic in the test, both axes, all channels |
| `identical_blocks_have_zero_delta_and_a_zero_counterpart_makes_delta_equal_base` | same | Test Cases rows 2 and 3, as relationships |
| `orientation_reducing_over_the_wrong_axis_gives_a_different_answer` | same | AC 2; non-square **and** square-asymmetric |
| `partials_compose_when_a_block_is_reduced_in_two_halves` | same | AC 3; 3 splits × 2 axes, against the reference's own halves and composed values, with the measured ULP asserted exactly |
| `two_runs_over_the_same_blocks_are_bit_identical` | same | AC 4 |
| `every_refusal_in_the_golden_set_is_refused_with_the_reason_named` | same | AC 5; 11 golden refusals, message content asserted |
| `a_block_whose_value_count_disagrees_with_its_shape_is_refused` | same | the panic found and fixed |
| `the_signature_is_neutral_about_where_the_counterpart_came_from` | same | AC 7 |
| `the_partials_held_track_channel_count_and_not_element_count` | same | AC 6 at the contract level |

**11 tests, 1 new binary.**

### Floor

| | rust tests | rust binaries | web tests | web files |
| --- | --- | --- | --- | --- |
| `main` / base `e82fe98` | 677 | 51 | 115 | 13 |
| after the implementation half (`988d4c6`) | 704 | 52 | 115 | 13 |
| **after this half (measured)** | **715** | **53** | **115** | **13** |

`704 + 11 = 715` and `52 + 1 = 53` reconcile exactly, which is also the check
that no pre-existing test was removed, weakened or `#[ignore]`d. `scripts/baseline.json`
is raised to 715/53. **Two other branches are raising this floor concurrently
and the controller reconciles at merge.**

### Failing first

Metrics are pure and deterministic, so there is no excuse, and the failure was
recorded twice.

**1. Before any implementation existed** — the differential test was written
first, against the intended API, and run:

```
error[E0432]: unresolved import `q_gpu::MAX_BLOCK_RANK`
  --> crates/q-gpu/tests/paired_reference_goldens.rs:41:72
error[E0599]: no method named `as_channel_partials` found for reference `&PairedPartials`
error[E0599]: no method named `merge` found for struct `PairedPartials`
…
error: could not compile `q-gpu` (test "paired_reference_goldens") due to 16 previous errors
exit=101
```

**2. Against the committed implementation** — after the test was rewritten
against the specified contract only, 10 of 11 passed and one failed, naming a
real defect:

```
test a_block_whose_value_count_disagrees_with_its_shape_is_refused ... FAILED

---- a_block_whose_value_count_disagrees_with_its_shape_is_refused stdout ----
thread '…' panicked at crates/q-gpu/src/paired.rs:286:36:
    a_block_whose_value_count_disagrees_with_its_shape_is_refused

test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101
```

**3. After the first fix attempt** — placing the new check inside `validate_pair`
pre-empted the pre-existing budget-precedence test, which was a real ordering
error on my part and is recorded rather than quietly corrected:

```
test paired::tests::every_refusal_precedes_every_accumulation ... FAILED
thread '…' panicked at crates/q-gpu/src/paired.rs:1286:9:
query rejected: paired reduction refuses a ragged block: the base block holds 0
values but declares the shape [4096, 4096] — 16777216 values
test result: FAILED. 31 passed; 1 failed
exit=101
```

Moving `require_dense` to sit **after** `check_workload` resolved it: a huge
declared shape with an empty buffer is a budget refusal, which is the more
specific answer, and both still precede every read of a value.

## Validation evidence

Exit codes captured on a line separate from any pipe.

```
cargo fmt --all -- --check                            exit=0
cargo clippy --workspace --all-targets -- -D warnings exit=0
cargo build --workspace --all-targets                 exit=0
cargo test --workspace                                exit=0
    715 passed; 0 failed; 0 ignored; 53 binaries
cargo test -p q-gpu                                   exit=0
    33 passed (lib) + 1 (paired_allocation_bounds)
  + 11 (paired_reference_goldens) + 0 (doc-tests)
```

```
./scripts/verify-baseline.sh                          exit=1
```

**`verify-baseline.sh` exits 1, and every Rust and CLI check inside it passes.**
The three failing checks are all the same web-toolchain problem, and none is
caused by this change:

```
  ok    guard self-tests
  ok    baseline.json parses and carries every floor
  ok    fixtures/tiny-llama-2shard is present
  ok    cargo fmt --all -- --check
  ok    cargo clippy --workspace --all-targets -- -D warnings
  ok    cargo build --workspace --all-targets
  ok    cargo test --workspace exited 0
  ok    rust: 0 failed
  ok    rust tests: measured 715, floor 715 — at floor
  ok    rust test binaries: measured 53, floor 53 — at floor
  FAIL  npx vitest run exited 1
  FAIL  web: 111 of 111 tests passed (skipped, todo, or failed)
  FAIL  web tests: measured 111, floor 115 — REGRESSION, 111 < 115
  ok    web test files: measured 13, floor 13 — at floor
  … all 12 CLI goldens ok …
  elapsed: 11s (budget: 300s)
```

The guard's own self-tests pass, so `./scripts/verify-baseline.test.sh` passes as
a precondition of the above.

**Cause, established rather than assumed.** One vitest file errors at collection:

```
FAIL quatricmorph-workspace/src/viz/__tests__/expr.test.ts
Error: Cannot find package 'three' imported from
  …/apps/web/quatricmorph-workspace/src/util/geometry.ts
```

`three` is declared in `apps/web/quatricmorph-workspace/package.json` (`^0.185.1`)
but **is not installed anywhere in this environment** — neither
`apps/web/node_modules/three` nor
`apps/web/quatricmorph-workspace/node_modules/three` exists **in the main
checkout**, which is where both of this worktree's `node_modules` symlinks point.
The worktree therefore sees exactly what the main checkout has, and the main
checkout cannot resolve `three` either. Installing it needs the network, which
the dispatch brief forbids and which `verify-baseline.sh`'s own header forbids
(*"this guard must not depend on a network"*).

**Not caused by this change, and not repairable within this boundary.**
`git status --short | grep apps/web` returns nothing: no web file was added,
modified or deleted here. `apps/web/**` is `QM-0150`'s live boundary and is on
this task's do-not-touch list. The four missing tests are the four in
`expr.test.ts`, which never ran. **This is reported for the controller, not
worked around, and no floor was lowered to accommodate it** — `web_tests` stays
at 115.

### Reference provenance — how a reviewer re-derives every golden

| | |
| --- | --- |
| Script | `python/reference/paired_reduction_reference.py` |
| Interpreter | CPython **3.14.6** |
| Package | **NumPy 2.5.1** (the only third-party import) |
| Command | `python3 python/reference/paired_reduction_reference.py --emit-goldens crates/q-gpu/tests/goldens/` |
| Output | `crates/q-gpu/tests/goldens/paired-reduction-goldens.json`, 1 694 668 bytes |
| **SHA-256** | **`de32521cd6b25d14d06f87e180742d5aba186b530528befe7a6683ade3ac4493`** |
| Contents | 14 numeric cases, 3 composition splits, 12 refusals |
| Network | none — the script imports only `argparse`, `hashlib`, `json`, `pathlib`, `sys` and `numpy` |

**Byte-reproducibility was verified, not assumed:** the script was run twice to
stdout and the outputs compared with `cmp` (exit 0), and the second run compared
against the emitted file with `cmp` (exit 0). The file carries **no version
strings** — interpreter and NumPy versions and the SHA-256 go to stderr — so a
regeneration that differs is a real change, not a toolchain artefact. Inputs come
from an **integer LCG** (`state = (state·1103515245 + 12345) mod 2³¹`), not a
NumPy RNG, so regeneration cannot drift with a NumPy stream change. This is the
discipline `python/reference/quantise_reference.py` established for `QM-0120`.

**The reference was written and run before any Rust implementation was read.**
The hand computation in the module docstring of
`the_hand_computed_3x4_case_matches_arithmetic_written_out_in_this_test` was done
by hand on paper first and matched the reference's output on all six whole-block
fields and all seven per-channel entries (3 rows + 4 columns) exactly — two
independent derivations agreeing before the third (the Rust) was consulted.

## Negative paths tested

| Path | Covered | How |
| --- | --- | --- |
| Shape mismatch rejected before execution | **Yes** | 2 golden refusals (rows differ, columns differ — the second so a check comparing only element counts cannot pass); the message must name all four extents |
| Empty input | **Yes** | 3 golden refusals: 0 rows, 0 columns, and 0×0 (where the shapes *match*, so it reaches the empty check rather than the shape check) |
| NaN / ±Inf handled explicitly, not propagated | **Yes** | 6 golden refusals: NaN in base, NaN in counterpart, +Inf, −Inf, both blocks non-finite (the **earliest** position across both must be named), and both non-finite at the same position (base is named, being checked first) |
| Division by zero | **N/A, and deliberately so** | This kernel performs **no division**. `sum_sq_base == 0` is a zero denominator only for a *derived* metric, and §4.1 puts every derived metric in the aggregation. `all_zero_blocks_2x3` covers the all-zero input and produces `+0.0` partials, not a NaN |
| Ragged block (value count vs declared shape) | **Yes — and it found a panic** | `a_block_whose_value_count_disagrees_with_its_shape_is_refused`; fixed by `require_dense` |
| Budget exceeded | Covered by the implementation's own tests, not by this differential test | See §Not performed |
| Rank > 3 refused rather than flattened | **Not reachable at this boundary** | `BlockData` is rank-2 by construction; ADR-010's ceiling is enforced upstream at block planning. Recorded rather than faked |
| Unknown dtype refused rather than guessed | **Not reachable at this boundary** | `BlockData::values` is `Vec<f32>`; no dtype enters this kernel. `SRC-014` owns dtype refusal at ingestion |

## Files changed

Added:

* `python/reference/paired_reduction_reference.py`
* `crates/q-gpu/tests/goldens/paired-reduction-goldens.json`
* `crates/q-gpu/tests/paired_reference_goldens.rs`
* `.plan/evidence/QM-0121-differential-verification.md` (this file)

Modified:

* `crates/q-gpu/Cargo.toml` — `serde_json` added to `[dev-dependencies]` only,
  to read the golden. It is absent from the built library, the same idiom
  `q-quant` uses for `quant-goldens.json`.
* `Cargo.lock` — the one line that follows from it.
* `scripts/baseline.json` — floor raised 704/52 → 715/53. Upward only.
* `crates/q-gpu/src/paired.rs` — `require_dense`, the panic fix. **This file is
  the other agent's; the fix was subsequently absorbed into their commit
  `988d4c6`, so it no longer appears as a change in this branch's working tree.**

Not touched: `crates/q-statistics`, `crates/q-catalog`, `crates/q-cli`,
`crates/q-nsir`, `crates/q-architecture`, `apps/web/**`,
`crates/q-tensor-runtime`, `crates/q-source`, `crates/q-quant`, `STATUS.md`,
`ARCHITECTURE.md`, `docs/decisions/`, `mm/`, `quatricmorph/`, `.github/`, the
root `Cargo.toml`, and every `scripts/` file except `baseline.json`.

`apps/web/node_modules` and `apps/web/quatricmorph-workspace/node_modules` were
symlinked from the main checkout so the baseline guard's vitest step can run.
Both are gitignored, nothing was installed, and no network was touched.

## Not performed

* **No GPU executed anything.** `CpuBackend` only. No Metal, no CUDA, no wgpu
  path was added or run. `QM-0126` owns the Metal lane.
* **No benchmark.** Nothing here measures throughput or latency, and no
  performance claim is made.
* **The `BudgetExceeded` path is not exercised by this differential test.** It
  is a backend-capability refusal rather than a numerical one, and the
  implementation's own unit tests cover it. No golden refusal exercises it
  because a NumPy reference has no notion of a device-memory budget.
* **The bad-axis refusal is not exercised by this differential test.**
  `ChannelAxis` is a two-variant enum, so an out-of-range axis is unrepresentable
  at the specified contract surface; the conversion that can refuse
  (`ChannelAxis::from_index`) is the implementation's own API and is covered by
  its unit tests. This test deliberately drives only the surface `TASK.md`
  §Data Contracts specifies.
* **The fidelity label is not verified.** See §Claim limits — this is a real gap.
* **No real quantised checkpoint was involved.** Every input is a synthetic
  fixture or an LCG stream.
* **The web suite was not modified** and its counts were not re-derived beyond
  the baseline guard confirming 115/13.
* **This agent did not write the implementation.** `crates/q-gpu/src/paired.rs`,
  the `crates/q-gpu/src/lib.rs` changes, `crates/q-gpu/tests/paired_allocation_bounds.rs`
  and `.plan/evidence/QM-0121.md` are another agent's work, committed as
  `988d4c6`. This record covers only the reference, the goldens, the differential
  test, and the `require_dense` fix.

## Claim limits

* **No GPU executed anything.** Every number here was produced by
  `q_gpu::CpuBackend` on the CPU. Claiming a GPU computed something the CPU
  computed is on `PRODUCT_SCOPE.md` §5.2's forbidden list.
* **No benchmark was run**, and nothing here says anything about deployment,
  partner behaviour, or how this performs at checkpoint scale.
* **Exact vs approximate.** Every result is `exact` in the sense that every
  value in the region was read and accumulated in `f64` — nothing is sampled and
  nothing is estimated. In `.plan/DATA_ARCHITECTURE.md` §8's vocabulary the
  correct label is **`aggregate`** (*"a statistic over a region, computed from
  all its values"*), which is what `QM-0020` established via
  `q_statistics::StatisticsFidelity`, and deliberately **not** §8's `exact`,
  which names the *values as stored in the checkpoint* rather than a statistic
  over them. The **bit-exact** claims in this record are a different and
  narrower statement: that two implementations produce identical bit patterns.
* **`PairedPartials` carries no fidelity label — a real gap.** The dispatch brief
  required every result to be labelled with §8's vocabulary. The committed type
  has no `fidelity()` method and no label on the wire, so the label exists only
  in this prose. A consumer receiving a `PairedPartials` cannot read its fidelity
  from the value. **Recommended for `QM-0122`/`QM-0123`:** add
  `fn fidelity(&self) -> StatisticsFidelity { Aggregate }`, derived and never
  stored, matching `TensorStatistics::fidelity`. Recorded rather than silently
  dropped.
* **§8's monotonic-degradation rule needs a ruling.** §8 says *"a statistic over
  quantized data is at best `sampled`, never `aggregate`."* Read literally, and
  given that v1's counterpart is a simulated quantisation, the whole diagnostic
  engine could only ever emit `sampled`. The reading taken here is that this
  kernel's partials describe **exactly the two f32 blocks it was handed**, both
  read in full, and make no claim about any checkpoint beyond them — so
  `aggregate` is correct and the counterpart's provenance is the caller's to
  carry (which is also acceptance criterion 7). **This is an interpretation, not
  a settled rule, and the controller should confirm it before `QM-0123` builds
  aggregation on top of it.**
* **No metric has been validated against a real quantised checkpoint.** Every
  fixture is synthetic. What is proven is that the Rust and an independent NumPy
  implementation of §4.1 agree, on inputs chosen to discriminate — not that the
  specification is the right one for real weights.
* **No semantic claim is made about model weights.** A metric is arithmetic.
  Quantisation error tells you nothing about what a layer *does*, and nothing
  here ranks importance, predicts accuracy, or interprets a value.
* **Agreement is bounded by the inputs chosen.** This is `QM-0120`'s lesson and
  it applies to this record too. What is proven is agreement on 14 cases selected
  to distinguish 10 named wrong formulas plus axis transposition. A wrong formula
  nobody thought of is not ruled out by any of it.
* **Two agents worked `QM-0121` concurrently in one worktree.** The full account
  is in §Concurrency below. Nothing another agent wrote was deleted, reverted or
  claimed; the one edit made to their file (`require_dense`) was a fix for a panic
  this verification found, and they absorbed it into their own commit.
* **`.plan/DIAGNOSTIC_ARCHITECTURE.md` §5 says `channels × 40 B`; `TASK.md` says
  `channels × 48 B`.** 48 is correct for the six-field struct both documents
  declare, and 48 is what is measured. §5 appears to predate a field. Neither
  document was edited — both are outside this task's boundary. Independently
  observed and independently recorded in `QM-0121.md`.

## Concurrency

Recorded because it bears on how this branch should be reviewed and merged.

* The dispatch brief assigned `impl-agent-14` one task and one worktree. A second
  agent was working `QM-0121` in the same worktree, on the same branch, at the
  same time.
* Timeline (file mtimes and `git log --date=format:%H:%M:%S`): my goldens landed
  17:09 and my reference 17:10; their `crates/q-gpu/src/lib.rs` edit 17:07 and
  `crates/q-gpu/src/paired.rs` 17:13. They committed `8154672`, then amended to
  **`988d4c6` at 17:27:08** after taking the `require_dense` fix, and recorded
  their evidence in **`9b7dc84` at 17:28:23**. This commit is **`7cf66aa` at
  17:32:55**.
* **Nothing of theirs was deleted, reverted, or claimed.** My only edit to a file
  of theirs is `require_dense` in `crates/q-gpu/src/paired.rs`, which fixed a
  panic; they then absorbed it and added their own unit test for it.
* **My work was backed up** to
  `…/scratchpad/qm0121-backup/` before any of this, so a clobber would have been
  recoverable. SHA-256 of the golden there matches the committed one.
* **The two halves are complementary, not duplicative.** `QM-0121.md` §Not
  performed states plainly that no Python/NumPy reference and no golden file was
  produced by that half. This half is exactly those artefacts.
* **The controller must reconcile the floor at merge.** `QM-0121.md` anticipated
  this: *"If the other agent's files are later committed to this branch, the
  counts recorded in `scripts/baseline.json` will no longer describe its head,
  and must be re-measured at merge."* They now are committed, and the floor is
  raised to the re-measured 715/53.
* `TASK.md`'s `## Status` was already set to `In Progress` by the controller in
  `e49ac24`, and `## Orchestration` was appended by the other agent. Neither was
  edited here, to avoid two agents writing one section.

## Independent review

## Merge
