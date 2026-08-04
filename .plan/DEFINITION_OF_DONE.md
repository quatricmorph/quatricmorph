# DEFINITION_OF_DONE — the 46 MVP acceptance criteria

Criteria `MVP-01` … `MVP-46` are the task specification §33 list, in order, given
stable IDs. The prefix is new deliberately: `STATUS.md` already uses `AC-001` …
`AC-010` for `ARCHITECTURE.md` §18's ten criteria, and reusing that range would
collide.

**Status column** is copied from `STATUS.md` and is authoritative there, not here.
Where a criterion is already satisfied, the "Task" column names a **verification**
task, not an implementation task — the work is done and what remains is proving
it still holds at release.

| Legend | |
| --- | --- |
| ✅ | Already satisfied and covered by a passing test today |
| 🟡 | Partially satisfied — the data model or policy exists, the surface does not |
| ⬜ | Not yet satisfied |
| 🔧 | Requires an RTX 3090 |

---

## Branding and product

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-01` | The application is branded as Quatricmorph | 🟡 | `QM-0050`, `QM-0090` | Header reads "Quatricmorph — Trillion-Scale Tensor Visualization"; no product surface presents itself as `mm`. Rust and web package names already say Quatricmorph |

## Ingestion

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-02` | A local SafeTensors file can be opened | ✅ | `QM-0001` | `SRC-004`, `ingests_a_single_file_checkpoint`; `q-cli inspect fixtures/tiny-llama-single` |
| `MVP-03` | A sharded checkpoint can be indexed | ✅ | `QM-0001` | `SRC-003`, `ingests_a_sharded_checkpoint`; `AC-002` |
| `MVP-04` | Indexing does not load the complete checkpoint into RAM | ✅ | `QM-0001` | `SRC-007`, `ingestion_reads_only_headers_not_payload`; `AC-001` |
| `MVP-05` | A synthetic trillion-parameter manifest indexes in bounded memory | ✅ | `QM-0013` | `CAT-006`: 47 278 tensors, 1.048×10¹² parameters, 2.10 TB described, **35.7 MB peak**, no artifact opened |
| `MVP-06` | Model, layer, module, tensor, and block metadata can be browsed | 🟡 | `QM-0021`, `QM-0055` | Catalog side ✅ (`CAT-003`); block rows need `QM-0021`; a browsable UI needs `QM-0055` |
| `MVP-07` | Tensor names map to stable canonical addresses | ✅ | `QM-0001` | `NSIR-004`, `canonical_names_are_stable_across_resolution_runs` |
| `MVP-08` | Unknown semantic roles remain unknown rather than guessed | ✅ | `QM-0010` | `NSIR-001`, `generic_resolver_returns_unknown_for_names_it_was_not_taught`. Re-asserted for the new Qwen resolver |
| `MVP-09` | Selected tensor blocks can be read by byte range | ✅ | `QM-0030` | `SRC-005`, `TILE-002`, `slice_read_matches_golden_and_reads_only_the_window` |

## GPU compute — 🍎 Metal in v1, 🔧 CUDA is the next step

`MVP-10` … `MVP-12` are the task specification §33 items and are written
against CUDA/RTX 3090 verbatim. **v1 ships Metal, not CUDA**
(`ADR-CANDIDATE-003`, `Decided`): the conversion-stage compute plugin behind
`q_gpu::Backend` is implemented and verified on Apple GPU hardware for v1,
and these three criteria are satisfied for v1 through that Metal path plus
the pre-existing waiver below. CUDA itself is deferred to the named next-step
task (`.plan/CUDA_ARCHITECTURE.md`) and is not scheduled for v1.

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-10` | 🔧 CUDA processing runs on an RTX 3090 (spec-literal; v1 satisfies the intent via Metal, see waiver) | ⬜ | `QM-0034`, `QM-0035` (CUDA, post-v1); new Metal task (v1) | Kernel output from the device — Metal on Apple GPU for v1 (`nvidia-smi` equivalent evidence for CUDA once the next step lands) |
| `MVP-11` | GPU processing uses bounded block buffers | 🟡 | new Metal task (v1); `QM-0034` (CUDA, post-v1) | `CUDA-006` verifies the ceiling arithmetic **without a device**; the v1 Metal backend verifies enforcement on Apple GPU hardware |
| `MVP-12` | 🔧 GPU results are validated against CPU references (spec-literal CUDA wording; v1 validates the Metal backend against CPU) | ⬜ | new Metal task (v1); `QM-0035`, `QM-0036` (CUDA, post-v1) | Differential test output — Metal vs `CpuBackend` for v1, at tolerances analogous to [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) §6 |

**CUDA/RTX 3090 itself is out of v1 scope by plan, not by hardware accident.**
`MVP-10` and `MVP-12`'s literal CUDA wording takes a written waiver for v1: the
CUDA code path is deferred (not merely unverified), `STATUS.md` records
`Hardware-Unverified` / `Not Started` as appropriate, the documentation claims
nothing more, and `QM-0092` states the limitation. `MVP-11` is satisfied via
the v1 Metal backend's enforcement path. See [`RISK_REGISTER.md`](RISK_REGISTER.md) R3.

## Conversion artifacts

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-13` | Conversion produces versioned `.qtile` artifacts | 🟡 | `QM-0041` | Format ✅ (`TILE-005`…`TILE-008`); generation is `TILE-004`. Files on disk with `version: 1` headers |
| `MVP-14` | Conversion produces valid GLB tile content | ⬜ | `QM-0042`, `QM-0046` | Khronos `gltf-validator` passes on generated tiles |
| `MVP-15` | Conversion produces a valid `tileset.json` | ⬜ | `QM-0044`, `QM-0046` | `3d-tiles-validator` passes; schema validation passes |
| `MVP-16` | Generated work can be cancelled and resumed | 🟡 | `QM-0033`, `QM-0045` | Ingestion ✅ (`SRC-009`, `SRC-010`); conversion needs the executor. A killed job resumes and produces byte-identical output |
| `MVP-17` | Completed block artifacts are reused from cache | 🟡 | `QM-0032` | L1/L2 ✅ (`CACHE-001`…`CACHE-004`); wiring is `CACHE-008`. A second run reports cache hits and skips compute |

