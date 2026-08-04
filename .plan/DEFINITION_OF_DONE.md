# DEFINITION_OF_DONE — v1

Criteria `V1-01` … `V1-32` define v1. They replace `MVP-01` … `MVP-46` as the
release gate; that older list is preserved in §7 with each item's disposition,
because it is the acceptance list for the **deferred platform release** and will
be needed again.

**Status is copied from [`../STATUS.md`](../STATUS.md) and is authoritative there,
not here.** Where a criterion is already satisfied, the Task column names a
*verification* task — the work is done, and what remains is proving it still
holds at release.

| Legend | |
| --- | --- |
| ✅ | Already satisfied and covered by a passing test today |
| 🟡 | Partially satisfied — the data model or policy exists, the surface does not |
| ⬜ | Not yet satisfied |
| 👤 | Cannot be closed by writing code |

---

## 1. The out-of-core claim

This is the section that separates Quatricmorph from a script. Every criterion
here is **measured**, never asserted.

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `V1-01` | A real open-weight SafeTensors checkpoint of **≥ 24 GB on disk** is acquired, header-verified, and indexed | ⬜ | `QM-0100` | `du -sh` output; `q-cli inspect` listing tensor count, dtypes, and total parameters; the checkpoint's source URL and revision hash |
| `V1-02` | Indexing that checkpoint reads **headers only** | ✅ mechanism / ⬜ at scale | `QM-0100` | `SRC-007`; bytes-read counter over the real checkpoint, expected ≪ 0.1 % of file size |
| `V1-03` | **Peak resident bytes ≤ 1.25 × a configured ceiling `C ≤ 2 GB`** while streaming the whole checkpoint | ⬜ | `QM-0101` | `/usr/bin/time -l` maximum resident set size, alongside the checkpoint size and the ratio `N = size / C` |
| `V1-04` | `N ≥ 100` — the checkpoint is at least 100× the resident ceiling | ⬜ | `QM-0101` | Arithmetic printed in the run-metadata block of the report |
| `V1-05` | Peak residency is **flat in checkpoint size** — the same ceiling holds at three sizes | ⬜ | `QM-0101` | Three measured runs: the tiny fixture, `models/distilbert-distilgpt2` (339 MB), and the `V1-01` checkpoint |
| `V1-06` | Streaming is cancellable and resumable at a block boundary; a resumed run produces byte-identical output | 🟡 | `QM-0033` | Ingestion side ✅ (`SRC-009`, `SRC-010`); the conversion side needs the job runner. Kill mid-run, resume, `diff` the manifests |
| `V1-07` | Completed block work is reused from cache on a second run | 🟡 | `QM-0032` | L1/L2 ✅ (`CACHE-001`…`CACHE-004`); wiring is `CACHE-008`. Second run reports cache hits and a materially shorter wall clock |

### Waiver — checkpoint size

**The development machine has 51 GB of free disk.** v1's headline checkpoint is
therefore capped at roughly 30–40 GB. The strategy document's 1.5 TB frontier-MoE
example is **not provable on this hardware**, and v1 makes no claim about it.

* What v1 claims: bounded residency, measured, on a checkpoint ≥ 24 GB — the
  reference ceiling for a 24 GB-class GPU.
