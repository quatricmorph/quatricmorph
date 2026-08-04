# REQUIREMENT_TRACEABILITY

## 0. Numbering policy

**This plan extends `STATUS.md`'s requirement IDs. It does not create a parallel
scheme.**

`STATUS.md` already uses the prefixes the task specification §35 asks for —
`SRC-`, `NSIR-`, `CAT-`, `WQL-`, `STAT-`, `TILE-`, `GLB-`, `CESIUM-`, `GRID-`,
`MATMUL-`, `CACHE-`, `GPU-`, `CUDA-`, `API-`, `SEC-`, `CHAT-`, `JOB-`, `AC-` —
across 129 rows, and it already documents reconciling two numbering schemes
(`TILE-*` from `docs/requirements/VIZ_MVP.md` and `PLAT-*` from
`docs/requirements/MVP_REQUIREMENTS.md`). A third scheme would be actively
harmful.

New requirements therefore take **the next free number in the existing prefix**:

| Prefix | Highest in `STATUS.md` | New IDs allocated here |
| --- | --- | --- |
| `SRC-` | 018 | `SRC-019` |
| `NSIR-` | 009 | `NSIR-010` |
| `CAT-` | 010 | `CAT-011`, `CAT-012` |
| `WQL-` | 012 | `WQL-013` |
| `STAT-` | 007 | `STAT-008` |
| `TILE-` | 008 | `TILE-009`…`TILE-012` |
| `GLB-` | 003 | `GLB-004` |
| `CESIUM-` | 005 | `CESIUM-006`…`CESIUM-013` |
| `GRID-` | 005 | `GRID-006`…`GRID-012` |
| `MATMUL-` | 005 | `MATMUL-006` |
| `CUDA-` | 006 | `CUDA-007`…`CUDA-009` |
| `API-` | 008 | `API-009`…`API-012` |
| `SEC-` | 005 | `SEC-006`…`SEC-009` |
| `CHAT-` | 004 | `CHAT-005` |
| `PERF-` | — (new prefix) | `PERF-001`…`PERF-004` |
| `DOC-` | — (new prefix) | `DOC-001`…`DOC-005` |
| `MVP-` | — (new prefix) | `MVP-01`…`MVP-46` — the §33 acceptance criteria |

`MVP-` is a genuinely new prefix rather than an extension of `AC-`, because
`AC-001`…`AC-010` are `ARCHITECTURE.md` §18's ten criteria and renumbering them
would break `STATUS.md`'s existing cross-references.

**`Now` columns below are copied from `STATUS.md` and are authoritative there.**
Where the two disagree, `STATUS.md` wins and this document is corrected
([`RISK_REGISTER.md`](RISK_REGISTER.md) R11).

---

## 1. Already verified — verification tasks only

102 requirement rows in `STATUS.md` are `Verified`. They need no implementation.
What they need is confirmation that they still hold at release, which is one
task, not 102:

| Requirements | Verification task | Documentation task |
| --- | --- | --- |
| `SRC-001`…`SRC-007`, `SRC-009`…`SRC-018` | `QM-0001` | `QM-0091` |
| `NSIR-001`…`NSIR-005`, `NSIR-007`…`NSIR-009` | `QM-0001` | `QM-0091` |
| `CAT-001`…`CAT-009` | `QM-0001` | `QM-0091` |
| `WQL-001`…`WQL-005`, `WQL-009`…`WQL-012` | `QM-0001` | `QM-0091` |
| `STAT-001`, `STAT-003`…`STAT-007` | `QM-0001` | `QM-0091` |
| `TILE-001`…`TILE-003`, `TILE-005`…`TILE-008` | `QM-0001` | `QM-0091` |
| `GLB-002`, `GLB-003` | `QM-0001` | `QM-0091` |
| `CESIUM-002`, `CESIUM-003`, `CESIUM-004` | `QM-0001` | `QM-0091` |
| `GRID-001`, `GRID-002`, `GRID-005` | `QM-0001` | `QM-0091` |
| `MATMUL-001`…`MATMUL-005` | `QM-0001` | `QM-0091` |
| `CACHE-001`…`CACHE-004` | `QM-0001` | `QM-0091` |
| `GPU-001`, `GPU-002` | `QM-0001` | `QM-0091` |
| `CUDA-006` | `QM-0001` | `QM-0091` |
| `API-001`…`API-008` | `QM-0001` | `QM-0091` |
| `SEC-001`…`SEC-005` | `QM-0001`, `QM-0094` | `QM-0091` |
| `CHAT-002`, `CHAT-003`, `CHAT-004` | `QM-0001` | `QM-0091` |
| `JOB-001`, `JOB-003` | `QM-0001` | `QM-0091` |
| `AC-001`, `AC-002`, `AC-003`, `AC-005`, `AC-007`, `AC-008`, `AC-009` | `QM-0001`, `QM-0080` | `QM-0091` |

