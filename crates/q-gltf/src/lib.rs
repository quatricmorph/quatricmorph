//! # q-gltf — Visualization Plane
//!
//! Data plane: **Visualization Plane** (ARCHITECTURE.md §2.1, §10).
//!
//! GLB tile-content generation. **Interface only in this pass** (`GLB-001`).
//!
//! ## GLB is not a tensor database
//!
//! ARCHITECTURE.md §10.1 draws the line and this crate is built to keep it. A
//! GLB tile may contain:
//!
//! * shared geometry (one unit cube, reused);
//! * instance transforms;
//! * quantized visual attributes;
//! * feature IDs;
//! * tile-local metadata.
//!
//! It must **never** contain full FP16/BF16 weights, many copies of the same
//! cube mesh, reproducible analysis results, or exact tensor data for the whole
//! model. Actual values live in `.qtile` sidecars (`q-tiles`), which this crate
//! deliberately cannot write.
//!
//! [`GlbBuilder`] is the trait a real implementation will satisfy;
//! [`UnimplementedGlbBuilder`] is what ships today, and it refuses rather than
//! emitting a plausible-looking empty GLB.

use q_source::error::{QError, Result};
use q_tensor_runtime::{BlockEncoding, TensorBlock, TileId};
use serde::{Deserialize, Serialize};

/// What a GLB tile is allowed to carry (ARCHITECTURE.md §10.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlbTileSpec {
    pub tile_id: TileId,
    /// Instances to draw. One shared mesh, `instance_count` transforms.
    pub instance_count: u32,
    /// How the per-instance value attribute is quantized.
    pub value_encoding: BlockEncoding,
    /// URI of the `.qtile` holding the actual values.
    pub qtile_uri: String,
    /// Whether to request `EXT_mesh_gpu_instancing`.
    ///
    /// ARCHITECTURE.md §10.2 notes the extension exists but that Quatricmorph
    /// must check the renderer's real support level and keep a fallback, so
    /// this is a request, not an assumption.
    pub request_gpu_instancing: bool,
}

impl GlbTileSpec {
    /// Reject a spec that would violate §10.1 / §19.
    ///
    /// A cube-per-weight GLB is a data explosion, not an optimization, so the
    /// instance ceiling is enforced here rather than discovered when a browser
    /// tab dies.
    pub fn validate(&self) -> Result<()> {
        if self.instance_count > MAX_INSTANCES_PER_TILE {
            return Err(QError::QueryRejected(format!(
                "tile {} requests {} instances; the ceiling is {MAX_INSTANCES_PER_TILE}. \
                 Split the block into more tiles rather than emitting one cube per weight \
                 (ARCHITECTURE.md §19).",
                self.tile_id, self.instance_count
            )));
        }
        if self.qtile_uri.is_empty() {
            return Err(QError::QueryRejected(format!(
                "tile {} has no .qtile sidecar; a GLB may not be the only carrier of tensor \
                 values (ARCHITECTURE.md §10.1)",
                self.tile_id
            )));
        }
        Ok(())
    }
}

/// Instances per GLB tile. A 256x256 block is 65 536 cells; beyond roughly this
/// the tile should be split rather than made denser.
pub const MAX_INSTANCES_PER_TILE: u32 = 262_144;

/// Produces GLB tile content.
pub trait GlbBuilder: Send + Sync {
    fn build(&self, spec: &GlbTileSpec, block: &TensorBlock) -> Result<Vec<u8>>;
}

/// The builder that ships today: it refuses.
pub struct UnimplementedGlbBuilder;

impl GlbBuilder for UnimplementedGlbBuilder {
    fn build(&self, spec: &GlbTileSpec, _block: &TensorBlock) -> Result<Vec<u8>> {
        Err(QError::not_implemented(
            "GLB-001",
            format!(
                "GLB tile generation is not built in this pass (tile {}). Returning an empty or \
                 placeholder GLB would be indistinguishable from a real one to a viewer, so \
                 nothing is returned. See ARCHITECTURE.md §10.",
                spec.tile_id
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::{ModelId, TensorId};
    use q_tensor_runtime::{BlockExtent, Lod};

    fn spec(instances: u32, qtile: &str) -> GlbTileSpec {
        let tid = TensorId::derive(ModelId::derive("m", "", "f"), "t");
        GlbTileSpec {
            tile_id: TileId::for_block(tid, Lod::Block, &BlockExtent::new(0, 4, 0, 4).unwrap()),
            instance_count: instances,
            value_encoding: BlockEncoding::QuantizedI16,
            qtile_uri: qtile.to_string(),
            request_gpu_instancing: true,
        }
    }

    #[test]
    fn a_reasonable_tile_spec_validates() {
        assert!(spec(65_536, "tiles/a.qtile").validate().is_ok());
    }

    #[test]
    fn cube_per_weight_explosions_are_refused() {
        let err = spec(10_000_000, "tiles/a.qtile").validate().unwrap_err();
        assert!(err.to_string().contains("one cube per weight"));
    }

    #[test]
    fn a_glb_without_a_qtile_sidecar_is_refused() {
        let err = spec(16, "").validate().unwrap_err();
        assert!(err.to_string().contains("may not be the only carrier"));
    }

    #[test]
    fn the_builder_refuses_rather_than_emitting_a_placeholder_glb() {
        use q_source::DType;
        use q_source::TensorDescriptor;
        let tid = TensorId::derive(ModelId::derive("m", "", "f"), "t");
        let d = TensorDescriptor {
            tensor_id: tid,
            raw_name: "t".into(),
            canonical_name: "t".into(),
            shape: vec![8, 8],
            dtype: DType::F32,
            shard_uri: "s".into(),
            byte_start: 0,
            byte_end: 256,
            layer_index: None,
            semantic_role: q_source::TensorRole::Unknown,
        };
        let block =
            TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 4, 0, 4).unwrap()).unwrap();
        let err = UnimplementedGlbBuilder
            .build(&spec(16, "tiles/a.qtile"), &block)
            .unwrap_err();
        assert_eq!(err.requirement_id(), Some("GLB-001"));
        assert!(err
            .to_string()
            .contains("indistinguishable from a real one"));
    }
}
