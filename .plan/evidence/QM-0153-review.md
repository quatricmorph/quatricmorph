# QM-0153 — independent review

**VERDICT: APPROVE**, with the residual risks named in *Defects* below. None of
them blocks: every one is either already disclosed in `.plan/evidence/QM-0153.md`,
pre-existing to `QM-0150`, or a test-quality nit whose underlying guarantee is
held by a different test that I proved bites.

Reviewer: independent agent, did not implement. Branch
`task/qm-0153-rendering-ceiling` at `4c1f828`, 5 commits ahead of `main`
(`39b3aa2`). Read-only: nothing in the worktree was modified except this file.
All mutation work was done on `git archive` copies in a scratchpad outside the
repository.

The claim I would stake the repository's credibility on is the narrow one the
task itself calls important: **no truncation path exists, and the tests that say
so will fail if one is added.** I verified that by mutation, not by watching a
green run.

---

## Gates, re-run by me, with measured exit codes

Exit codes captured with the command's own status (not a pipeline's tail).

```bash
cd /Users/thanh/Quatricmorph/.qm-worktrees/r4-qm0153
./scripts/verify-baseline.sh      # verify-baseline EXIT=0
./scripts/license-audit.sh        # license-audit  EXIT=0
cd apps/web && npx vitest run     # vitest         EXIT=0
```

```
 Test Files  22 passed (22)
      Tests  361 passed (361)
```

`verify-baseline.sh` summary, my run:

```
  ok    cargo fmt --all -- --check
  ok    cargo clippy --workspace --all-targets -- -D warnings
  ok    cargo build --workspace --all-targets
  ok    cargo test --workspace exited 0
  ok    rust tests: measured 744, floor 744 — at floor
  ok    rust test binaries: measured 54, floor 54 — at floor
  ok    web tests: measured 361, floor 361 — at floor
  ok    web test files: measured 22, floor 22 — at floor
  ...13 CLI goldens ok
verify-baseline: OK
```

`license-audit.sh`: `all checks passed` (the implementer recorded it as not run;
I ran it, and it is clean).

---

## Floor arithmetic, checked independently of the implementer's table

`336 + 24 + 1 = 361` over `21 + 1 = 22`. Measured 361/22. Reconciles exactly.

The stronger check is the diff, not the table. Across the whole branch, the only
files under `apps/web/diagnostics/src/__tests__/` that changed are:

```
 .../__tests__/artifacts.test.ts    |   6 +      (0 deletions)
 .../__tests__/degradation.test.ts  | 419 +++++  (new file)
```

`artifacts.test.ts` is purely additive — one new generated `it` for
`aggregated-colour.svg`. No pre-existing web test file was touched at all, so no
existing test could have been removed, weakened, `.skip`ped, or edited to make
new code pass. That is a direct proof, and it is what the reconciliation
arithmetic is a proxy for. `scripts/baseline.json` was raised in `9401dc8`, the
same commit as the implementation and the passing tests.

Rust: no Rust file appears in `git diff --stat main..HEAD`; 744/54 confirmed at
floor by my own `cargo test --workspace` inside `verify-baseline.sh`.

---

## Failing-first: real, and the 11 passers are not vacuous

Reproduced without touching the worktree: `git archive main` into a scratchpad,
then `9950ec7`'s version of `degradation.test.ts` dropped on top of `main`'s
source.

```
 Test Files  1 failed (1)
      Tests  13 failed | 11 passed (24)
```

Exactly the split claimed. `git show 9950ec7 --stat` is the test file alone, 419
insertions, no source. `git diff 9950ec7 HEAD -- <that file>` is 3 insertions /
3 deletions, all of them `'sampled'` → `'engine-coarse'` in two assertions,
exactly as disclosed.

The 11 that already passed on `main` are the anti-truncation assertions. They
are **not** vacuous — see the mutation results below, where all four of them fail
under two independently constructed truncation bugs.

---

## AC5 — do the tests actually bite? Yes. Three mutations.

All mutations applied to a scratchpad copy of `HEAD`; the worktree was never
modified.

**Mutation 1 — gross truncation.** `planRow`: force `factor = 1` and cap the
loop at `Math.min(bandCount, maxColumns)`, i.e. draw the first `maxColumns`
channels and drop the tail. This is the shape of a "performance fix" someone
would plausibly write.