`QM-0001` runs both suites, records the counts, and fails if any of these
regresses. Several are additionally re-exercised by the end-to-end run in
`QM-0080`.

---

## 2. Gaps in existing requirements

| ID | Requirement | Now | Implementation | Verification | Doc |
| --- | --- | --- | --- | --- | --- |
| `SRC-008` | HTTP Range transport | Stub | *extension point — not MVP* | — | `QM-0092` |
| `NSIR-006` | Qwen / Kimi / DeepSeek resolvers | Not started | `QM-0010` (**Qwen only**) | `QM-0011` | `QM-0090` |
| `CAT-010` | DuckDB / Arrow / Parquet backend | Not started | *out of scope — `ADR-003`* | — | `QM-0092` |
| `WQL-006` | Matmul execution | Stub | `QM-0070` | `QM-0080` | `QM-0090` |
| `WQL-007` | Statistical `SELECT … GROUP BY` | Not started | `QM-0072` | `QM-0072` | `QM-0090` |
| `WQL-008` | Stacked slice composition | Stub | `QM-0071` | `QM-0071` | — |
| `STAT-002` | Statistics persisted and served | Stub | `QM-0020` | `QM-0080` | — |
| `TILE-004` | Tile pyramid generation | Not started | `QM-0041` | `QM-0046` | `QM-0092` |
| `GLB-001` | GLB tile-content generation | Stub | `QM-0042` | `QM-0046` | — |
| `CESIUM-001` | `tileset.json` generation | Stub | `QM-0044` | `QM-0046` | — |
| `CESIUM-005` | A viewer that renders | Not started | `QM-0050`, `QM-0051` | `QM-0080` | `QM-0090` |
| `CACHE-005` | L0 GPU cache | Not started | *extension point* | — | `QM-0092` |
| `CACHE-006` | L3 browser cache | Stub | *extension point* | — | `QM-0092` |
| `CACHE-007` | L4 remote cache | Stub | *extension point* | — | `QM-0092` |
| `CACHE-008` | Cache wired into the query path | Not started | `QM-0032` | `QM-0081` | — |
| `GPU-003` | Metal backend (v1 GPU compute lane); wgpu remains *extension point* | Not started | `QM-0037` + new Metal tasks (v1) | — | `QM-0092` |
| `CUDA-001` | CUDA backend implements the trait (next step, post-v1) | 🔧 Unverified | `QM-0034` | `QM-0035` 🔧 | `QM-0092` |
| `CUDA-002` | Reduction kernels (next step, post-v1) | 🔧 Unverified | `QM-0034` | `QM-0035` 🔧 | `QM-0092` |
| `CUDA-003` | Histogram kernel (next step, post-v1) | 🔧 Unverified | `QM-0034` | `QM-0035` 🔧 | `QM-0092` |
| `CUDA-004` | Tiled block matmul (next step, post-v1) | 🔧 Unverified | `QM-0034` | `QM-0036` 🔧 | `QM-0092` |
| `CUDA-005` | Quantization / Morton kernels (next step, post-v1) | 🔧 Unverified | `QM-0034` | `QM-0036` 🔧 | `QM-0092` |
| `CHAT-001` | Chat assistant | Not started | `QM-0074` | `QM-0080` | `QM-0090` |
| `JOB-002` | Job runner | Stub | `QM-0033` | `QM-0081` | — |
| `AC-004` | Click resolves to the correct address | Partial | `QM-0053` | `QM-0080` | — |
| `AC-006` | Zooming out loads no exact values | Verified policy | `QM-0052` | `QM-0080` | — |
| `AC-010` | UI distinguishes fidelity | Partial | `QM-0054` | `QM-0094` | — |

---

## 3. New requirements

