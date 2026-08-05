# REPOSITORY_ANALYSIS — what the repository actually contains

Everything in §1–§7 was **verified by running a command or reading a file** in
this repository. §8 separates that from assumption and recommendation, as the
task specification §6 requires.

---

## 0. A correction to the task's premise

The task specification names the repository as `./mm` and asks for a plan that
avoids "a superficial rebranding of the existing `mm` matrix visualization
application". That premise is a generation behind the repository.

`mm/` is **one read-only directory** inside a much larger tree. The
redesign it warns against has already happened: `mm` was analysed symbol by
symbol in `docs/CURRENT_ARCHITECTURE.md`, ported to TypeScript under
`apps/web/quatricmorph-workspace/`, and surrounded by an 18-crate Rust workspace
(`ls crates | wc -l` → 18) implementing SafeTensors ingestion, canonical
addressing, a metadata catalog, WeightQL, a `.qtile` format, a cache, a
diagnostics manifest, and a local HTTP daemon.

This plan is therefore a **delta plan**. It plans the work that remains. Where
the task specification asks for something already built and tested, the plan
records the evidence and schedules a verification task rather than manufacturing
an implementation task.

---

## 1. Verified build and test baseline

Measured at commit `1d49ffa` (`QM-0002` rebased onto `main` at `eca5a6a`). Every
number in this table is re-measured whenever it is quoted; the commit above is
the one the run was made against, not the last commit that edited this file.

| Command | Result |
| --- | --- |
| `cargo test --workspace` | **exit 0** — `434 passed; 0 failed; 0 ignored`, summed over 43 `test result:` lines (43 test binaries) |
| `npx vitest run` (from `apps/web`) | **exit 0** — `115 passed (115)`, 13 files |
| `./scripts/verify-baseline.sh` | **exit 0** — at floor: rust 434/43, web 115/13 |

**These numbers move with every merge, and this table has been wrong twice for
that reason.** The readings and what closed the gap each time:

| Read at | Rust | Web | What changed |
| --- | --- | --- | --- |
| `ace7d09` | `290` / 39 | `101` (12 files) | the original run |
| `4e0e85c` | `318` / 39 | `115` (13 files) | `QM-0006` restored the 9 web test files `103297d` had silently de-collected and added `workspace-paths.test.ts`; `QM-0012` added Rust tests |
| `1d49ffa` | **`434`** / **43** | `115` (13 files) | `QM-0140` added `crates/q-report` (97 tests over 2 binaries) and `QM-0100` added `tests/tests/real_checkpoint_record.rs`; `QM-0001` raised the recorded floor to match |

The reading above is **machine-checked** rather than trusted to prose: at this
branch's base `eca5a6a`, `scripts/baseline.json` records `rust_tests: 434`,
`rust_binaries: 43`, `web_tests: 115`, `web_files: 13`, and
`./scripts/verify-baseline.sh` exits 0 reporting every one of them "at floor".

**That floor is itself a moving number, and this document does not own it.** `main`
has already raised it past this branch's base — `4bddf6c` pins `rust_tests: 502`
over the same 43 binaries after `QM-0010` and `QM-0020` merged — and the controller
reconciles the value at each merge. Read `scripts/baseline.json` for the current
floor; the numbers quoted here are the ones true at `eca5a6a`.

**`STATUS.md:9-10` still claims `290` and `101 (12 files)` and is behind the
tree** — that is `QM-0091`'s to regenerate, not this document's to assert.
Registered as `DIV-009`.

Test distribution measured from the vitest run:

| File | Tests |
| --- | --- |
| `query-interface/src/__tests__/weightql.test.ts` | 17 |
| `quatricmorph-workspace/src/math/__tests__/matmul.test.ts` | 17 |
| `quatricmorph-workspace/src/util/__tests__/workspace-paths.test.ts` | 14 |
| `quatricmorph-workspace/src/layout/__tests__/grid-ruler.test.ts` | 13 |
| `quatricmorph-workspace/src/math/__tests__/blocking.test.ts` | 10 |
| `quatricmorph-workspace/src/viz/__tests__/array2d.test.ts` | 9 |
| `quatricmorph-workspace/src/math/__tests__/animation-schedule.test.ts` | 7 |
| `quatricmorph-workspace/src/interaction/__tests__/interaction.test.ts` | 6 |
| `model-viewer/src/__tests__/lod-policy.test.ts` | 6 |
| `model-viewer/src/__tests__/tile-client.test.ts` | 4 |
| `quatricmorph-workspace/src/viz/__tests__/expr.test.ts` | 4 |
| `quatricmorph-workspace/src/util/__tests__/params.test.ts` | 3 |
| `quatricmorph-workspace/src/tensor/__tests__/block-adapter.test.ts` | 5 |