## Viewer

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-18` | CesiumJS loads the generated tileset | ⬜ | `QM-0051` | Screenshot plus a Playwright assertion that tiles rendered |
| `MVP-19` | CesiumJS performs camera-based LOD loading | 🟡 | `QM-0052` | Policy ✅ (`CESIUM-002`); wiring needed. Request log shows refinement as the camera approaches |
| `MVP-20` | Zooming out does not load exact scalar data | ✅ policy / ⬜ wired | `QM-0052` | `never_reads_exact_values_from_camera_movement_alone`; plus a request log from a real navigation session showing no exact reads |
| `MVP-21` | Selecting a visual feature resolves to the correct tensor or block | 🟡 | `QM-0053` | Addressing ✅ (`GRID-001`, `NSIR-004`); picking needed. `AC-004`. A click's resolved address equals `q-cli`'s for the same index |
| `MVP-22` | Clicking or querying a scalar returns the correct exact value | ✅ API / ⬜ UI | `QM-0053`, `QM-0080` | `API-002`, `exact_value_route_returns_the_golden_scalar`; end to end through the viewer |
| `MVP-23` | The exact value matches a Python SafeTensors reference | ✅ | `QM-0080` | `AC-005`: `tests/tests/end_to_end_scalar_slice.rs`, 4 tests over 6 golden scalars, 2 golden slices, 2 bf16 tensors, against `safetensors==0.8.0` |
| `MVP-24` | The UI distinguishes aggregate, sampled, quantized, approximate, and exact | 🟡 | `QM-0054` | `AC-010` is `Partial`: the data model carries fidelity end to end and is verified; **no UI renders it**. Screenshots of every badge state |

## Matrix workspace

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-25` | A selected tensor block can be opened in the matrix workspace | ⬜ | `QM-0066` | `GRID-004`. Block fetched from the daemon and rendered, with fidelity shown |
| `MVP-26` | Tensors align to the shared 3D grid ruler | ✅ locally / ⬜ shared | `QM-0060` | `GRID-001`, `GRID-002` ✅; `GRID-006` makes it shared with the viewer |
| `MVP-27` | Matrix, row vector, column vector, and scalar layouts use one coordinate system | 🟡 | `QM-0065` | Placement ✅; framing and labelling of rank-0 and rank-1 needs completion |
| `MVP-28` | Compatible matrix blocks can be multiplied | 🟡 | `QM-0067`, `QM-0070` | Pure matmul ✅ (`MATMUL-001`, 17 tests); real blocks need `QM-0066`; server-side execution is `WQL-006` |
| `MVP-29` | Incompatible shapes are rejected before CUDA execution | ✅ | `QM-0001` | `WQL-004`, `AC-009`. The planning engine has no `ModelSource`, so it **cannot** read even if it wanted to |
| `MVP-30` | The multiplication path can be animated deterministically | ✅ pure / ⬜ wired | `QM-0067` | `MATMUL-003`, 7 tests on the pure schedule; wired to real blocks |
| `MVP-31` | Play, pause, step, previous, and reset work | 🟡 | `QM-0067` | Schedule ✅; the control surface needs building. `previous` must be exactly `forward`'s inverse |

