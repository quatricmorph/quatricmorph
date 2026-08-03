//! # q-safetensors — Artifact Plane
//!
//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1), producing
//! **Metadata Plane** records.
//!
//! SafeTensors header parsing, shard-index handling, bounded metadata
//! ingestion, and exact byte-range reads.
//!
//! ## Why this crate can describe a checkpoint it cannot hold
//!
//! A SafeTensors header states every tensor's name, dtype, shape, and byte
//! offsets. Reading it costs `8 + header_length` bytes per shard. That is the
//! whole basis for "trillion-scale" in this codebase: **metadata and addressing
//! scale, payload stays on disk.** There is intentionally no API here that
//! loads a tensor, a shard, or a model into memory.
//!
//! ```text
//! manifest            -> file names + lengths        (no payload read)
//! SafeTensorsHeader   -> names, dtypes, byte ranges  (header bytes only)
//! read_scalar         -> 1 element                   (dtype width in bytes)
//! read_slice_2d       -> a window                    (window size in bytes)
//! ```

pub mod header;
pub mod index;
pub mod ingest;
pub mod read;

pub use header::{HeaderEntry, SafeTensorsHeader, METADATA_KEY};
pub use index::{ShardIndex, ShardIndexMetadata};
pub use ingest::{ingest_local, CheckpointIngestor, IngestOutcome};
pub use read::{read_row, read_scalar, read_slice_2d, ScalarRead, SliceRead};