These sum to 115 across 13 files. `STATUS.md` was accurate for its `2026-08-03`
run and was adopted as this plan's factual baseline on that basis; the tree has
since moved ahead of it under `QM-0006` and `QM-0012`. The baseline discipline is
unchanged — the numbers above are what the commands print today, and `QM-0091`
regenerates `STATUS.md` to match.

### Confirmed current commands

```bash
# Rust
cargo test --workspace
cargo build --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# CLI (crates/q-cli, binary name `q`)
cargo run -p q-cli -- inspect fixtures/tiny-llama-2shard
cargo run -p q-cli -- layers  fixtures/tiny-llama-2shard
cargo run -p q-cli -- tensors fixtures/tiny-llama-2shard
cargo run -p q-cli -- value   fixtures/tiny-llama-2shard 'Q[10]' --index 100,42
cargo run -p q-cli -- slice   fixtures/tiny-llama-2shard <address> --rows a:b --columns c:d
cargo run -p q-cli -- query   fixtures/tiny-llama-2shard 'show tensor("Q[10]") @ transpose(tensor("K[10]"))'
cargo run -p q-cli -- stats   fixtures/tiny-llama-2shard <address> --rows 100:104 --columns 40:44

# Daemon
cargo run -p q-daemon -- --model-root fixtures/tiny-llama-2shard

# Web (npm workspaces rooted at apps/web)
cd apps/web && npm install
npx vitest run
npm run build --workspace quatricmorph-workspace
npm run dev  --workspace quatricmorph-workspace     # vite
npm run dev  --workspace model-viewer         # vite, shell only

# Fixtures
python3 -m venv .venv && .venv/bin/pip install numpy safetensors
.venv/bin/python fixtures/generate_fixtures.py
```

CI (`.github/workflows/build.yaml`) runs three jobs: `rust` (fmt, clippy, build,
test), `fixtures` (regenerate and `git diff --exit-code`), and `web` (vitest plus
a `quatricmorph-workspace` build uploaded as an artifact). It contains **no CUDA job**,
with a comment explaining that a job which "passed" without an RTX 3090 would be
worse than no job.

---

## 2. Repository layout, as it exists

Every label below is re-measured at `1d49ffa`. A written-down number in a tree
diagram cannot carry its own deriving command, so the commands are given after the
diagram and this whole block is expected to drift; it is re-measured, not trusted.

```text
ARCHITECTURE.md              1376 lines — implementation SoT, untouched
STATUS.md                     279 lines — 131 requirement rows from a real run
AGENTS.md                      47 lines — agent rules; mm/ is read-only
README.md                     142 lines
MASTER_DOCUMENT.md           1054 lines
Cargo.toml                             — workspace, 19 members (18 crates + tests), edition 2021, rust-version 1.78
crates/                        18 crates, 18 692 lines of Rust over 46 .rs files
gpu/cuda/                       4 .cu files + README (HARDWARE-UNVERIFIED)
gpu/metal/compute.metal                — placeholder
gpu/wgsl/compute.wgsl                  — placeholder
apps/web/                      npm workspaces: quatricmorph-workspace, model-viewer, query-interface
architectures/                 generic, llama (implemented); qwen, kimi, deepseek (declared)
schemas/                        5 JSON schemas — nsir 116, qtile 93, visualization 119, weightql 166,
                               diagnostics/manifest.v1.json 592 lines (added by QM-0140)
fixtures/                      tiny-llama-single (108 KB), tiny-llama-2shard (1.2 MB) + golden.json,
                               real-checkpoint-record.json (added by QM-0100)
tests/                         cross-crate integration: end_to_end_scalar_slice.rs,
                               real_checkpoint_record.rs
docs/                          13 ADRs, requirements, roadmap, testing, mm evidence record
python/                        binding scaffold
mm/                            READ-ONLY historical reference (5216 lines across 5 files)
target/                        build output — machine-local, not a repository fact
```

Derivations, so no reader has to trust the numbers above:

