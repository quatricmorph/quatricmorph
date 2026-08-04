# MIGRATION_STRATEGY

## 0. What is being migrated

Not much — and that is the finding.

The large migration this plan might have had to describe **already happened**:
`mm` was analysed symbol by symbol (`docs/CURRENT_ARCHITECTURE.md`), ported to
TypeScript under `apps/web/matrix-workspace/`, and surrounded by a Rust workspace
that implements `ARCHITECTURE.md` §16's layout. `ADR-002` records that the crates
were *rewritten, not migrated*.

What remains is four narrower moves:

| # | Move | Risk | Task |
| --- | --- | --- | --- |
| M1 | Grid and LOD constants → one shared contract | **High** — touches three packages and two languages | `QM-0004`, `QM-0005`, `QM-0060` |
| M2 | `matrix-workspace/src/layout/` → `apps/web/core/spatial/` | Medium — import paths across one package | `QM-0060` |
| M3 | `model-viewer/src/lod-policy.ts` constants → `apps/web/core/lod/` | Low — one file, 10 tests | `QM-0060` |
| M4 | Stubs → implementations behind unchanged traits | Low — the seams are already there | Phases 03–07 |

---

## 1. The governing constraint

> **The 391-test baseline may not regress at any point.**

Not "at the end" — at any point. Every task's verification runs both suites, and
a task that reduces the passing count is not `Complete` regardless of what it
added. This is the mechanism that lets 62 tasks land incrementally without a
big-bang integration.

---

## 2. M1 — the shared spatial contract

The highest-risk move, because it is the one place where three implementations
must converge on one.

### Current state

| Constant | Rust | TypeScript |
| --- | --- | --- |
| LOD ladder | `q_tensor_runtime::Lod` | `model-viewer/src/lod-policy.ts:20` — its own enum |
| `ROOT_GEOMETRIC_ERROR` | `q_tileset:34` = `1024.0` | `lod-policy.ts:102` — `1024` inline |
| Distance thresholds | — | `lod-policy.ts:51` |
| Grid parameters | — | `matrix-workspace/src/layout/grid-ruler.ts:63` |

### Sequence

```text
1. QM-0004  Add `spatial_contract` to schemas/visualization/schema.json.
            Values are EXACTLY today's values. Nothing changes behaviourally.
            → both suites still pass, unchanged.

2. QM-0005  Add conformance tests: Rust asserts its constants equal the schema;
            vitest asserts the same; a golden vector is asserted by both.
            → tests pass because step 1 chose today's values.
            → GATE G1.

3. QM-0060  Create apps/web/core. Move grid + LOD constants there, importing
            from the schema. Re-export from the old paths.
            → existing imports keep working; both suites still pass.

4. later    New code imports from apps/web/core. The re-exports stay until no
            importer remains, then a task removes them.
```

**Step 1 changes no value.** That is what makes the migration safe: the contract
is introduced as a *description* of the current state, verified to be accurate,
and only then becomes the source. A migration that changed values and locations
at once could not tell a transcription error from a deliberate change.

### Rollback

Each step is independently revertible. Reverting step 3 restores local constants;
reverting step 1 removes a schema definition nothing yet depends on.

---

## 3. M2 / M3 — package moves

`ADR-CANDIDATE-007` records the decision to create `apps/web/core` rather than
have `model-viewer` depend on `matrix-workspace`. The reason: the viewer needs
the grid and the LOD ladder, not Three.js, `lil-gui`, or the `mm` heritage.
Depending on the workspace would drag ~2 MB of renderer into a package that
renders with Cesium.

Mechanics: `apps/web/package.json` already declares npm workspaces, so adding
`core` to the `workspaces` array and to two `dependencies` blocks is the whole
change. `vitest.config.ts` at `apps/web/` already discovers tests across
packages.

**Back-compatible aliases stay.** `grid-ruler.ts` already demonstrates the
pattern — `GridRuledLinesConfig`, `MarginGridConfig`, `DEFAULT_GRID_RULED_LINES`
are kept as aliases for previous names *"so existing imports keep working"*. The
same courtesy applies to the move.

