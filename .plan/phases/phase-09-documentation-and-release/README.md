# Phase 09 — Documentation and release

## Goal

```text
A reproducible local MVP, documented so that nothing claims more than it does
```

## Why this phase is not a formality

This repository's distinguishing property is that its documents can be trusted.
`STATUS.md` marks five requirements `Hardware-Unverified` with the test column
reading *"none — never compiled or executed"*. `gpu/cuda/README.md` says of its
own kernels: *"Treat every performance or numerical claim below as an intention,
not a measurement."* `README.md` has a section titled "What does not work yet".

That property is easy to lose in the release that finally makes things work. This
phase exists to keep it.

## Entry conditions

* **G4** passed — the end-to-end demonstration runs.
* Phases 00–08 complete.
* `ADR-009` accepted, so the `ARCHITECTURE.md` §8.2 correction has a recorded
  rationale. **Satisfied 2026-08-04.**

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0090` | Documentation update and divergence resolution | Documentation | `DOC-001`, `DOC-005`, `MVP-01`, `MVP-45` |
| `QM-0091` | Regenerate `STATUS.md` from a real run | Documentation | `DOC-002`, `MVP-45` |
| `QM-0092` | CUDA requirements, dtypes, and limitations | Documentation | `DOC-003`, `MVP-45` |
| `QM-0093` | Attribution and license audit | Verification | `DOC-004`, `MVP-44` |
| `QM-0094` | MVP acceptance audit against all 46 criteria | Verification | `MVP-46`, `SEC-009`, all |

## `QM-0090` — the divergence resolution

The one place this plan sanctions editing `ARCHITECTURE.md` **§8.2**, and only
under a recorded ADR — now
[`ADR-009`](../../../docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md):

`ARCHITECTURE.md` §8.2 says `A: XY, B: YZ, C: XZ`. The code implements, and the
task specification independently states, a mapping that resolves to `A: YZ,
B: XZ, C: XY`. Two sources agree against one; 13 tests hold the code's version.

**§8.2 is corrected**, with a note that the change is a documentation fix
recorded by an ADR, not a design change. `AGENTS.md` instructs agents to follow
`ARCHITECTURE.md` and *"fix or remove the conflicting text"* — leaving it wrong
would eventually cause someone to implement the wrong mapping and break 13 tests.

## `QM-0091` — regenerating `STATUS.md`

From a **real run**, at the release commit, with the commands and counts recorded
at the top exactly as the current document does.

Rules:

* **No row may be more favourable than its evidence.**
* `Verified` requires a named, passing test.
* `Hardware-Unverified` stays for every `CUDA-*` requirement no RTX 3090 has run.
* Stubs that are still stubs stay stubs.
* The "What a reader should not be surprised by" section is rewritten to match
  what is then true.

## `QM-0092` — limitations

Documented plainly, not buried:

* What "trillion-scale" means here: metadata and addressing under bounded memory.
  Never loading a trillion parameters anywhere.
* **An RTX 3090 has 24 GB. It cannot hold a trillion-parameter model, and this
  product does not claim it can.** With the arithmetic: ~0.5 % at f32.
* Supported dtypes: f32, bf16, f16. **fp8 refuses rather than approximating.**
* Which resolvers exist: generic, Llama, Qwen. Kimi and DeepSeek are declared and
  never claim a model.
* Conversion throughput, measured, with the hardware named.
* Extension points that refuse: HTTP Range, L0/L3/L4 cache, wgpu/Metal, rank > 3,
  implicit tiling.
* Every requirement still `Stub` or `Not Started` at release, by ID.

## `QM-0093` — attribution

| Check |
| --- |
| `mm/LICENSE` byte-identical to its original |
| `mm/` unmodified in its entirety |
| `apps/web/matrix-workspace/LICENSE` reproduces the MIT text |
| `apps/web/matrix-workspace/NOTICE.md` attributes Meta Platforms, Inc. |
| `package.json` names `mm` in its description |
| `Cargo.toml` declares `MIT OR Apache-2.0` |
| Every third-party dependency's license recorded |

## Exit conditions

1. `README.md` accurately describes what works and what does not.
2. `STATUS.md` regenerated from a real run, with no row exceeding its evidence.
3. `ARCHITECTURE.md` §8.2 corrected, with the ADR promoted to `docs/decisions/`.
4. Limitations documented, including the RTX 3090 arithmetic.
5. Attribution and licensing intact and audited.
6. All 46 acceptance criteria are ✅ or carry a **written waiver** naming the
   reason, the requirement ID, and the task that would close it.
7. **No document in the repository claims a capability the tests do not
   demonstrate.**

## Parallelization

`QM-0090`, `QM-0092`, and `QM-0093` are independent. `QM-0091` runs after them,
since it records the final state. `QM-0094` is last and gates the release.

## Risks

| Risk | Mitigation |
| --- | --- |
| R11 — `STATUS.md` and the plan diverge | `QM-0091` regenerates from a run; `STATUS.md` wins by precedence |
| Documentation overstates what shipped | `QM-0094` audits every claim against a cited test |
| The §8.2 correction is made without the ADR | `ADR-009` records the rationale; `QM-0090` cites it and its diff is reviewed line by line |