```bash
wc -l ARCHITECTURE.md STATUS.md AGENTS.md README.md MASTER_DOCUMENT.md
grep -cE '^\| [A-Z][A-Z0-9]*-[0-9]+ \|' STATUS.md      # STATUS requirement rows
ls crates | wc -l                                       # crates
find crates -name '*.rs' | xargs wc -l | tail -1        # Rust lines
find schemas -name '*.json' | wc -l                     # JSON schemas
ls docs/decisions/*.md | wc -l                          # ADRs
wc -l mm/*.js mm/*.html                                 # mm/, 5 files
```

`mm/` has exactly one commit in its history (`c7b1f7e`, "feat: add mm") and
`AGENTS.md` makes it read-only, so `5216` is stable. The **file count was wrong**:
§3 nine lines below already enumerated five files — `viz.js`, `index.html`,
`gui.js`, `util.js` **and** `ref.html` — while the label above said four. The
label, not §3, was the defect.

### Crate inventory, by line count

All **18** crates, re-measured at `1d49ffa`. `Lines` is the crate's total `.rs`
line count and `Files` its `.rs` file count, both from one command per crate:

```bash
for c in crates/*/; do
  printf '%s %s %s\n' "$(basename "$c")" \
    "$(find "$c" -name '*.rs' | xargs wc -l | tail -1 | awk '{print $1}')" \
    "$(find "$c" -name '*.rs' | wc -l)"
done | sort -k2 -rn
```

The previous revision of this table gave hand-copied per-file breakdowns
(`673+640+321+40`) for 17 crates. Every one of them had drifted as the crates
grew, and `q-report` was missing entirely, so the column is now a single
re-derivable total. Sum: **18 692** lines over **46** files.

| Crate | Lines | Files | Plane | State |
| --- | --- | --- | --- | --- |
| `q-report` | 3034 | 3 | Diagnostics | v1 manifest schema, round-trip validated (`QM-0140`) |
| `q-source` | 2270 | 11 | Artifact | Working |
| `q-catalog` | 2104 | 4 | Metadata | Working; SQLite (`ADR-003`) |
| `q-weightql` | 1935 | 5 | Metadata | Parses, plans, executes reads |
| `q-nsir` | 1688 | 5 | Metadata | Working |
| `q-safetensors` | 1469 | 5 | Artifact | Working |
| `q-daemon` | 1087 | 2 | — | 8 live routes, 5 × 501 |
| `q-cache` | 714 | 1 | — | L1/L2 working, unwired |
| `q-cli` | 672 | 1 | — | 7 subcommands |
| `q-architecture` | 647 | 1 | Metadata | Plugin registry |
| `q-expression` | 606 | 1 | Metadata | Closed AST |
| `q-tiles` | 566 | 1 | Tensor Tile | `.qtile` v1 complete |
| `q-statistics` | 481 | 1 | Tensor Tile | Computes; never persisted |
| `q-tensor-runtime` | 478 | 1 | Metadata | LOD ladder, block planning |
| `q-gpu` | 363 | 1 | — | CPU backend is the reference |
| `q-cuda` | 221 | 1 | — | Refuses every operation; a legitimate crate per `ADR-007` |
| `q-tileset` | 195 | 1 | Visualization | **Refuses to emit** |
| `q-gltf` | 162 | 1 | Visualization | **Refuses to emit** |

---

## 3. `mm/` — the historical reference

`mm/` is 5 216 lines across `viz.js` (2 105), `index.html` (839), `gui.js` (380),
`util.js` (365), plus `ref.html` (1 527), vendored Three.js / lil-gui, assets, an
intro article, and `LICENSE` (MIT, Meta Platforms, Inc.).

**The symbol-level analysis the task specification §6 requires already exists**
at `docs/CURRENT_ARCHITECTURE.md` (305 lines). It records, per symbol: current
file, line range, responsibility, dependencies, problems, and a reuse decision —
across `Array2D`, `Mat`, `MatMul`, initialization, multiplication, scene
creation, placement, value→colour and value→size mapping, row guides, flow
guides, text labels, camera setup and fitting, `OrbitControls`, hover, selection,
animation, GUI state, URL serialization, state compression, and disposal. It
tallies 4 reuse-as-is, ~45 extract, ~20 extract-and-refactor, 9 deprecate.

It also records six defects found by reading, and one security finding: `mm`
reaches `eval` from a URL parameter (`mm/viz.js:119-126` via
`mm/index.html:531`). That path is **not** carried into
`apps/web/quatricmorph-workspace` and is the origin of the closed-expression design in
`q-expression` (`ADR-006`).

This plan does not repeat that analysis. It cites it, and its Phase 00 task
`QM-0002` re-validates the citations still resolve.