| ID | Requirement | Implementation | Verification | Doc |
| --- | --- | --- | --- | --- |
| `SRC-019` | LOD-capable generated fixture with golden values | `QM-0003` | `QM-0003` | `QM-0092` |
| `NSIR-010` | Model-level metadata from `config.json` | `QM-0012` | `QM-0012` | — |
| `CAT-011` | `models.hidden_size` / `layer_count` / `parameter_count` populated | `QM-0012` | `QM-0012` | — |
| `CAT-012` | `visual_tiles` written; tile↔tensor resolution both ways | `QM-0021` | `QM-0021` | — |
| `CAT-013` | `tensor_blocks` populated by a conversion | `QM-0022` | `QM-0081` | — |
| `WQL-013` | Execution tier selection recorded in the plan | `QM-0073` | `QM-0073` | `QM-0090` |
| `STAT-008` | Statistics pass over a whole tensor, persisted | `QM-0031` | `QM-0080` | — |
| `TILE-009` | Bounded streaming block reader with named budgets | `QM-0030` | `QM-0030` | — |
| `TILE-010` | LOD ladder and block-layout planner; bounds containment | `QM-0040` | `QM-0040` | — |
| `TILE-011` | Atomic output and resume manifests | `QM-0045` | `QM-0081` | — |
| `TILE-012` | External artifact validation in CI | `QM-0046` | `QM-0046` | — |
| `GLB-004` | Feature IDs and structural metadata | `QM-0043` | `QM-0046` | — |
| `CESIUM-006` | Cesium initialized with GIS features disabled | `QM-0050` | `QM-0050` | — |
| `CESIUM-007` | Feature pick → canonical address | `QM-0053` | `QM-0080` | — |
| `CESIUM-008` | Exactness badges in the UI | `QM-0054` | `QM-0094` | — |
| `CESIUM-009` | Hierarchy, breadcrumbs, search by address and alias | `QM-0055` | `QM-0055` | — |
| `CESIUM-010` | glTF extension capability probe and fallback profiles | `QM-0057` | `QM-0057` | `QM-0092` |
| `CESIUM-011` | Implicit tiling seam preserved in the node type | `QM-0044` | `QM-0044` | — |
| `CESIUM-012` | Visual encoding applied viewer-side, not baked into the GLB | `QM-0042` | `QM-0046` | — |
| `CESIUM-013` | Camera fit / reset / presets, URL state, full disposal | `QM-0056` | `QM-0082` | — |
| `GRID-006` | One spatial contract consumed by Rust and both web apps | `QM-0004`, `QM-0060` | `QM-0005` | `QM-0090` |
| `GRID-007` | Axis binding; rank ≤ 3; rank > 3 refuses with this ID | `QM-0061` | `QM-0061` | `QM-0092` |
| `GRID-008` | Ruled-grid rendering: minor, major, origin, axis labels | `QM-0062` | `QM-0062` | — |
| `GRID-009` | Sphere-block cells; value → scale, colour, opacity | `QM-0063` | `QM-0063` | — |
| `GRID-010` | Sphere budget and documented degradation | `QM-0064` | `QM-0064` | `QM-0092` |
| `GRID-011` | Cross-language spatial conformance test | `QM-0005` | `QM-0005` | — |
| `GRID-012` | Hover/selection metadata; never colour-only | `QM-0068` | `QM-0068` | — |
| `MATMUL-006` | Real-block `A @ B` with the full control set | `QM-0067` | `QM-0080` | — |
| `CUDA-007` | `nvcc` build integration, feature-gated (next step, post-v1) | `QM-0034` | `QM-0034` | `QM-0092` |
| `CUDA-008` | Differential verification against the CPU reference (next step, post-v1) | `QM-0035`, `QM-0036` | `QM-0035` 🔧 | `QM-0092` |
| `CUDA-009` | Device-memory leak soak (next step, post-v1) | `QM-0083` | `QM-0083` 🔧 | — |
| `API-009` | Job routes: create, status, cancel, resume | `QM-0033` | `QM-0081` | — |
| `API-010` | SSE progress route | `QM-0033` | `QM-0081` | — |
| `API-011` | Query cancellation, acknowledged | `QM-0073` | `QM-0073` | — |
| `API-012` | Cache inspection and clearing routes | `QM-0032` | `QM-0032` | — |
| `SEC-006` | KaTeX sanitization contract | `QM-0075` | `QM-0075` | — |
| `SEC-007` | Daemon origin policy, bind address, request limits | `QM-0075` | `QM-0085` | — |
| `SEC-008` | CSP for both web applications | `QM-0050` | `QM-0085` | — |
| `SEC-009` | Security audit at release | `QM-0094` | `QM-0094` | `QM-0092` |
| `CHAT-005` | Candidate resolution UI; never a silent pick | `QM-0075` | `QM-0075` | — |
| `PERF-001` | Conversion peak RSS independent of tensor size | `QM-0031` | `QM-0031` | `QM-0092` |
| `PERF-002` | Viewer frame time and heap budgets | `QM-0052` | `QM-0082` | `QM-0092` |
| `PERF-003` | Workspace render budgets at 65 536 and 262 144 cells | `QM-0063` | `QM-0064` | `QM-0092` |
| `PERF-004` | Benchmark harness with reproducible reports | `QM-0084` | `QM-0084` | `QM-0092` |
| `DOC-001` | `README.md` reflects reality | `QM-0090` | `QM-0094` | — |
| `DOC-002` | `STATUS.md` regenerated from a real run | `QM-0091` | `QM-0094` | — |
| `DOC-003` | CUDA requirements, dtypes, limitations documented | `QM-0092` | `QM-0094` | — |
| `DOC-004` | Attribution and license audit | `QM-0093` | `QM-0093` | — |
| `DOC-005` | `ARCHITECTURE.md` §8.2 divergence resolved | `QM-0090` | `QM-0094` | — |

