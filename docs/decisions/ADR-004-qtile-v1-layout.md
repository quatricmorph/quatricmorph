# ADR-004 — `.qtile` v1 binary layout

**Status:** Accepted
**Date:** 2026-08-03
**Implements:** ARCHITECTURE.md §10.3

## Context

ARCHITECTURE.md §10.3 specifies the tensor sidecar's header as a Rust struct:

```rust
pub struct QTileHeader {
    pub version: u16, pub encoding: u16, pub lod: u8, pub dimensions: u8,
    pub count: u32, pub tensor_id: [u8; 16],
    pub origin: [u32; 3], pub extent: [u32; 3],
    pub min_value: f32, pub max_value: f32,
}
```

That is an in-memory struct. A `.qtile` is a **file**, read by a Rust encoder, a
browser, and eventually a CUDA host — and a file needs two things a struct does
not: a way to tell whether it is a `.qtile` at all, and a way to know where the
payload ends.

## Decision

The 72-byte header, little-endian throughout:

```text
offset  size  field
0       8     magic          "QTILE\0\0\0"
8       2     version        u16 == 1
10      2     encoding       u16  (BlockEncoding)
12      1     lod            u8   (0..=5)
13      1     dimensions     u8
14      2     _reserved      u16 == 0
16      4     count          u32
20      16    tensor_id      [u8; 16]
36      12    origin         [u32; 3]
48      12    extent         [u32; 3]
60      4     min_value      f32
64      4     max_value      f32
68      4     payload_len    u32
72      ...   payload
```

Every field of §10.3's struct is present, in the order it is declared. Three
additions:

1. **`magic`** — eight bytes so a reader can reject a non-`.qtile` before
   interpreting anything. Without it, any file is a valid header.
2. **`payload_len`** — the struct's `count` gives the number of cells, but bytes
   per cell depends on the encoding, so a reader would have to know every
   encoding's width to find the end of the payload. An explicit length also lets
   the decoder refuse an absurd declaration *before allocating*
   (`MAX_QTILE_PAYLOAD_BYTES`).
3. **`_reserved: u16`** — two bytes at offset 14 so that `count` starts at 16,
   4-byte aligned. Decoding rejects a non-zero value, which keeps the bytes
   genuinely reserved.

## Alternatives considered

**No magic, no payload length — the struct verbatim.** Rejected: a corrupt or
unrelated file would parse as a header full of garbage, and the decoder could
not bound its allocation.

**`serde` + `bincode`.** Rejected: the byte layout would then be an artefact of
a library version rather than a specification, and a `.qtile` must be readable
from JavaScript and C without a Rust library.

**Protocol Buffers / FlatBuffers.** Rejected: the header is fixed-size and the
payload is a flat array of cells. A schema compiler buys nothing here and adds a
build-time dependency to every consumer.

**Native endianness.** Rejected outright. A tile written on one machine must
read identically on another.

## Consequences

* `q_tiles::QTile::encode` / `decode` are byte-exact inverses, pinned by a
  round-trip test that asserts on the header, the payload, and the re-encoded
  bytes.
* Adding an encoding is a `BlockEncoding` variant plus its bytes-per-cell; the
  header does not change.
* v2 would bump `version` and could claim the two reserved bytes. Existing
  readers reject a version they do not know rather than misreading it.
* `schemas/qtile/schema.json` documents this layout for non-Rust readers.
