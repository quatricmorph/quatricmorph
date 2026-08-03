//! # q-tiles — Tensor Tile Plane
//!
//! Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §10.3, §11.1).
//!
//! The `.qtile` v1 binary container.
//!
//! ARCHITECTURE.md §10.1 is emphatic that **GLB is not a tensor database**. A
//! `.qtile` is the tensor-native sidecar that holds the actual values; the GLB
//! beside it holds only geometry, instance transforms, and feature IDs. Nothing
//! in this crate can produce a GLB, which is the structural way to keep that
//! separation honest.
//!
//! ## File layout (v1)
//!
//! ```text
//! offset  size  field
//! 0       8     magic          "QTILE\0\0\0"
//! 8       2     version        u16 LE   == 1
//! 10      2     encoding       u16 LE   (q_tensor_runtime::BlockEncoding)
//! 12      1     lod            u8       (0..=5, ARCHITECTURE.md §9.1)
//! 13      1     dimensions     u8       (2 for a 2-D tensor block)
//! 14      2     _reserved      u16 LE   == 0, keeps `count` 4-byte aligned
//! 16      4     count          u32 LE   number of cells in the payload
//! 20      16    tensor_id      [u8; 16]
//! 36      12    origin         [u32; 3] LE
//! 48      12    extent         [u32; 3] LE
//! 60      4     min_value      f32 LE
//! 64      4     max_value      f32 LE
//! 68      4     payload_len    u32 LE
//! 72      ...   payload        `payload_len` bytes, encoding-dependent
//! ```
//!
//! The header mirrors `QTileHeader` in ARCHITECTURE.md §10.3 field for field
//! and in order. Two additions were needed to make it a *file* rather than an
//! in-memory struct — a magic number and an explicit `payload_len` — plus two
//! reserved bytes for alignment. Both are recorded in
//! `docs/decisions/ADR-004-qtile-v1-layout.md`.
//!
//! Every multi-byte field is little-endian, unconditionally, so a `.qtile`
//! written on one machine reads identically on another.

use q_source::error::{QError, Result};
use q_source::TensorId;
use q_tensor_runtime::{BlockEncoding, BlockExtent, Lod};
use serde::{Deserialize, Serialize};

pub const QTILE_MAGIC: &[u8; 8] = b"QTILE\0\0\0";
pub const QTILE_VERSION: u16 = 1;
/// Byte length of the fixed header.
pub const QTILE_HEADER_BYTES: usize = 72;

/// Refuse to decode a header claiming a payload larger than this. A `.qtile` is
/// a *tile*, not a checkpoint; anything this large is corrupt or hostile.
pub const MAX_QTILE_PAYLOAD_BYTES: u32 = 256 * 1024 * 1024;

/// The `.qtile` header (ARCHITECTURE.md §10.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QTileHeader {
    pub version: u16,
    pub encoding: BlockEncoding,
    pub lod: Lod,
    /// Rank of the region this tile covers (2 for a matrix block).
    pub dimensions: u8,
    /// Number of cells in the payload.
    pub count: u32,
    pub tensor_id: TensorId,
    /// Logical origin of the tile inside its tensor, `[row, column, depth]`.
    pub origin: [u32; 3],
    /// Logical extent, `[rows, columns, depth]`.
    pub extent: [u32; 3],
    pub min_value: f32,
    pub max_value: f32,
}

impl QTileHeader {
    /// Build a header for a 2-D block.
    pub fn for_block(
        tensor_id: TensorId,
        lod: Lod,
        extent: &BlockExtent,
        encoding: BlockEncoding,
        min_value: f32,
        max_value: f32,
    ) -> Result<Self> {
        let count = extent.element_count();
        if count > u32::MAX as u64 {
            return Err(QError::malformed(
                "qtile",
                format!("block of {count} cells exceeds the u32 cell count of qtile v1"),
            ));
        }
        Ok(Self {
            version: QTILE_VERSION,
            encoding,
            lod,
            dimensions: 2,
            count: count as u32,
            tensor_id,
            origin: [
                u32::try_from(extent.row_start).map_err(|_| origin_overflow())?,
                u32::try_from(extent.column_start).map_err(|_| origin_overflow())?,
                0,
            ],
            extent: [
                u32::try_from(extent.rows()).map_err(|_| origin_overflow())?,
                u32::try_from(extent.columns()).map_err(|_| origin_overflow())?,
                1,
            ],
            min_value,
            max_value,
        })
    }

