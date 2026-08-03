/**
 * LOD loading policy — Visualization Plane (ARCHITECTURE.md §9.3).
 *
 * Pure decision logic, no renderer. The rules it encodes:
 *
 * ```text
 * zoom out          -> only load summary tiles
 * zoom in           -> load tensor metadata
 * zoom deeper       -> load block summaries
 * select or inspect -> range-read exact bytes from SafeTensors
 * ```
 *
 * The load-bearing invariant is the last one: **exact values are read only on
 * an explicit selection**, never as a side effect of camera movement
 * (ARCHITECTURE.md §18 AC-006, AC-007). Expressing that as a pure function is
 * what makes it testable without a GPU or a camera.
 */

/** The six tiers of ARCHITECTURE.md §9.1. Mirrors `q_tensor_runtime::Lod`. */
export enum Lod {
  Model = 0,
  Subsystem = 1,
  Layer = 2,
  Tensor = 3,
  Block = 4,
  ScalarRegion = 5,
}

export type CameraState = {
  /** Distance from the camera to the tile, in tileset units. */
  distance: number
  /** Screen-space error the viewer tolerates before refining. */
  screenSpaceErrorTolerance: number
}

/** What the viewer intends to do, not merely where it is looking. */
export type Interaction = 'idle' | 'navigating' | 'hovering' | 'selected'

export type LoadDecision = {
  lod: Lod
  /** Tiles to fetch now. */
  fetchTiles: boolean
  /** Child tiles to warm. §13.3: prefetch children and sibling metadata... */
  prefetchChildren: boolean
  /** ...but never exact values. */
  fetchExactValues: boolean
  reason: string
}

/** Distance thresholds, coarsest first. Named, not magic. */
export const LOD_DISTANCE_THRESHOLDS: readonly number[] = [4096, 1024, 256, 64, 16]

/** Coarsest LOD whose threshold the camera is still beyond. */
export function lodForDistance(distance: number): Lod {
  for (let i = 0; i < LOD_DISTANCE_THRESHOLDS.length; i++) {
    if (distance >= LOD_DISTANCE_THRESHOLDS[i]) return i as Lod
  }
  return Lod.ScalarRegion
}

/**
 * Decide what to load.
 *
 * `fetchExactValues` is true if and only if the interaction is `selected`. No
 * distance, however small, produces exact reads on its own.
 */
export function decideLoad(camera: CameraState, interaction: Interaction): LoadDecision {
  const lod = lodForDistance(camera.distance)
  if (interaction === 'selected') {
    return {
      lod: Lod.ScalarRegion,
      fetchTiles: true,
      prefetchChildren: false,
      fetchExactValues: true,
      reason: 'explicit selection — range-read the exact bytes for this region',
    }
  }
  if (interaction === 'navigating') {
    return {
      lod,
      fetchTiles: true,
      // Prefetching during motion would multiply requests while the target is
      // still changing; §13.3 prefetches when the camera settles.
      prefetchChildren: false,
      fetchExactValues: false,
      reason: 'camera in motion — summary tiles for the current level only',
    }
  }
  return {
    lod,
    fetchTiles: true,
    prefetchChildren: lod < Lod.ScalarRegion,
    fetchExactValues: false,
    reason:
      interaction === 'hovering'
        ? 'hovering — tile and children, but hovering is not selecting'
        : 'settled — tile plus children and sibling metadata',
  }
}

/** Geometric error for a LOD level; mirrors `q_tileset::GeometricError`. */
export function geometricErrorForLod(lod: Lod): number {
  return 1024 / 2 ** lod
}
