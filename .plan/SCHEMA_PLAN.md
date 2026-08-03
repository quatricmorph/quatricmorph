# SCHEMA_PLAN — schemas, binary formats, versioning

## 1. Inventory

| Schema | Lines | `$id` | Describes |
| --- | --- | --- | --- |
| `schemas/nsir/schema.json` | 116 | `…/nsir/v1` | Semantic tensor records, canonical addresses, roles, axes |
| `schemas/qtile/schema.json` | 93 | `…/qtile/v1` | `.qtile` header fields and encodings |
| `schemas/weightql/schema.json` | 166 | `…/weightql/v1` | Statements, plans, results |
| `schemas/visualization/schema.json` | 119 | `…/visualization/v1` | `bounding_box`, `visual_tile_row`, `tileset_node`, `glb_tile_spec` |

All four already carry an honest self-description. The visualization schema's own
`description` states that tileset and GLB generation are not implemented and that
the schema exists *"so the daemon, the catalog, and the viewer cannot drift while
the builders are written."* That is exactly the right use of a schema in a
partially built system.

---

## 2. The gap: no spatial contract

`schemas/visualization/schema.json` fixes what a tile *record* looks like. It
does not fix **where anything is**, and that is what three independent
implementations currently each decide for themselves
([`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md) §5):

| Concept | Rust runtime | Rust visualization | TypeScript viewer | TypeScript workspace |
| --- | --- | --- | --- | --- |
| LOD ladder | `q_tensor_runtime::Lod` | uses it | **own `enum Lod`** | — |
| Geometric error | — | `ROOT/2^lod`, `ROOT=1024.0` | **`1024 / 2**lod`, hand-mirrored** | — |
| Distance thresholds | — | — | `[4096,1024,256,64,16]` | — |
| Grid parameters | — | — | — | **`DEFAULT_GRID_RULER`** |
| Snap tolerance | — | — | — | `1e-6` |

`QM-0004` closes this by adding a `spatial_contract` definition to the
visualization schema; `QM-0005` adds the conformance tests that keep it closed.

### 2.1 The `spatial_contract` definition

```jsonc
{
  "spatial_contract": {
    "description": "The single spatial authority. Rust (q-tensor-runtime, q-tileset) and TypeScript (apps/web/core) both derive from this; a constant that disagrees fails a conformance test.",
    "type": "object",
    "required": ["version", "grid", "lod_ladder", "geometric_error", "snap_tolerance"],
    "properties": {
      "version": { "const": 1 },
      "grid": {
        "type": "object",
        "required": ["cellSize","minorGridSpacing","majorGridInterval","tensorPadding",
                     "labelMargin","framePadding","operandGap","axisMargin",
                     "depthSpacing","origin"],
        "properties": {
          "cellSize":          { "type": "number", "exclusiveMinimum": 0, "default": 1 },
          "minorGridSpacing":  { "type": "number", "exclusiveMinimum": 0, "default": 1 },
          "majorGridInterval": { "type": "integer", "minimum": 1, "default": 5 },
          "tensorPadding":     { "type": "number", "minimum": 0, "default": 1 },
          "labelMargin":       { "type": "number", "minimum": 0, "default": 1 },
          "framePadding":      { "type": "number", "minimum": 0, "default": 1 },
          "operandGap":        { "type": "number", "minimum": 0, "default": 4 },
          "axisMargin":        { "type": "number", "minimum": 0, "default": 1 },
          "depthSpacing":      { "type": "number", "minimum": 0, "default": 0 },
          "origin": { "type": "array", "items": {"type":"number"}, "minItems":3, "maxItems":3 }
        }
      },
      "lod_ladder": {
        "type": "array", "minItems": 6, "maxItems": 6,
        "items": {
          "type": "object",
          "required": ["level","name","carries_exact_values","distance_threshold"],
          "properties": {
            "level": { "type": "integer", "minimum": 0, "maximum": 5 },
            "name":  { "enum": ["model","subsystem","layer","tensor","block","region"] },
            "carries_exact_values": { "type": "boolean" },
            "distance_threshold":   { "type": ["number","null"] }
          }
        }
      },
      "geometric_error": {
        "type": "object",
        "required": ["root","falloff"],
        "properties": {
          "root":    { "type": "number", "exclusiveMinimum": 0, "default": 1024.0 },
          "falloff": { "const": "halving" }
        }
      },
      "snap_tolerance": { "type": "number", "exclusiveMinimum": 0, "default": 1e-6 },
      "axis_binding": {
        "description": "Tensor axis → world axis. Rank > 3 is an extension point (GRID-007).",
        "type": "object",
        "properties": {
          "world_axes": { "const": { "X": "J", "Y": "I", "Z": "K" } },
          "max_implemented_rank": { "const": 3 }
        }
      },
      "instance_ceiling": { "const": 262144 }
    }
  }
}
```

`instance_ceiling` appears here because it is one number governing two
subsystems: `q_gltf::MAX_INSTANCES_PER_TILE` and the workspace's
`MAX_WORKSPACE_SPHERES`. Defining it twice is how they drift apart.

### 2.2 Consumption

| Consumer | Mechanism | Task |
| --- | --- | --- |
| `q-tensor-runtime`, `q-tileset` | A test loads the JSON and asserts each Rust constant equals its schema value | `QM-0005` |
| `apps/web/core` | The JSON is imported at build time and re-exported as typed constants — the **only** definition in TypeScript | `QM-0004` |
| `model-viewer`, `matrix-workspace` | Import from `apps/web/core`; declare no spatial constants of their own | `QM-0060` |

**Assert, do not generate, on the Rust side.** Generated Rust would need a build
script, would complicate `cargo test` for a machine without Node, and would make
the constants harder to read at their point of use. An assertion test gives the
same guarantee — drift turns a suite red — at a fraction of the cost.

### 2.3 The golden vector

A conformance corpus at `schemas/visualization/golden-spatial.json`, asserted by
**both** a Rust test and a vitest test:

```jsonc
{
  "geometric_error": [
    { "lod": 0, "expected": 1024.0 }, { "lod": 3, "expected": 128.0 },
    { "lod": 5, "expected": 32.0 }
  ],
  "lod_for_distance": [
    { "distance": 5000, "expected_lod": 0 }, { "distance": 100, "expected_lod": 3 },
    { "distance": 8,    "expected_lod": 5 }
  ],
  "cell_center": [
    { "i": 0, "j": 0, "cellSize": 1, "tensorPadding": 1, "expected": [1, 1, 0] },
    { "i": 255, "j": 128, "cellSize": 1, "tensorPadding": 1, "expected": [129, 256, 0] }
  ],
  "load_decision": [
    { "distance": 5000, "interaction": "navigating", "reads_exact": false },
    { "distance": 8,    "interaction": "hovering",   "reads_exact": false },
    { "distance": 8,    "interaction": "selected",   "reads_exact": true  }
  ]
}
```

The last block is the mechanised form of `AC-006`: **camera movement alone never
reads exact values**, asserted in both languages against the same table.

---

## 3. `.qtile` v1 — frozen

Fully specified in [`DATA_ARCHITECTURE.md`](DATA_ARCHITECTURE.md) §6 and
implemented. 72-byte header, magic `QTILE\0\0\0`, little-endian on every host,
256 MiB payload ceiling, three encodings. Round trip is byte-exact and eight
corruption classes are rejected.

**v1 is frozen.** Additions go in v2 with a version bump; a reader refuses a
version it does not know rather than guessing a layout. Deferred to v2:

| Deferred | Why not v1 |
| --- | --- |
| Rank > 3 regions | `dimensions: u8` already carries rank; the payload layout for higher ranks is undesigned (`GRID-007`) |
| `u64` origin/extent | `u32` caps a tensor axis at 4.29×10⁹ — no real tensor is close |
| Per-tile compression | Measure first. Quantized tiles are already 2 B/cell; zstd on top may not pay for the decode |
| Checksum | The catalog's `content_hash` already covers integrity; an in-file checksum would duplicate it |

---

## 4. Schema-to-code map

| Schema | Rust | TypeScript |
| --- | --- | --- |
| `nsir` | `q-nsir` — `CanonicalAddress`, `Alias`, `SemanticRecord`; `q_source::TensorRole` | `apps/web/core/address` |
| `qtile` | `q-tiles` — `QTileHeader`, `QTile`, `BlockEncoding` | Worker decoder in `matrix-workspace` and `model-viewer` |
| `weightql` | `q-weightql` — `Statement`, `Script`, `Plan`; `q_expression::Expr` | `query-interface/src/weightql.ts` |
| `visualization` | `q-tileset` — `TilesetNode`, `BoundingBox`, `GeometricError`; `q-gltf` — `GlbTileSpec`; `q_catalog` `visual_tiles` | `apps/web/core/{spatial,lod}` |

---

## 5. Versioning and migration procedure

| Artifact | Version carrier | Breaking-change procedure |
| --- | --- | --- |
| JSON schemas | `$id` ends `/v1` | Mint `/v2`; both may exist during migration; consumers pin |
| `.qtile` | `QTILE_VERSION` | Bump; readers refuse unknown versions; regenerate tiles |
| Catalog | `CURRENT_SCHEMA_VERSION` + `schema_migrations` | Append a numbered migration. **Never edit a shipped one** — it has already run on someone's disk |
| Spatial contract | `spatial_contract.version` | Bump; both conformance suites fail until updated; regenerate tilesets, since bounds and errors change |
| HTTP API | `/v1/` prefix | Additive within a major version |
| Statistics | `algorithm_version` per row | A change mints new rows and new cache keys; old results stay readable and comparable |

### The procedure for a breaking change

1. Write the ADR. A breaking change without a recorded reason is indistinguishable
   from a mistake six months later.
2. Bump the version in the schema.
3. Update both conformance suites; watch them fail, then pass.
4. Add a catalog migration if a persisted shape changed.
5. Regenerate affected artifacts; **do not attempt to upgrade them in place**.
6. Update `STATUS.md` and the affected `.plan/` documents.

`ARCHITECTURE.md` §19's discipline applies: refuse rather than reinterpret. A
reader that guesses at an unknown version produces a plausible wrong answer, which
is the failure mode this repository most consistently designs against.

---

## 6. Requirements

| ID | Requirement | Task |
| --- | --- | --- |
| `GRID-006` | `spatial_contract` in `schemas/visualization/schema.json` | `QM-0004` |
| `GRID-011` | Cross-language conformance tests + golden vector | `QM-0005` |
| `SCHEMA-001` | Every schema's `$id` carries an explicit version | `QM-0004` |
| `SCHEMA-002` | A reader refuses an unknown version rather than guessing | Already true for `.qtile` and the catalog; asserted for the contract in `QM-0005` |