```
Tests  8 failed | 16 passed (24)
  × every_channel_index_maps_into_exactly_one_cell_at_a_thousand_layers_of_sixty_five_thousand_channels
  × every_row_of_a_grid_built_at_the_ceiling_covers_every_one_of_its_channels
  × no_band_is_dropped_even_when_the_manifest_publishes_no_channel_extent
  × the_cell_count_stays_inside_the_ceiling_at_dimensions_that_divide_badly
  (+4 legend/fidelity tests)
```

**Mutation 2 — subtle off-by-one truncation.** Keep the aggregation, drop only
the final partial group: `columnIndex * factor < bandCount` →
`(columnIndex + 1) * factor <= bandCount`. This one loses a handful of channels
per row and would be invisible on screen.

```
Tests  4 failed | 20 passed (24)
```

The four failures are precisely the four anti-truncation tests. Both the
occupancy check (a channel covered zero times) and the band-count check
(`sum(bandsPerCell) === bands.length`, which still holds where the manifest
publishes no channel extent) catch it. AC5's guard is genuine and multi-angled.

**Mutation 3 — mean instead of maximum.** `worst = Math.max(...)` → a running
mean.

```
Full diagnostics suite: Tests  4 failed | 242 passed (246)
  × heatmap.test.ts:184  aggregation_is_by_maximum_so_a_single_bad_channel_is_not_averaged_away
  × heatmap.test.ts:220  bands_of_unequal_width_keep_their_own_channel_extents_when_merged
  × artifacts: aggregated-greyscale.svg, aggregated-colour.svg
```

The **maximum-not-mean contract is protected** — but by `QM-0150`'s test, not by
this task's. `degradation.test.ts` alone passes 24/24 under the mean mutation.
See Defect B.

---

## AC1 — cell count never exceeds 250 000

Read of the arithmetic: `maxColumnsPerRow = floor(ceiling / rowCount)`;
`factor = ceil(bandCount / maxColumns)`; cells per row =
`ceil(bandCount / factor) <= maxColumns`; total `<= rowCount * floor(ceiling /
rowCount) <= ceiling`. Sound for all positive inputs.

Checked empirically over 117 shapes (rows ∈ {1, 2, 3, 7, 17, 99, 251, 999,
1000, 4999, 10007, 124999, 250000} × channels ∈ {1, 2, 3, 17, 999, 4097, 65537,
250001, 1000003}): worst total **250 000**, attained at 1000 × 999, never
exceeded. `rowCount > ceiling` throws rather than dropping rows
(`heatmap.ts:171`), with the message *"Nothing is truncated silently; narrow the
selection instead."* — a refusal, which is the repository's correct answer.
**AC1 met.**

Note that `MAX_HEATMAP_CELLS`, the aggregation arithmetic and the maximum rule
all shipped in `QM-0150`; this branch adds no ceiling logic. The evidence file
says so explicitly in its first section, which is the honest framing.

---

## Aggregation is by maximum — verified in the code

`heatmap.ts:216-219`:

```ts
// Maximum, not mean. `null` is absent, and absent never lowers a maximum.
if (band.relativeError !== null) {
  worst = worst === null ? band.relativeError : Math.max(worst, band.relativeError)
}
```

Verified in the arithmetic, and by Mutation 3 above, not only in the legend
text. A `null` band cannot pull a maximum down. Correct.

---

## Adjudication of the six disclosed departures

### 1. `render.ts` changed although not in `Files Expected to Change` — **JUSTIFIED, not scope creep**

AC2 and AC4 are about a mark a reader can see. `render.ts` is the only file in
the program boundary (`apps/web/diagnostics`) that emits geometry. Meeting the
criteria without touching it is impossible. The change is confined: one exported
marker constant, one `sampledWedge` helper, one attribute, one legend swatch
call, one legend line — no refactor, no drive-by. It stays inside the declared
`Program Boundary`.

Residual bookkeeping defect: `CLAUDE.md` says *"Before writing code, confirm
every path in the task's `Files Expected to Change` still exists. If one does
not, the plan is stale and fixing the plan takes precedence."* The converse case
— a file that must change and is not listed — was recorded in the evidence but
the task's own `Files Expected to Change` list was never corrected. See Defect E.

### 2. `channelsPerCell: number | null` rather than the spec's `number` — **SOUND, and `null` strengthens rather than weakens the contract**