* What v1 does not claim: that a 1.5 TB checkpoint has been streamed.
* What closes the gap: external NVMe, or the NVIDIA Inception credits
  ([`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §2). Neither is a v1 blocker.
* What may **never** substitute for it: `CAT-006`, the synthetic 10¹² manifest.
  It proves metadata scale and is silent about streaming real bytes. Any document
  that lets it stand in for `V1-03` is wrong.

---

## 2. Numerical correctness

The engine is the product. A wrong number is worse than a missing one.

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `V1-08` | Quantisation simulation (RTN int8/int4; per-tensor, per-channel, per-group; symmetric and asymmetric) matches an independent Python/NumPy reference | ⬜ | `QM-0120` | Golden tensors checked in; agreement exact where the arithmetic is exact, within a stated tolerance otherwise |
| `V1-09` | Every error metric — ‖W−Ŵ‖_F, RMSE, max\|Δ\|, mean\|Δ\|, relative error — matches the same reference | ⬜ | `QM-0122` | Per-metric comparison table in the task's completion evidence |
| `V1-10` | Streaming aggregation equals whole-tensor computation | ⬜ | `QM-0122` | The `STAT-004` pattern (`streaming_in_chunks_equals_computing_at_once`) extended to paired metrics |
| `V1-11` | Per-output-channel error vectors are correct and correctly oriented | ⬜ | `QM-0121` | A hand-computed asymmetric fixture where transposing the axis would change the answer |
| `V1-12` | Aggregation to module, layer, expert, and model is exact, not re-derived from rounded values | ⬜ | `QM-0123` | Sums of squares propagate; a test asserts equality with direct computation |
| `V1-13` | Results are deterministic across runs and across backends | ⬜ | `QM-0122`, `QM-0127` | Two CPU runs byte-identical; CPU vs. Metal within the tolerance in [`DIAGNOSTIC_ARCHITECTURE.md`](DIAGNOSTIC_ARCHITECTURE.md) §4.3 |
| `V1-14` | The Metal backend is differentially verified against the CPU reference on Apple GPU hardware | ⬜ | `QM-0127` | Device name, per-metric max deviation, tolerance cited |
| `V1-15` | Unsupported dtypes and schemes are refused, never approximated | ✅ pattern | `QM-0120` | `SRC-014` (`fp8_refuses_rather_than_approximates`) extended to `QuantScheme` |

---

## 3. The artifact

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `V1-16` | A run produces a **versioned JSON manifest** covering every tensor examined | ⬜ | `QM-0140` | Schema in `schemas/diagnostics/`; the manifest validates against it |
| `V1-17` | A run produces a **Markdown report** readable without the tool | ⬜ | `QM-0141` | The report itself, checked in as a golden |
| `V1-18` | The report is **deterministic**: same checkpoint + same config → byte-identical output | ⬜ | `QM-0141`, `QM-0142` | Two runs, `cmp` returns 0. Run metadata (timestamps, wall clock, host) confined to one clearly delimited block |
| `V1-19` | Changing one config parameter produces a **readable `git diff`** | ⬜ | `QM-0142` | The diff itself, in the task's evidence: int8 → int4 should change numbers, not reflow the document |
| `V1-20` | The report names the **fragile layers** and the **mixed-precision frontier** | ⬜ | `QM-0125`, `QM-0141` | A ranked table; a frontier table of (bytes added, aggregate error removed) for each candidate keep-set |
| `V1-21` | The report states the measured peak RSS, checkpoint size, backend, and elapsed time | ⬜ | `QM-0141` | The run-metadata block |
| `V1-22` | The report contains **no accuracy prediction** and carries the weight-space caveat | ⬜ | `QM-0141`, `QM-0090` | String audit against [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §5.2 |
| `V1-23` | CI and coding agents can consume results without the UI | ⬜ | `QM-0143` | `quatricmorph diagnose --fail-above 0.05` returns a non-zero exit code; `GET /v1/diagnostics/{runId}` returns the same manifest the CLI wrote |

---

## 4. The surface

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `V1-24` | A heat-map renders layer × channel error from a real manifest | ⬜ | `QM-0150`, `QM-0152` | Screenshot of the `V1-01` checkpoint |
| `V1-25` | **Legibility**: a reader who has not seen the tool names the three most fragile layers from one screenshot, unprompted | ⬜ 👤 | `QM-0151` | Written account of at least three attempts, including any that failed |
| `V1-26` | Above the rendering ceiling the surface degrades to an aggregate cell **and says so** | ⬜ | `QM-0153` | Screenshot of the degraded state with its label |
| `V1-27` | Magnitude is legible without colour alone | ⬜ | `QM-0150` | Greyscale screenshot in which the ranking is still readable |
| `V1-28` | No unresolved runtime errors in the browser console across the manual checklist | ⬜ | `QM-0085` | Console capture, empty |

---

## 5. Validation — the criteria that decide whether v1 mattered

These come from the strategy document §10 and are ordered by its priority. They
cannot be closed by writing code, and no amount of engineering substitutes for
them.

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `V1-29` | **A design partner runs it on a checkpoint the founder did not choose** | ⬜ 👤 | `QM-0161` | Dated account: who, which model, what they found, what surprised them |
| `V1-30` | **A documented case where the output changed a real engineering decision** | ⬜ 👤 | `QM-0162` | The decision before, the output, the decision after, in the partner's own words where possible |
| `V1-31` | Repeated use — the same user returns across weeks, not one session | ⬜ 👤 | `QM-0164` | Session log with dates |
| `V1-32` | A willingness-to-pay signal — a pilot, a card, or an explicit budget statement | ⬜ 👤 | `QM-0163` | The artifact of the probe |

**`V1-30` is the release gate that matters.** Everything above it is
infrastructure for producing it. The strategy is blunt about the alternative:
GitHub stars and demo-video views are explicitly *not* success metrics, and two of
these four missing by month 6 is a kill signal, not a reason to build module 2.

---

## 6. Engineering hygiene

| ID | Criterion | Now | Task | Evidence |
| --- | --- | --- | --- | --- |
| `V1-H1` | `cargo test --workspace` and `npx vitest run` pass, above the 290 + 101 baseline, no newly ignored tests | ✅ baseline | `QM-0001`, `QM-0165` | Test counts with the commands above them |
| `V1-H2` | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -D warnings` clean | ✅ | `QM-0165` | Command output |
| `V1-H3` | `STATUS.md` regenerated from a real run; no row more favourable than its evidence | 🟡 | `QM-0091` | The regenerated file |
| `V1-H4` | Original `mm` license and Meta Platforms attribution intact | ✅ | `QM-0093` | `mm/LICENSE` unmodified; workspace `LICENSE` + `NOTICE.md` present |
| `V1-H5` | Root documents amended so `ARCHITECTURE.md`, `MASTER_DOCUMENT.md`, and `.plan/` agree on what v1 is | ⬜ | `QM-0167` | The amended sections, and the removal of the precedence note in `.plan/README.md` |
| `V1-H6` | No document or UI string claims a capability the tests do not demonstrate | 🟡 | `QM-0090`, `QM-0092` | Audit across `README.md`, `ARCHITECTURE.md`, `STATUS.md`, `.plan/`, report templates, and every UI string |

---

## 7. Disposition of the previous 46 criteria

Preserved so nothing is silently dropped. `MVP-01`…`MVP-46` remain the acceptance
list for the **deferred platform release**.

| Range | Criteria | Disposition |
| --- | --- | --- |
| `MVP-01` | Branding | **Carried into v1** — folded into `V1-H6` and `QM-0090` |
| `MVP-02`…`MVP-09` | Ingestion, sharding, bounded indexing, canonical addresses, byte-range reads | **Carried into v1** — mostly ✅ already; `V1-01`…`V1-05` strengthen them from synthetic to real data |
| `MVP-10`…`MVP-12` | CUDA on an RTX 3090 | **Deferred.** v1 satisfies the intent through the Metal lane (`V1-14`); CUDA remains the post-v1 accelerator. The written waiver in the previous revision stands |
| `MVP-13`…`MVP-15` | `.qtile` pyramid, GLB, `tileset.json` | **Deferred** — platform release |
| `MVP-16`, `MVP-17` | Cancel/resume, cache reuse | **Carried into v1** as `V1-06`, `V1-07` |
| `MVP-18`…`MVP-24` | Cesium viewer, LOD, picking, exactness badges | **Deferred** — platform release. `MVP-24`'s intent (fidelity is visible, never implied) survives as `V1-22` and `V1-26` |
| `MVP-25`…`MVP-31` | Matrix workspace, grid ruler, animated matmul | **Deferred** — platform release |
| `MVP-32`…`MVP-37` | WeightQL queries, aliases, slices, KaTeX | **Already ✅** and retained; no v1 task depends on them and none removes them |
| `MVP-38`, `MVP-39` | Cost preview, cancellation | **Partly carried** — cancellation is `V1-06`; the cost-preview UI is deferred with the query surface |
| `MVP-40` | Chat cannot read bytes directly | **Deferred** — there is no chat in v1 |
| `MVP-41`…`MVP-43` | Memory soaks, console cleanliness | **Carried** — `V1-28`; the browser soak shrinks to match the smaller surface |
| `MVP-44`…`MVP-46` | Licensing, documentation honesty, no trillion-parameter claim | **Carried into v1** as `V1-H4`, `V1-H6`, and the §1 waiver |

---

## 8. Tally

| State | Count |
| --- | --- |
| ✅ Already satisfied and tested | 4 |
| 🟡 Partially satisfied | 5 |
| ⬜ Not yet satisfied | 23 |
| 👤 Of which cannot be closed by writing code | 5 |

Thirty-two criteria against the previous forty-six, and the five that cannot be
engineered are the ones the strategy says decide whether any of this was worth
building.

---

## 9. Release gate

v1 ships when:

1. Every `V1-*` criterion is ✅ or carries a **written waiver** naming the reason,
   the requirement ID, and the task that would close it.
2. `V1-30` — the documented decision-change case — is satisfied **without a
   waiver**. It is the one criterion that may not be waived; waiving it means
   shipping a demo and calling it a product.
3. The headline run completes from a clean checkout on a machine with no NVIDIA
   GPU, and its measured numbers appear in the report rather than in a claim.
4. `STATUS.md` is regenerated from that run and contains no row whose status is
   more favourable than its evidence.
5. No document in the repository claims a capability the tests do not demonstrate.

Criterion 5 is the one that matters most, and it is why `STATUS.md` exists in the
form it does. A plan can be optimistic; a release cannot.