---

## 4. M4 — stubs to implementations

The easiest migration in the plan, because the seams were designed for it.

| Stub | Trait it already implements | Becomes |
| --- | --- | --- |
| `UnimplementedGlbBuilder` | `GlbBuilder` | `InstancedGlbBuilder` |
| `UnimplementedTilesetBuilder` | `TilesetBuilder` | `ExplicitTilesetBuilder` |
| `CudaBackend` (refusing) | `q_gpu::Backend` | The same type, with kernels behind a feature |
| `DaemonBlockSource` (refusing) | `TensorBlockSource` | The same class, with a fetch |
| 501 routes | axum handlers | Real handlers on the same paths |

Rules:

1. **The trait does not change.** If implementing forces a trait change, that is a
   separate task with its own review, because a trait change ripples to every
   implementor including the tests.
2. **The refusal test survives**, retargeted: `q-cuda` without the `cuda` feature
   must still refuse, and that must still be tested. Deleting the refusal test
   would remove the evidence that the fallback is safe.
3. **A 501 becoming a 200 is not a breaking API change.** The client already
   handles both (`treats_a_501_as_a_declared_gap_not_a_failure_to_retry`).

---

## 5. Data migration

| Artifact | On upgrade |
| --- | --- |
| Catalog | Numbered migration, idempotent, forward-only. A future schema is refused, never opened (`CAT-002`) |
| `.qtile` | v1 is frozen. A v2 reader refuses v1 rather than guessing; tiles are regenerated |
| `tileset.json` | Regenerated whenever the spatial contract's version changes, since bounds and geometric errors change |
| Cache | Keys include `algorithm_version` and encoding, so a change invalidates cleanly rather than serving stale artifacts |
| URL state | Unknown keys dropped; `castToType` against defaults; never applied blindly |

**Artifacts are regenerated, never upgraded in place.** In-place upgrade of a
binary format needs a reader for every historical version, and the only thing
that guarantees is that the oldest path is the least tested one.

---

## 6. Documentation migration

Three documents outside `.plan/` will need correction. **Each is a task, not an
edit made here.**

| Document | Correction | Task |
| --- | --- | --- |
| `ARCHITECTURE.md` §8.2 | Plane mapping — code and the task specification agree against it | `QM-0090` (rationale: `ADR-009`, accepted) |
| `STATUS.md` | Regenerate from a real run at release | `QM-0091` |
| `README.md` | "What works today" / "What does not work yet" both change substantially | `QM-0090` |

`AGENTS.md`'s instruction — *"If any document conflicts with `ARCHITECTURE.md`,
follow `ARCHITECTURE.md` and fix or remove the conflicting text"* — is why §8.2
cannot simply be left wrong: an agent reading it would implement the wrong plane
mapping and break 13 tests.

---

## 7. What is not migrated

| Item | Disposition |
| --- | --- |
| `mm/` | **Read-only, permanently.** `AGENTS.md` marks it so. No task touches it |
| `mm`'s `eval` path | Never carried forward (`SEC-004`) |
| `mm`'s vendored Three.js, lil-gui, es-module-shims, OrbitControls, FontLoader | Replaced by npm dependencies |
| `mm`'s synchronous-XHR data loading and `config`-URL branch | Deprecated |
| `quatricmorph/` legacy tree | Already consolidated into the root (commit `d6cd2a2`) |
| `apps/desktop/` | Never created — Tauri is a non-goal |
| DuckDB / Arrow / Parquet catalog | Not migrated to; SQLite stands (`ADR-003`, `CAT-010`) |

---

## 8. Order of operations

```text
Phase 00 ── M1 steps 1–2 ── GATE G1 ──┬── Lane A (artifacts)  ── M4
                                       ├── Lane B (viewer)     ── M3
                                       ├── Lane C (workspace)  ── M2
                                       ├── Lane D (query)      ── M4
                                       └── Lane E (CUDA)       ── M4  [RTX 3090]
```

M1 gates everything, which is why it is Phase 00 and why its first step is
deliberately behaviour-free. The other three migrations are internal to their
lanes and carry no cross-lane risk.