**Verified consequence:** the port is real, not aspirational. `mm/viz.js`'s
`grid`, `dotprod`, `ikjmul`, `scatterFromCount`, and the `getVmprodBump` /
`getMvprodBump` / `getVvprodBump` cursors now exist as pure, separately tested
modules at `apps/web/quatricmorph-workspace/src/math/{blocking,matmul,animation-schedule}.ts`
with 34 tests between them.

---

## 4. Verified gaps

Read directly from `STATUS.md` and confirmed against the source. These are the
MVP's actual work.

### Nothing renders

* `q_tileset::UnimplementedTilesetBuilder` — test name:
  `the_builder_refuses_rather_than_emitting_a_fake_tileset` (`CESIUM-001`)
* `q_gltf::UnimplementedGlbBuilder` — `the_builder_refuses_rather_than_emitting_a_placeholder_glb` (`GLB-001`)
* No tile pyramid is ever generated (`TILE-004`, Not Started)
* `apps/web/model-viewer/` is `index.ts` + `lod-policy.ts` + `tile-client.ts`.
  Its `package.json` description says so: *"app shell only; tileset rendering is
  not built (CESIUM-001)"* (`CESIUM-005`, Not Started)

### Nothing computes on a GPU

* `gpu/cuda/{reduce,histogram,matmul,quantize}.cu` — `gpu/cuda/README.md`:
  *"None has been compiled, linked, or executed. There is no `nvcc` step in the
  build, no `build.rs`, and no FFI binding"*
* `q_cuda::CudaBackend` returns `NotImplemented` for every operation;
  `q_cuda::KERNEL_SOURCES` lists the four files as data, not as a build input
* Only the VRAM *ceiling arithmetic* is tested, explicitly "without a device"

### Computed but not persisted, generated, or wired

* Statistics: `q-statistics` and `q_gpu::block_statistics_default` work;
  `tensor_statistics` is empty and `GET /v1/tensors/{id}/statistics` returns 501
  (`STAT-002`)
* Cache: L1 and L2 pass 4 requirement rows; **no query path calls them**
  (`CACHE-008`, Not Started)
* Jobs: the state machine and persistence are tested; **nothing executes a job**
  (`JOB-002`)
* Matmul: plans, resolves, type-checks, costs — then stops (`WQL-006`)

### Product surfaces missing

* Chat (`CHAT-001`, deliberately not built)
* Qwen / Kimi / DeepSeek resolvers (`NSIR-006`; declared with
  `implemented = false`, and a test asserts they never claim a model)