`Cell.channelsPerCell` has been `number | null` since `QM-0150` for a real
reason: manifest v1's summary projection publishes one number per layer and no
shape. Narrowing `CellFidelity` to `number` would have forced the code to write
`1` where the manifest published nothing — an invented resolution, which is
exactly what `QError::NotImplemented` exists to prevent on the Rust side. `null`
here is the type saying "not published", and
`an_aggregated_cell_with_no_published_extent_says_so_rather_than_claiming_one_channel`
(`degradation.test.ts:210`) pins it.

This is the minimal widening. The spec's `number` was written before the summary
projection's shape was a known constraint; the departure fixes the spec, not the
code.

### 3. Legend entry kind is `engine-coarse`, not `sampled` — **CLAIM VERIFIED, but only for reader-visible strings**

I verified the claim empirically rather than reading the test. Building the
summary surface with `fidelity: 'approximate'`:

```
legend entries: ... fidelity: every value on this map is approximate
                    aggregated: cells with a dashed border aggregate more than one channel, by maximum (factor 1)
                    engine-coarse: a corner wedge marks approximate values from the engine; a dashed border marks columns the renderer merged
fidelityNote:   The numbers on this map are approximate, which describes how they were obtained. ...
cell attrs:     data-cell-fidelity="sampled"
```

Every reader-visible string says *approximate*. The legend entry label
interpolates `grid.fidelity`, so it cannot drift.
`an_approximate_engine_value_is_never_described_to_the_reader_with_the_word_sampled`
(`degradation.test.ts:233`) asserts `not.toMatch(/\bsampled\b/)` over
`surfaceStrings()` **and** every `<text>` node. The claim holds.

The renaming to `engine-coarse` was the right call and is well argued. It is
incomplete: the machine attribute was not renamed. See Defect A.

### 4. AC6 — **MET, and the implementer undersold it**

I scrutinised this hardest, as instructed. My conclusion is *more* favourable
than the implementer's own, and I want to be explicit about why, because
"the AC conflicts with an existing test" is the rationalisation shape.

AC6 turns on what "the unaggregated case" means. There are two readings and
**both are proved**:

* *Aggregation factor is 1* — `degradation.test.ts:389` plans the same 64 unit
  bands twice, once with exactly 64 columns of budget and once with 100 000, and
  asserts `toEqual` on the whole `Row`. That is the literal structural identity
  AC6 names, and it is non-vacuous.
* *No cell covers more than one channel* — `degradation.test.ts:397` asserts
  `grid.anyAggregated === false`, `cell.aggregated === false` and
  `cellFidelityOf(cell) === {kind:'exact'}` for every cell. `render.ts:401`'s
  ternary then emits `stroke-dasharray="none"`, so the Error Handling row's
  *"Aggregation factor of 1 → No marker"* is satisfied.

The only reading that fails is one that conflates *aggregation factor* (the
renderer's merge, a `Row`/`Grid` property) with *`Cell.aggregated`* (whether the
cell's number spans more than one channel) — two quantities the task's own
`Data Contracts` section keeps separate. A single band six channels wide at
factor 1 is genuinely an aggregate over six channels, and marking it is the more
honest behaviour; `heatmap.test.ts:234` fixed that deliberately in `QM-0150`.

So: **AC6 is met, no `QM-0150` test needed relaxing, and nothing was quietly
dropped.** I checked `git diff` to be sure `heatmap.test.ts` is byte-identical
to `main` — it is.

### 5. No screenshot; committed SVGs stand in — **HONEST, and the greyscale claim is partly asserted**

The evidence is unusually careful here. It states the SVGs "are not screenshots
and are not offered as such", names the reason (no browser, no headless
renderer), and does not smuggle the word "screenshot" back in anywhere. That
meets `CLAUDE.md`'s rule against claiming an unexercised capability.

What the artifacts actually support, verified by me:

* `sampled-greyscale.svg`: 3 `sampled-mark` paths (2 cells + 1 legend swatch),
  3 `stroke-dasharray="3 2"`. Both marks, greyscale palette.
* `sampled-experts-colour.svg`: 3 and 3. Both marks, colour palette.
* `aggregated-colour.svg` / `aggregated-greyscale.svg`: 0 wedges, 4 dashes —
  an `exact` run at `cellCeiling: 3`, factor 2.