    /// Bytes each cell occupies in the payload for this encoding.
    pub fn bytes_per_cell(&self) -> usize {
        match self.encoding {
            BlockEncoding::RawF32 => 4,
            BlockEncoding::QuantizedI16 => 2,
            // morton u32 + quantized i16 + flags u16 (ARCHITECTURE.md §11.1)
            BlockEncoding::MortonSparseI16 => 8,
        }
    }

    pub fn expected_payload_len(&self) -> usize {
        self.count as usize * self.bytes_per_cell()
    }
}

fn origin_overflow() -> QError {
    QError::malformed(
        "qtile",
        "tile origin or extent exceeds the u32 range of qtile v1",
    )
}

/// A complete `.qtile`: header plus payload.
#[derive(Debug, Clone, PartialEq)]
pub struct QTile {
    pub header: QTileHeader,
    pub payload: Vec<u8>,
}

impl QTile {
    pub fn new(header: QTileHeader, payload: Vec<u8>) -> Result<Self> {
        let want = header.expected_payload_len();
        if payload.len() != want {
            return Err(QError::malformed(
                "qtile",
                format!(
                    "payload is {} bytes but the header declares {} cells x {} bytes = {want}",
                    payload.len(),
                    header.count,
                    header.bytes_per_cell()
                ),
            ));
        }
        Ok(Self { header, payload })
    }

    /// Encode exact f32 values (`BlockEncoding::RawF32`).
    pub fn from_f32(
        tensor_id: TensorId,
        lod: Lod,
        extent: &BlockExtent,
        values: &[f32],
    ) -> Result<Self> {
        if values.len() as u64 != extent.element_count() {
            return Err(QError::malformed(
                "qtile",
                format!(
                    "{} values supplied for an extent of {} cells",
                    values.len(),
                    extent.element_count()
                ),
            ));
        }
        let (min, max) = min_max(values);
        let header =
            QTileHeader::for_block(tensor_id, lod, extent, BlockEncoding::RawF32, min, max)?;
        let payload = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        QTile::new(header, payload)
    }

    /// Encode values quantized to i16 against the tile's own range.
    ///
    /// Lossy by construction — [`BlockEncoding::QuantizedI16.is_lossless()`]
    /// is `false`, and consumers must label such values as approximate.
    pub fn from_f32_quantized(
        tensor_id: TensorId,
        lod: Lod,
        extent: &BlockExtent,
        values: &[f32],
    ) -> Result<Self> {
        if values.len() as u64 != extent.element_count() {
            return Err(QError::malformed(
                "qtile",
                format!(
                    "{} values supplied for an extent of {} cells",
                    values.len(),
                    extent.element_count()
                ),
            ));
        }
        let (min, max) = min_max(values);
        let header =
            QTileHeader::for_block(tensor_id, lod, extent, BlockEncoding::QuantizedI16, min, max)?;
        let payload = values
            .iter()
            .flat_map(|v| quantize_i16(*v, min, max).to_le_bytes())
            .collect();
        QTile::new(header, payload)
    }