* Statistical `SELECT … GROUP BY` (`WQL-007`); stacked slices (`WQL-008`)
* Live tensor-block adapter (`GRID-004`, stub that "refuses rather than returning
  plausible zeros")
* HTTP Range transport (`SRC-008`; range arithmetic is verified, transport is not)

---

## 5. The three-spatial-authority problem

Not recorded in `STATUS.md`; found by reading. This is the most consequential
structural finding in the analysis, because it silently gates Phases 04, 05, and
06 at once.

**The LOD ladder and geometric-error rule are implemented three times, in two
languages, with no test that they agree.**

| Authority | Location | What it defines |
| --- | --- | --- |
| Rust runtime | `q_tensor_runtime::Lod` (`crates/q-tensor-runtime/src/lib.rs:35`) | The 6-level ladder, `carries_exact_values`, `access_scale`, parent/child |
| Rust visualization | `q_tileset::GeometricError::for_lod` (`crates/q-tileset/src/lib.rs:46`) | `ROOT_GEOMETRIC_ERROR / 2^lod`, `ROOT_GEOMETRIC_ERROR = 1024.0` |
| TypeScript viewer | `apps/web/model-viewer/src/lod-policy.ts:20,51,102` | Its **own** `enum Lod`, `LOD_DISTANCE_THRESHOLDS = [4096,1024,256,64,16]`, and `geometricErrorForLod = 1024 / 2 ** lod` |

The TypeScript comment says *"mirrors `q_tileset::GeometricError`"* — hand-mirrored,
by a human, with no mechanism to detect drift. Change `ROOT_GEOMETRIC_ERROR` in
Rust and the viewer silently refines at the wrong distance.

A fourth authority is about to appear: `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts`
holds the ten grid parameters (`cellSize`, `minorGridSpacing`, `majorGridInterval`,
`tensorPadding`, `labelMargin`, `framePadding`, `operandGap`, `axisMargin`,
`depthSpacing`, `origin`) that the product requirement says must be *shared
across all visualizations and mathematical operations*. Today they are local to
one web package and unknown to Rust and to the viewer.

`schemas/visualization/schema.json` exists (119 lines) and already fixes the
`tileset_node`, `visual_tile_row`, `bounding_box`, and `glb_tile_spec` shapes,
with a stated purpose: *"so the daemon, the catalog, and the viewer cannot drift
while the builders are written."* It does **not** yet carry the grid parameters,
the LOD ladder, or the geometric-error rule.

**Consequence for the plan:** `QM-0004` extends that schema into the single
spatial contract, and `QM-0005` adds the cross-language conformance test. Both
are Phase 00 and both gate Lanes A, B, and C. This is the shared-schema blocker
the task specification §36 asks to be identified.

---

## 6. Fixture ceiling

Verified by reading `fixtures/tiny-llama-2shard/config.json` and `golden.json`:

```json
{ "hidden_size": 48, "intermediate_size": 64, "num_hidden_layers": 12,
  "num_attention_heads": 8, "vocab_size": 64, "torch_dtype": "float32" }
```

111 tensors, 2 shards, **1 196 736 bytes total**. The largest tensor is
`model.layers.10.self_attn.q_proj.weight` at `[128, 48]` — 6 144 elements.

This fixture is excellent for what it was built for: exact-value equality against
Python `safetensors`, sharding, cancellation, resume, and stable IDs. It is
**too small to exercise the MVP's visual pipeline**. A 128×48 tensor cannot be
decomposed into 256×256 blocks, cannot produce a five-level LOD pyramid, and
cannot demonstrate that zooming out avoids exact reads — there is nothing to zoom
out from.

`fixtures/generate_fixtures.py` exists and CI already asserts the fixtures are
reproducible from it. **`QM-0003` extends it** with a larger, generated,
**not git-committed** fixture (one tensor of at least 4096×4096, which is 64 MiB
at f32) plus its golden values, and `.gitignore` coverage. This is a hard
prerequisite for Phases 04, 05, and 08.

---

## 7. Licensing and attribution — verified intact

| File | Content |
| --- | --- |
| `mm/LICENSE` | MIT, Meta Platforms, Inc. Unmodified |
| `apps/web/quatricmorph-workspace/LICENSE` | The same MIT text, reproduced |
| `apps/web/quatricmorph-workspace/NOTICE.md` | Attribution for the derivation |
| `Cargo.toml` `workspace.package.license` | `MIT OR Apache-2.0` |
| `apps/web/quatricmorph-workspace/package.json` | `"license": "MIT"`, description names `mm` |

`AGENTS.md` marks `mm/` read-only. No task in this plan modifies it.
`QM-0093` audits this at release.

---

## 8. Assumptions and recommendations — **not** verified facts

Separated per the task specification §6. Nothing below is asserted as true of the
repository.

### Assumptions this plan makes

1. **The development machine has no NVIDIA GPU.** The environment is darwin /
   Apple silicon; CI is `ubuntu-latest` with no CUDA job. If an RTX 3090 becomes
   available, Lane E unblocks; the critical path does not change.
2. **`STATUS.md`'s `2026-08-03` run is representative.** Corroborated by
   re-running both suites at the same counts, but only on this platform.
3. **CesiumJS is not yet a dependency.** `apps/web/model-viewer/package.json`
   declares only `typescript`, `vite`, `vitest`. Adding `cesium` is a real
   dependency decision with a bundle-size cost — `ADR-CANDIDATE-010`.
4. **`target/` at 4.3 GB is build output**, not tracked content. Not verified
   against `.gitignore` line by line.
5. **`python/quatricmorph/` is a scaffold.** Its two files were listed but not
   read in depth; no plan task depends on it.

### Recommendations

1. Treat `STATUS.md`, not this plan, as the state of the world. Regenerate it
   from a real run at every release (`QM-0091`).
2. Do not add a fourth spatial authority. Land `QM-0004`/`QM-0005` before any
   Phase 04–06 task.
3. Keep the refuse-with-a-requirement-ID idiom for every new gap. It is why this
   repository's claims can be trusted, and it is cheaper than a placeholder that
   later has to be found and removed.
4. Generate the large fixture; do not commit it. CI's reproducibility job already
   establishes the pattern.
5. Resolve the `ARCHITECTURE.md` §8.2 plane-mapping divergence
   (see [`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) §6) by decision, not
   by drift.