---

## 4. §33 acceptance criteria → tasks

Full text, evidence, and waiver policy in
[`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md). Summary map:

| Criterion | Tasks | | Criterion | Tasks |
| --- | --- | --- | --- | --- |
| `MVP-01` | `QM-0050`, `QM-0090` | | `MVP-24` | `QM-0054` |
| `MVP-02` | `QM-0001` | | `MVP-25` | `QM-0066` |
| `MVP-03` | `QM-0001` | | `MVP-26` | `QM-0060` |
| `MVP-04` | `QM-0001` | | `MVP-27` | `QM-0065` |
| `MVP-05` | `QM-0013` | | `MVP-28` | `QM-0067`, `QM-0070` |
| `MVP-06` | `QM-0021`, `QM-0055` | | `MVP-29` | `QM-0001` |
| `MVP-07` | `QM-0001` | | `MVP-30` | `QM-0067` |
| `MVP-08` | `QM-0010` | | `MVP-31` | `QM-0067` |
| `MVP-09` | `QM-0030` | | `MVP-32` | `QM-0001` |
| `MVP-10` 🔧 | `QM-0034`, `QM-0035` | | `MVP-33` | `QM-0001` |
| `MVP-11` | `QM-0034` | | `MVP-34` | `QM-0075` |
| `MVP-12` 🔧 | `QM-0035`, `QM-0036` | | `MVP-35` | `QM-0001` |
| `MVP-13` | `QM-0041` | | `MVP-36` | `QM-0070` |
| `MVP-14` | `QM-0042`, `QM-0046` | | `MVP-37` | `QM-0075` |
| `MVP-15` | `QM-0044`, `QM-0046` | | `MVP-38` | `QM-0073` |
| `MVP-16` | `QM-0033`, `QM-0045` | | `MVP-39` | `QM-0073` |
| `MVP-17` | `QM-0032` | | `MVP-40` | `QM-0074` |
| `MVP-18` | `QM-0051` | | `MVP-41` | `QM-0082` |
| `MVP-19` | `QM-0052` | | `MVP-42` 🔧 | `QM-0083` |
| `MVP-20` | `QM-0052` | | `MVP-43` | `QM-0085` |
| `MVP-21` | `QM-0053` | | `MVP-44` | `QM-0093` |
| `MVP-22` | `QM-0053`, `QM-0080` | | `MVP-45` | `QM-0090`…`QM-0092` |
| `MVP-23` | `QM-0080` | | `MVP-46` | `QM-0094` |

**No acceptance criterion is unmapped.** 46 of 46 have at least one task.

---

## 5. Coverage audit

| Check | Result |
| --- | --- |
| Every §33 criterion maps to ≥ 1 task | ✅ 46/46 |
| Every new requirement has an implementation task | ✅ |
| Every new requirement has a verification task | ✅ |
| Every task maps to ≥ 1 requirement | ✅ — asserted per task in `Requirements Covered` |
| Every `Verified` requirement has a regression guard | ✅ `QM-0001` |
| Every extension point is marked *not MVP* | ✅ — `SRC-008`, `CAT-010`, `CACHE-005`…`007` (`GPU-003`/Metal is v1 scope, no longer an extension point — see `ADR-CANDIDATE-003`) |
| Requirements requiring an RTX 3090 are marked 🔧 and deferred post-v1 | ✅ — `CUDA-001`…`005`, `CUDA-008`, `CUDA-009`, `MVP-10`, `MVP-12`, `MVP-42` (next step, not v1) |
| No requirement ID collides with `STATUS.md` | ✅ — new IDs start above each prefix's maximum |