    /// Decode a `RawF32` payload back to values.
    pub fn to_f32(&self) -> Result<Vec<f32>> {
        if self.header.encoding != BlockEncoding::RawF32 {
            return Err(QError::UnsupportedDType {
                dtype: format!("{:?}", self.header.encoding),
                operation: "exact f32 decode".into(),
            });
        }
        Ok(self
            .payload
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(QTILE_HEADER_BYTES + self.payload.len());
        out.extend_from_slice(QTILE_MAGIC);
        out.extend_from_slice(&self.header.version.to_le_bytes());
        out.extend_from_slice(&self.header.encoding.code().to_le_bytes());
        out.push(self.header.lod.level());
        out.push(self.header.dimensions);
        out.extend_from_slice(&0u16.to_le_bytes()); // reserved
        out.extend_from_slice(&self.header.count.to_le_bytes());
        out.extend_from_slice(self.header.tensor_id.as_bytes());
        for v in self.header.origin {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.header.extent {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.header.min_value.to_le_bytes());
        out.extend_from_slice(&self.header.max_value.to_le_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        debug_assert_eq!(out.len(), QTILE_HEADER_BYTES);
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < QTILE_HEADER_BYTES {
            return Err(QError::malformed(
                "qtile",
                format!(
                    "{} bytes is shorter than the {QTILE_HEADER_BYTES}-byte header",
                    bytes.len()
                ),
            ));
        }
        if &bytes[0..8] != QTILE_MAGIC {
            return Err(QError::malformed(
                "qtile",
                "missing QTILE magic; this is not a .qtile file",
            ));
        }
        let version = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        if version != QTILE_VERSION {
            return Err(QError::malformed(
                "qtile",
                format!("version {version} is not supported (this build reads v{QTILE_VERSION})"),
            ));
        }
        let encoding =
            BlockEncoding::from_code(u16::from_le_bytes(bytes[10..12].try_into().unwrap()))?;
        let lod = Lod::from_level(bytes[12])?;
        let dimensions = bytes[13];
        let reserved = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
        if reserved != 0 {
            return Err(QError::malformed(
                "qtile",
                "reserved header bytes must be zero in v1",
            ));
        }
        let count = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[20..36]);
        let read_u32x3 = |o: usize| {
            [
                u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()),
                u32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
                u32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
            ]
        };
        let origin = read_u32x3(36);
        let extent = read_u32x3(48);
        let min_value = f32::from_le_bytes(bytes[60..64].try_into().unwrap());
        let max_value = f32::from_le_bytes(bytes[64..68].try_into().unwrap());
        let payload_len = u32::from_le_bytes(bytes[68..72].try_into().unwrap());

        if payload_len > MAX_QTILE_PAYLOAD_BYTES {
            // Refuse before allocating.
            return Err(QError::BudgetExceeded {
                budget_name: "qtile_payload",
                requested: payload_len as u64,
                limit: MAX_QTILE_PAYLOAD_BYTES as u64,
            });
        }
        let end = QTILE_HEADER_BYTES + payload_len as usize;
        if bytes.len() < end {
            return Err(QError::malformed(
                "qtile",
                format!(
                    "header declares a {payload_len}-byte payload but only {} bytes follow",
                    bytes.len() - QTILE_HEADER_BYTES
                ),
            ));
        }

        let header = QTileHeader {
            version,
            encoding,
            lod,
            dimensions,
            count,
            tensor_id: TensorId::from_bytes(id),
            origin,
            extent,
            min_value,
            max_value,
        };
        QTile::new(header, bytes[QTILE_HEADER_BYTES..end].to_vec())
    }
}

fn min_max(values: &[f32]) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if values.is_empty() {
        (0.0, 0.0)
    } else {
        (min, max)
    }
}

