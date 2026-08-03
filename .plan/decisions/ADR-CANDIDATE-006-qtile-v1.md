# ADR-CANDIDATE-006 — `.qtile` v1 binary schema

## Status

`Decided` — already promoted: `docs/decisions/ADR-004-qtile-v1-layout.md`.

## Context

`ARCHITECTURE.md` §10.3 sketches a `QTileHeader`. The repository implements it.
The remaining question is what is **frozen** in v1 and what waits for v2.

## Repository evidence

* `crates/q-tiles/src/lib.rs` — `QTILE_MAGIC = b"QTILE\0\0\0"`,
  `QTILE_VERSION = 1`, `QTILE_HEADER_BYTES = 72`,
  `MAX_QTILE_PAYLOAD_BYTES = 256 MiB`.
* Three encodings: `RawF32` (4 B/cell), `QuantizedI16` (2), `MortonSparseI16`
  (8 = morton `u32` + quantized `i16` + flags `u16`).
* `TILE-005`…`TILE-008` **Verified**: byte-exact round trip, exact f32
  preservation, 8 corruption classes rejected, little-endian on any host,
  quantized tiles declare themselves lossy.
* `schemas/qtile/schema.json`, 93 lines.

## Decision required

None for v1. Recording the freeze and the v2 backlog.

## Options considered at the time

| Option | |
| --- | --- |
| **A** | Fixed 72-byte header, three encodings (chosen) |
| **B** | A self-describing container — CBOR, FlatBuffers, or similar |
| **C** | Reuse an existing tensor format — NPY, Zarr chunk, Arrow IPC |

## Advantages of A

Zero-copy header read; fixed offsets; no parser dependency; a decoder that fits
in a Web Worker in a few dozen lines; a payload ceiling that makes an allocation
bomb impossible; little-endian normalization so a file written on any host reads
identically on any other.

## Disadvantages of A

Adding a field needs a version bump. `u32` origin and extent cap an axis at
4.29×10⁹ elements.

## Risks

Low. `MAX_QTILE_PAYLOAD_BYTES` — *"a `.qtile` is a tile, not a checkpoint;
anything this large is corrupt or hostile"* — closes the main hostile-input
vector, and eight corruption classes are already covered by tests.

## Recommended default

**A**, frozen. Deferred to v2:

| Deferred | Why not now |
| --- | --- |
| Rank > 3 regions | `dimensions: u8` already carries rank; the payload layout is undesigned (`GRID-007`) |
| `u64` origin/extent | No real tensor axis approaches 4.29×10⁹ |
| Per-tile compression | **Measure first.** Quantized tiles are already 2 B/cell; zstd may not pay for the decode |
| In-file checksum | The catalog's `content_hash` already covers integrity; duplicating it adds a second thing to keep consistent |

**A reader refuses an unknown version rather than guessing a layout** — the same
discipline as `SRC-014`'s unknown dtypes and `NSIR-001`'s unknown roles.

## Tasks affected

`QM-0041` (generation), `QM-0046` (validation). Neither changes the format.

## Decision deadline

Passed.
