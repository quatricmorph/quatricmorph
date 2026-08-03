//! # q-tileset — Visualization Plane
//!
//! Data plane: **Visualization Plane** (ARCHITECTURE.md §2.1, §9, §10).
//!
//! 3D Tiles `tileset.json` generation. **Interface only in this pass**
//! (`CESIUM-001`).
//!
//! ## What a tileset is for
//!
//! ARCHITECTURE.md §9.3 delegates traversal to Cesium: zoom out loads summary
//! tiles, zoom in loads tensor metadata, zoom deeper loads block summaries, and
//! only selection triggers an exact range read. The tileset is the hierarchy
//! that makes that possible — bounding volumes, geometric error, and child
//! links, mapped onto the LOD ladder of §9.1.
//!
//! ## Why this returns nothing rather than something
//!
//! A hand-written `tileset.json` with plausible bounding volumes would load in
//! CesiumJS and show a plausible scene. It would also be fiction. §20 forbids
//! exactly that, so [`UnimplementedTilesetBuilder`] refuses.
//!
//! The types below are real and used: [`GeometricError`] encodes the LOD→error
//! mapping, and [`TilesetNode`] is the structure a real builder will emit.

use q_source::error::{QError, Result};
use q_tensor_runtime::{Lod, TileId};
use serde::{Deserialize, Serialize};

/// 3D Tiles version this project targets.
pub const TILES_VERSION: &str = "1.1";

/// Root geometric error, in the tileset's own units. Every finer level halves
/// it, which is the conventional 3D Tiles ladder.
pub const ROOT_GEOMETRIC_ERROR: f64 = 1024.0;

/// Geometric error for a LOD level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometricError(pub f64);

impl GeometricError {
    /// Halve per level: LOD0 -> 1024, LOD5 -> 32.
    ///
    /// Cesium refines a tile when its screen-space error exceeds the viewer's
    /// tolerance, so a monotonically decreasing sequence is what makes
    /// "zoom in to refine" work at all.
    pub fn for_lod(lod: Lod) -> Self {
        GeometricError(ROOT_GEOMETRIC_ERROR / 2f64.powi(lod.level() as i32))
    }
}

/// An axis-aligned box: centre plus three half-axes, the 3D Tiles `box` form.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub center: [f64; 3],
    pub half_axes: [f64; 3],
}

/// One node of the tileset hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilesetNode {
    pub tile_id: TileId,
    pub lod: u8,
    pub bounding_box: BoundingBox,
    pub geometric_error: f64,
    /// GLB content, when the tile has renderable geometry.
    pub glb_uri: Option<String>,
    /// `.qtile` sidecar holding the tensor values.
    pub qtile_uri: Option<String>,
    pub children: Vec<TilesetNode>,
}

impl TilesetNode {
    /// Total nodes in this subtree.
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(TilesetNode::node_count).sum::<usize>()
    }

    /// Check that geometric error decreases with depth.
    ///
    /// A child with error greater than or equal to its parent's never refines,
    /// so the subtree below it is unreachable — a silent, invisible bug in a
    /// hand-built tileset.
    pub fn validate_refinement(&self) -> Result<()> {
        for child in &self.children {
            if child.geometric_error >= self.geometric_error {
                return Err(QError::malformed(
                    "tileset",
                    format!(
                        "tile {} has geometric error {} but its child {} has {}; error must \
                         decrease with depth or the child never refines",
                        self.tile_id, self.geometric_error, child.tile_id, child.geometric_error
                    ),
                ));
            }
            child.validate_refinement()?;
        }
        Ok(())
    }
}

/// Produces `tileset.json`.
pub trait TilesetBuilder: Send + Sync {
    fn build(&self, root: &TilesetNode) -> Result<String>;
}

/// The builder that ships today: it refuses.
pub struct UnimplementedTilesetBuilder;

impl TilesetBuilder for UnimplementedTilesetBuilder {
    fn build(&self, root: &TilesetNode) -> Result<String> {
        Err(QError::not_implemented(
            "CESIUM-001",
            format!(
                "tileset.json generation is not built in this pass ({} node(s) requested). A \
                 hand-written tileset would load in CesiumJS and look correct while being \
                 fiction, so nothing is emitted. See ARCHITECTURE.md §9 and §10.",
                root.node_count()
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::{ModelId, TensorId};
    use q_tensor_runtime::BlockExtent;

    fn tile(lod: Lod, rows: (u64, u64)) -> TilesetNode {
        let tid = TensorId::derive(ModelId::derive("m", "", "f"), "t");
        TilesetNode {
            tile_id: TileId::for_block(
                tid,
                lod,
                &BlockExtent::new(rows.0, rows.1, 0, 4).unwrap(),
            ),
            lod: lod.level(),
            bounding_box: BoundingBox {
                center: [0.0, 0.0, 0.0],
                half_axes: [1.0, 1.0, 1.0],
            },
            geometric_error: GeometricError::for_lod(lod).0,
            glb_uri: None,
            qtile_uri: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn geometric_error_halves_down_the_ladder() {
        assert_eq!(GeometricError::for_lod(Lod::Model).0, 1024.0);
        assert_eq!(GeometricError::for_lod(Lod::Layer).0, 256.0);
        assert_eq!(GeometricError::for_lod(Lod::ScalarRegion).0, 32.0);
        // Strictly decreasing across the whole ladder.
        let mut prev = f64::INFINITY;
        for lod in Lod::ALL {
            let e = GeometricError::for_lod(lod).0;
            assert!(e < prev, "{lod} error {e} did not decrease");
            prev = e;
        }
    }

    #[test]
    fn refinement_validation_accepts_a_decreasing_hierarchy() {
        let mut root = tile(Lod::Layer, (0, 8));
        root.children.push(tile(Lod::Tensor, (0, 4)));
        root.children.push(tile(Lod::Tensor, (4, 8)));
        assert!(root.validate_refinement().is_ok());
        assert_eq!(root.node_count(), 3);
    }

    #[test]
    fn a_child_that_never_refines_is_rejected() {
        let mut root = tile(Lod::Layer, (0, 8));
        let mut child = tile(Lod::Tensor, (0, 4));
        child.geometric_error = root.geometric_error * 2.0;
        root.children.push(child);
        let err = root.validate_refinement().unwrap_err();
        assert!(err.to_string().contains("never refines"));
    }

    #[test]
    fn the_builder_refuses_rather_than_emitting_a_fake_tileset() {
        let err = UnimplementedTilesetBuilder
            .build(&tile(Lod::Model, (0, 4)))
            .unwrap_err();
        assert_eq!(err.requirement_id(), Some("CESIUM-001"));
        assert!(err.to_string().contains("fiction"));
    }

    #[test]
    fn the_targeted_tiles_version_is_recorded() {
        assert_eq!(TILES_VERSION, "1.1");
    }
}