/// Map `value` in `[min, max]` onto the full i16 range.
fn quantize_i16(value: f32, min: f32, max: f32) -> i16 {
    if max <= min {
        return 0;
    }
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
    (t * (u16::MAX as f32) - 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

/// Inverse of [`quantize_i16`]. Lossy; callers must label results approximate.
pub fn dequantize_i16(q: i16, min: f32, max: f32) -> f32 {
    if max <= min {
        return min;
    }
    let t = (q as f32 + 32768.0) / (u16::MAX as f32);
    min + t * (max - min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::ModelId;

    fn tensor_id() -> TensorId {
        TensorId::derive(ModelId::derive("m", "", "f"), "model.layers.10.self_attn.q_proj.weight")
    }

    fn extent() -> BlockExtent {
        BlockExtent::new(100, 104, 40, 44).unwrap()
    }

    fn sample_values() -> Vec<f32> {
        (0..16).map(|i| (i as f32 - 8.0) * 0.125).collect()
    }

    #[test]
    fn header_is_exactly_72_bytes_and_the_magic_leads() {
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &sample_values()).unwrap();
        let bytes = tile.encode();
        assert_eq!(&bytes[0..8], QTILE_MAGIC);
        assert_eq!(bytes.len(), QTILE_HEADER_BYTES + 16 * 4);
    }

    #[test]
    fn round_trip_preserves_header_and_payload_byte_for_byte() {
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &sample_values()).unwrap();
        let encoded = tile.encode();
        let decoded = QTile::decode(&encoded).unwrap();
        assert_eq!(decoded.header, tile.header);
        assert_eq!(decoded.payload, tile.payload);
        assert_eq!(decoded, tile);
        // Re-encoding is byte-identical: the format has no free bits.
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn round_trip_preserves_exact_f32_values() {
        let values = sample_values();
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &values).unwrap();
        let back = QTile::decode(&tile.encode()).unwrap().to_f32().unwrap();
        assert_eq!(back, values);
        // Including the awkward ones.
        let odd = vec![
            f32::MIN_POSITIVE,
            -0.0,
            1.0 / 3.0,
            f32::MAX,
            -f32::MAX,
            1e-30,
            0.1,
            0.2,
            0.3,
            123456.789,
            -0.000001,
            2.0,
            4.0,
            8.0,
            16.0,
            32.0,
        ];
        let t2 = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &odd).unwrap();
        let b2 = QTile::decode(&t2.encode()).unwrap().to_f32().unwrap();
        assert_eq!(b2.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                   odd.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    }

    #[test]
    fn header_fields_carry_the_architecture_md_semantics() {
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &sample_values()).unwrap();
        let h = &tile.header;
        assert_eq!(h.version, QTILE_VERSION);
        assert_eq!(h.encoding, BlockEncoding::RawF32);
        assert_eq!(h.lod, Lod::Block);
        assert_eq!(h.dimensions, 2);
        assert_eq!(h.count, 16);
        assert_eq!(h.tensor_id, tensor_id());
        assert_eq!(h.origin, [100, 40, 0]);
        assert_eq!(h.extent, [4, 4, 1]);
        assert_eq!(h.min_value, -1.0);
        assert_eq!(h.max_value, 0.875);
    }

    #[test]
    fn quantized_tiles_are_half_the_size_and_declare_themselves_lossy() {
        let values = sample_values();
        let raw = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &values).unwrap();
        let q = QTile::from_f32_quantized(tensor_id(), Lod::Block, &extent(), &values).unwrap();
        assert_eq!(q.payload.len() * 2, raw.payload.len());
        assert!(!q.header.encoding.is_lossless());
        // An exact decode of a lossy tile is refused, not approximated.
        assert!(matches!(
            q.to_f32(),
            Err(QError::UnsupportedDType { .. })
        ));
        // Round-tripping through the container is still byte-exact.
        assert_eq!(QTile::decode(&q.encode()).unwrap(), q);
    }

    #[test]
    fn quantization_endpoints_reconstruct_the_declared_range() {
        let min = -1.0f32;
        let max = 0.875f32;
        assert!((dequantize_i16(quantize_i16(min, min, max), min, max) - min).abs() < 1e-4);
        assert!((dequantize_i16(quantize_i16(max, min, max), min, max) - max).abs() < 1e-4);
        // A degenerate range does not divide by zero.
        assert_eq!(quantize_i16(5.0, 5.0, 5.0), 0);
        assert_eq!(dequantize_i16(0, 5.0, 5.0), 5.0);
    }

    #[test]
    fn corrupt_and_hostile_files_are_rejected() {
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &sample_values()).unwrap();
        let good = tile.encode();

        // Truncated.
        assert!(QTile::decode(&good[..40]).is_err());
        // Wrong magic.
        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert!(QTile::decode(&bad_magic).unwrap_err().to_string().contains("magic"));
        // Unsupported version.
        let mut bad_version = good.clone();
        bad_version[8..10].copy_from_slice(&999u16.to_le_bytes());
        assert!(QTile::decode(&bad_version).is_err());
        // Unknown encoding.
        let mut bad_encoding = good.clone();
        bad_encoding[10..12].copy_from_slice(&77u16.to_le_bytes());
        assert!(QTile::decode(&bad_encoding).is_err());
        // LOD outside the ladder.
        let mut bad_lod = good.clone();
        bad_lod[12] = 9;
        assert!(QTile::decode(&bad_lod).is_err());
        // Non-zero reserved bytes.
        let mut bad_reserved = good.clone();
        bad_reserved[14..16].copy_from_slice(&1u16.to_le_bytes());
        assert!(QTile::decode(&bad_reserved).is_err());
        // Payload shorter than declared.
        let mut short = good.clone();
        short.truncate(QTILE_HEADER_BYTES + 8);
        assert!(QTile::decode(&short).is_err());
        // Absurd payload length: refused before allocating.
        let mut huge = good.clone();
        huge[68..72].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            QTile::decode(&huge),
            Err(QError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn header_payload_disagreement_is_rejected_at_construction() {
        let h = QTileHeader::for_block(
            tensor_id(),
            Lod::Block,
            &extent(),
            BlockEncoding::RawF32,
            0.0,
            1.0,
        )
        .unwrap();
        assert!(QTile::new(h, vec![0u8; 8]).is_err());
    }

    #[test]
    fn value_count_must_match_the_extent() {
        assert!(QTile::from_f32(tensor_id(), Lod::Block, &extent(), &[1.0, 2.0]).is_err());
    }

    #[test]
    fn encoding_is_little_endian_regardless_of_host() {
        let tile = QTile::from_f32(tensor_id(), Lod::Block, &extent(), &sample_values()).unwrap();
        let bytes = tile.encode();
        // count == 16 -> 0x10 0x00 0x00 0x00
        assert_eq!(&bytes[16..20], &[0x10, 0x00, 0x00, 0x00]);
        // origin[0] == 100 -> 0x64 0x00 0x00 0x00
        assert_eq!(&bytes[36..40], &[0x64, 0x00, 0x00, 0x00]);
    }
}