## Query and chat

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-32` | A user can query a canonical tensor address | ✅ | `QM-0001` | `WQL-003`, `CAT-004` |
| `MVP-33` | A user can query aliases such as `Q[10]` | ✅ | `QM-0001` | `NSIR-005`, `alias_and_canonical_references_resolve_to_the_same_tensor` |
| `MVP-34` | Ambiguous aliases return candidate tensors | ✅ | `QM-0075` | `NSIR-007`, `API-007`. The UI must present candidates and not choose |
| `MVP-35` | A user can submit a slice query | ✅ | `QM-0001` | `WQL-005`, `scalar_and_slice_queries_execute_and_are_labelled_exact` |
| `MVP-36` | A user can submit a constrained matrix expression | ✅ plan / ⬜ execute | `QM-0070` | Planning ✅ (`WQL-002`, `explicit_transpose_makes_the_expression_type_check`); execution is `WQL-006` |
| `MVP-37` | Mathematical expressions render with KaTeX | ✅ | `QM-0075` | `CHAT-003`, `renders_the_grouping_the_parser_chose_not_the_source_order`. Plus the sanitization contract |
| `MVP-38` | Query cost is estimated before expensive execution | ✅ plan / 🟡 UI | `QM-0073` | `WQL-010` ✅; the cost card and the confirmation gate need building |
| `MVP-39` | Queries can be cancelled | ⬜ | `QM-0073` | Cancellation acknowledged; latency bounded by one block |
| `MVP-40` | Chat uses WeightQL and cannot directly access arbitrary checkpoint bytes | ⬜ | `QM-0074` | `CHAT-001`. Architectural: chat's only output is a WeightQL string. Asserted by a test that chat has no `ModelSource` and no fetch to a byte route |

## Robustness

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-41` | Repeated selection and re-initialization do not leak browser memory | ⬜ | `QM-0082` | 100 iterations; heap and `renderer.info.memory` return within 10 % of baseline |
| `MVP-42` | 🔧 Repeated GPU block jobs do not leak device memory (spec-literal CUDA wording; v1 verifies the Metal backend, CUDA verification is the next-step task) | ⬜ | new Metal task (v1); `QM-0083` (CUDA, post-v1) | 10 000 jobs; Metal device memory (Apple `os_signpost`/Instruments or `q_gpu` accounting) returns to its start value for v1. `cudaMemGetInfo` evidence lands with the CUDA next step |
| `MVP-43` | The browser console contains no unresolved runtime errors | ⬜ | `QM-0085` | Console capture across the full manual checklist, empty |

## Documentation and licensing

| ID | Criterion | Now | Task | Evidence required |
| --- | --- | --- | --- | --- |
| `MVP-44` | The original license and attribution are preserved | ✅ | `QM-0093` | `mm/LICENSE` unmodified; `apps/web/quatricmorph-workspace/LICENSE` + `NOTICE.md` present; `mm/` untouched per `AGENTS.md` |
| `MVP-45` | Documentation accurately describes implemented capabilities and limitations | 🟡 | `QM-0090`, `QM-0091`, `QM-0092` | `STATUS.md` regenerated from a real run; no row more favourable than its evidence; `ARCHITECTURE.md` §8.2 divergence resolved |
| `MVP-46` | The product does not claim one RTX 3090 can hold or fully compute a 10¹²-parameter model | ✅ | `QM-0094` | Text audit across `README.md`, `ARCHITECTURE.md`, `STATUS.md`, `.plan/`, and every UI string. `STATUS.md` already states: *"Trillion-scale means metadata … It proves nothing about loading weights, because that is not possible and is not claimed anywhere."* |

---

## Tally

| State | Count |
| --- | --- |
| ✅ Already satisfied and tested | 17 |
| 🟡 Partially satisfied | 15 |
| ⬜ Not yet satisfied | 14 |
| 🔧 Of which are spec-literal CUDA/RTX 3090 wording, satisfied for v1 via Metal instead | 3 (`MVP-10`, `MVP-12`, `MVP-42`) |

**Forty-three of forty-six are achievable with no NVIDIA hardware, and v1 ships
all forty-six with zero CUDA** — the three CUDA-worded criteria above are met
through the Metal backend and a written waiver on the literal CUDA/RTX 3090
wording. That is the plan's central scheduling fact, and it is why CUDA sits
off the critical path and is deferred to the next step after v1.

---

## Release gate

The MVP ships when:

1. Every criterion above is ✅ or carries a **written waiver** naming the reason,
   the requirement ID, and the task that would close it.
2. `cargo test --workspace` and `npx vitest run` pass with **no failures and no
   newly ignored tests**. The baseline is 290 + 101; the release count must
   exceed it.
3. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -D warnings`
   are clean.
4. The §32 end-to-end demonstration runs **from a clean checkout on a machine
   with no NVIDIA GPU**.
5. Generated artifacts pass external validation — `gltf-validator`,
   `3d-tiles-validator`, and the 3D Tiles schema.
6. `STATUS.md` is regenerated from that run and contains no row whose status is
   more favourable than its evidence.
7. The manual checklist in [`TEST_STRATEGY.md`](TEST_STRATEGY.md) §5 is complete
   with screenshots.
8. No document in the repository claims a capability the tests do not demonstrate.

Criterion 8 is the one that matters most, and it is the reason `STATUS.md` exists
in the form it does. A plan can be optimistic; a release cannot.