So "both marks in both palettes" is supported. **"The mark is present in
greyscale and is a shape, not a hue" is supported. "The mark is *legible*" is
asserted by construction**, via the `tier >= 4` ink flip inherited from the
magnitude glyph. No human eye and no rasteriser confirmed perceptibility. That
is recorded under *Not verified*.

One further ceiling worth stating plainly: `artifacts.test.ts`'s SVGs are
goldens regenerated by the code under test (`QM_WRITE_ARTIFACTS=1`), which
`CLAUDE.md`'s *Tests* section explicitly disfavours as an oracle. They are
change-detectors, and my Mutation 3 showed they do detect change — that is their
real value here, and it is enough for what they are being asked to carry.

### 6. Canvas draws neither mark — **A REAL GAP, but pre-existing and correctly disclosed; not a blocker**

Confirmed by reading `paintHeatmap` (`render.ts`): it draws `clearRect`, the
cell fill, the magnitude bar and the selection stroke, and nothing else. No
dash, no wedge. `index.html` places `<canvas id="heatmap-canvas">` **above**
`<div id="diagnostics">`, and `present.ts:87-88` writes the full marked SVG into
the div and then paints the unmarked canvas. So the browser shows two renderings
of the same grid, the more prominent of which carries no aggregation or
coarseness mark.

Why this does not block:

* It is **pre-existing**. `paintHeatmap` never drew the `QM-0150` aggregation
  dash either. `git diff main..HEAD` touches no line of `paintHeatmap`. This
  branch neither creates nor widens the gap; it follows the existing convention.
* The marked surface is on screen — `present.test.ts:187` pins that
  `screen.root.innerHTML` contains `CELL_RECT_MARKER`, so the SVG (marks
  included) reaching the page is test-covered, not just source-read.
* It is disclosed in the evidence's *Not performed* section in plain terms
  ("that is a gap, not a completed surface").

AC2 is met on the SVG surface, which is the surface every test, every artifact
and every piece of evidence in this repository is written against.
**Recommendation to the controller:** open a follow-up task for `paintHeatmap`
to draw both marks (it will need the recording-context test harness extended
with `setLineDash` and path drawing). Per `CLAUDE.md`, a correction outside
`.plan/` is written as a task, not folded into this merge.

---

## Defects

**A. `render.ts:390` — an approximate run emits `data-cell-fidelity="sampled"`.**
Minor, inert, disclosed. Verified empirically: on a manifest with
`fidelity: 'approximate'`, every cell rect carries
`data-fidelity="approximate" data-cell-fidelity="sampled"`. A machine-readable
field naming an approximate figure "sampled" is the same class of error as
presenting a sampled figure as exact, which `CLAUDE.md` forbids. Mitigations: no
consumer exists — `grep -rn 'data-cell-fidelity' apps/web/diagnostics/src`
returns exactly one hit, the emitter itself; `data-fidelity` carries the truth
immediately beside it; `degradation.test.ts:244` pins that `data-fidelity` is
never `"sampled"` on such a run; and the evidence file calls it out rather than
leaving it to be found. The inconsistency is that the *legend kind* was renamed
`engine-coarse` for precisely this reason and the *attribute* was not. A future
task should rename it to `engine-coarse` for symmetry.

**B. `degradation.test.ts:374` `a_merged_cell_takes_the_worst_of_its_columns_and_not_their_average` does not bite.**
It survives Mutation 3 (running mean) — the whole file passes 24/24 under it.
The test compares the global maximum across the merged and unmerged grids, and
on the `full.v1.json` fixture the largest value does not land in the merged
cell, so a mean changes nothing it looks at. The maximum contract is genuinely
protected, by `heatmap.test.ts:184`, which does fail. So this is a test-quality
defect, not a hole in the guarantee — but the test's name promises a protection
it does not provide.

**C. `degradation.test.ts:397` name overpromises.**
`unit_wide_bands_at_factor_one_carry_no_aggregation_mark_and_no_aggregated_legend_entry`
never builds a `Surface` and never inspects `legend.entries`; it infers from
`grid.anyAggregated`. If `buildLegend` began pushing the aggregated entry
unconditionally, this test would still pass. Rename or extend.

**D. `render.ts:102` — the wedge has no minimum cell-size gate.**
`const size = Math.max(1, Math.min(SAMPLED_WEDGE, width, height))`. The
magnitude glyph is suppressed below `width >= 14 && height >= 12`
(`render.ts:416`); the wedge is not, so at the ceiling it degrades to a 1-pixel
triangle rather than being withheld. Not a false claim — the mark is present and
the fidelity word is still in the header and the legend — but inconsistent with
the neighbouring redundancy policy, and a 1px mark is arguably "present in the
file and absent to the eye", which is the failure the wedge's own doc comment
names.

**E. `TASK.md` `Files Expected to Change` still lists only `heatmap.ts` and
`app.ts`.** The `render.ts` departure is recorded in `.plan/evidence/QM-0153.md`
and in the task's `Completion Evidence` prose, but the file list itself was
never corrected. Bookkeeping only.

**F. Evidence wording, minor.** *"produced by the same `render.ts` draw plan the
browser's 2-D canvas consumes"* is loose — `paintHeatmap` is a separate code
path that shares `placeCells`, `fillOf` and `encodeMagnitude` but not the SVG
emitter. The evidence corrects itself two bullets later ("the 2-D canvas painter
draws neither mark"), so a reader is not left misled, but the first phrasing on
its own overstates the coupling.

---

## Repository-rule compliance

* **Never claim an unexercised capability** — met. The screenshot was not taken
  and is not claimed; the ceiling arithmetic is credited to `QM-0150` rather
  than to this task; the canvas gap is stated as a gap.
* **Every result labelled exact / sampled / approximate** — met on every
  reader-visible surface, verified empirically on an `approximate` run.
  Qualified only by Defect A at the DOM-attribute level.
* **Never present a sampled figure as exact** — met. `data-fidelity` always
  carries the manifest's own word.
* **Data-plane doc comment** — no new module was added. `heatmap.ts` and
  `render.ts` already carry theirs, and both were extended with `QM-0153`-cited
  paragraphs rather than replaced.
* **No network, no GPU, no large fixtures in tests** — met; everything runs off
  checked-in JSON fixtures. The 1 000 × 65 536 test explains in a comment why it
  drives `planRow` per row rather than `buildGrid`, to bound peak residency —
  the right instinct in this repository.
* **Commit hygiene** — five commits, all `type(scope): description [QM-0153]`.
  Test commit precedes implementation commit.
* **Floor only ever raised** — met; 336 → 361, 21 → 22, in the implementation
  commit.

---

## Not verified

* **Perceptual legibility of the wedge.** I confirmed it is emitted, that it is
  a `<path>` and not a hue, that it is byte-identical across palettes, and that
  its ink flips to `#ffffff` over the dark end of the ramp. I did not rasterise
  any SVG and no human looked at one. "Legible in greyscale" is supported by
  construction and by the shape-vs-outline distinction of kind; it is not
  supported by observation.
* **Browser behaviour.** No page was loaded. The claim that the marked SVG
  appears on screen rests on `present.ts:87` plus `present.test.ts:187`, not on
  a running browser.
* **The 1px-wedge case (Defect D) at real ceiling dimensions.** I reasoned it
  from `render.ts:102`; I did not render a 250 000-cell SVG to see it.
* **`buildSurface` throwing on > 250 000 rows.** `present()` calls
  `buildSurface` without a `try`, so `maxColumnsPerRow`'s refusal would escape
  as an uncaught exception rather than a rendered refusal. I did not trace
  whether `main.ts` or the manifest schema bounds layer count first. Pre-existing
  to `QM-0150`, and it needs a checkpoint with a quarter-million layers to reach,
  so I did not pursue it.
* **Per-file vitest JSON counts.** I did not re-derive the implementer's
  per-file table. The `git diff --stat` over `__tests__/` — one new file, one
  additive `+6/-0` — is a stronger proof of the same property and is what I
  relied on.

---

## Summary

The one criterion this task calls important is the one best proved. AC5's guard
survives adversarial mutation in two independent forms, including the subtle
off-by-one that would be invisible on screen. AC1 holds across a 117-shape sweep
and by argument. Aggregation is by maximum in the code, and a mean is caught.
The floor reconciles exactly and no pre-existing test was so much as reformatted.
The failing-first commit is real and reproduces to the assertion.

The departures are all defensible, and the evidence file is one of the more
honest in this repository — it volunteers the canvas gap and the missing
screenshot rather than letting a reviewer find them. On AC6 the implementer was
more pessimistic about their own work than the facts warrant: the criterion is
met on both readings that respect the spec's own vocabulary.

Merge. Open a follow-up for `paintHeatmap` (departure 6) and fold Defects A–D
into whichever task next touches these files.
