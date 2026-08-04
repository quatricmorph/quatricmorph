/**
 * GridRuler3D — the single spatial layout authority (`GRID-001`).
 *
 * Every tensor, frame, label, and guide in the workspace is positioned through
 * this module. That is the point: one ruler means a cell drawn at world
 * position p corresponds to exactly one logical index, so clicking it can
 * return a canonical tensor address (ARCHITECTURE.md §18 AC-004).
 *
 * World axes: X → J (output cols), Y → I (output rows), Z → K (contraction).
 * Planes: A on I×K, B on K×J, C on I×J.
 *
 * **Grid invariant:** every position this module produces is an integer
 * multiple of `cellSize`, within {@link GRID_SNAP_TOLERANCE}. Off-grid
 * positions accumulate into visible drift between a tensor's cells and its
 * frame, and — worse — into a mis-addressed click. {@link GridRuler3D.assertSnapped}
 * turns that from a rendering artefact into an error.
 *
 * `GridRuledLines3D` and `MarginGrid3D` are the previous names for this module
 * and remain as aliases at the bottom of the file so existing imports keep
 * working. New code should use `GridRuler3D`.
 */

export type Vec3 = { x: number; y: number; z: number }

export type GridRuler3DConfig = {
  cellSize: number
  minorGridSpacing: number
  majorGridInterval: number
  tensorPadding: number
  labelMargin: number
  framePadding: number
  /** Gap between operand planes (legacy `layout.gap`). */
  operandGap: number
  axisMargin: number
  depthSpacing: number
  origin: Vec3
}

export type PlacementHints = {
  polarity: 'positive' | 'negative'
  leftPlacement: 'left' | 'right'
  rightPlacement: 'top' | 'bottom'
  resultPlacement: 'front' | 'back'
  leftScatter?: number
  rightScatter?: number
}

export type PlaneTransform = {
  position: Vec3
  /** Euler radians applied to the tensor's local I×J plane. */
  rotation: Vec3
  /** Axis labels for the local h/w edges. */
  axes: { h: string; w: string }
}

export type TensorExtents = {
  /** Display extent along local X (cols / J or K depending on plane). */
  x: number
  y: number
  z: number
}

export const DEFAULT_GRID_RULER: GridRuler3DConfig = {
  cellSize: 1,
  minorGridSpacing: 1,
  majorGridInterval: 5,
  tensorPadding: 1,
  labelMargin: 1,
  framePadding: 1,
  operandGap: 4,
  axisMargin: 1,
  depthSpacing: 0,
  origin: { x: 0, y: 0, z: 0 },
}

export function snapToGrid(value: number, cellSize: number, eps = 1e-9): number {
  if (cellSize === 0) return value
  const n = Math.round(value / cellSize)
  return n * cellSize
}

export function isGridSnapped(value: number, cellSize: number, tol = 1e-6): boolean {
  if (cellSize === 0) return true
  const r = Math.abs(value / cellSize - Math.round(value / cellSize))
  return r <= tol
}

/** Cell center in local tensor coordinates (row i, col j). */
export function cellCenterLocal(
  i: number,
  j: number,
  config: GridRuler3DConfig,
): Vec3 {
  const { cellSize, tensorPadding } = config
  return {
    x: snapToGrid(tensorPadding + j * cellSize, cellSize),
    y: snapToGrid(tensorPadding + i * cellSize, cellSize),
    z: 0,
  }
}

/** Axis-aligned extent of an m×n tensor frame in local coords (before rotation). */
export function localTensorExtent(
  rows: number,
  cols: number,
  config: GridRuler3DConfig,
): TensorExtents {
  const { cellSize, tensorPadding, framePadding } = config
  const pad = tensorPadding + framePadding
  return {
    x: cols * cellSize + 2 * pad - cellSize,
    y: rows * cellSize + 2 * pad - cellSize,
    z: 0,
  }
}

/**
 * Multiplication-volume extent in world units (pre-rotation local sizes of A/B/C).
 * Matches legacy mm: extent uses display sizes + 2*gap - 1.
 */
export function mulVolumeExtent(
  m: number,
  k: number,
  n: number,
  config: GridRuler3DConfig,
): Vec3 {
  const gap = config.operandGap
  const cs = config.cellSize
  return {
    x: n * cs + 2 * gap - cs,
    y: m * cs + 2 * gap - cs,
    z: k * cs + 2 * gap - cs,
  }
}

/**
 * Place A (I×K), B (K×J), C (I×J) forming the multiplication corner.
 * Preserves the proven mm polarity/placement semantics, expressed via GridRuledLines.
 */
export function placeOperands(
  m: number,
  k: number,
  n: number,
  config: GridRuler3DConfig = DEFAULT_GRID_RULER,
  hints: PlacementHints = {
    polarity: 'negative',
    leftPlacement: 'left',
    rightPlacement: 'top',
    resultPlacement: 'front',
  },
): { A: PlaneTransform; B: PlaneTransform; C: PlaneTransform; extent: Vec3 } {
  const cs = config.cellSize
  const leftScatter = hints.leftScatter ?? 0
  const rightScatter = hints.rightScatter ?? 0
  const extent = mulVolumeExtent(m, k, n, config)

  // Leaf Mat extents use z=0 in local space (mm Mat.getExtent); preserve that.
  const leftExtentZ = 0
  const rightExtentZ = 0

  const positive = hints.polarity === 'positive'
  const leftIsLeft = hints.leftPlacement === 'left'
  const rightIsTop = hints.rightPlacement === 'top'
  const resultFront = hints.resultPlacement === 'front'

  let A: PlaneTransform
  if (positive) {
    A = {
      position: {
        x: leftIsLeft
          ? snapToGrid(-leftScatter, cs)
          : snapToGrid(extent.x + leftExtentZ + leftScatter, cs),
        y: 0,
        z: 0,
      },
      rotation: { x: 0, y: -Math.PI / 2, z: 0 },
      axes: { h: 'I', w: 'K' },
    }
  } else {
    A = {
      position: {
        x: leftIsLeft
          ? snapToGrid(-(leftExtentZ + leftScatter), cs)
          : snapToGrid(extent.x + leftScatter, cs),
        y: 0,
        z: snapToGrid(extent.z, cs),
      },
      rotation: { x: 0, y: Math.PI / 2, z: 0 },
      axes: { h: 'I', w: 'K' },
    }
  }

  let B: PlaneTransform
  if (positive) {
    B = {
      position: {
        x: 0,
        y: rightIsTop
          ? snapToGrid(-rightScatter, cs)
          : snapToGrid(extent.y + rightExtentZ + rightScatter, cs),
        z: 0,
      },
      rotation: { x: Math.PI / 2, y: 0, z: 0 },
      axes: { h: 'K', w: 'J' },
    }
  } else {
    B = {
      position: {
        x: 0,
        y: rightIsTop
          ? snapToGrid(-(rightExtentZ + rightScatter), cs)
          : snapToGrid(extent.y + rightScatter, cs),
        z: snapToGrid(extent.z, cs),
      },
      rotation: { x: -Math.PI / 2, y: 0, z: 0 },
      axes: { h: 'K', w: 'J' },
    }
  }

  const C: PlaneTransform = {
    position: {
      x: 0,
      y: 0,
      z: resultFront ? 0 : snapToGrid(extent.z, cs),
    },
    rotation: { x: 0, y: 0, z: 0 },
    axes: { h: 'I', w: 'J' },
  }

  return { A, B, C, extent }
}

/** Camera look targets for MVP presets (relative to centered scene). */
export type CameraPreset = 'isometric' | 'front' | 'top' | 'volume'

export function cameraPresetPose(
  preset: CameraPreset,
  extent: Vec3,
): { position: Vec3; target: Vec3 } {
  const mag = Math.max(extent.x, extent.y, extent.z, 1) * 1.8
  const target = { x: 0, y: 0, z: 0 }
  switch (preset) {
    case 'front':
      return { position: { x: 0, y: 0, z: mag }, target }
    case 'top':
      return { position: { x: 0, y: -mag, z: 0.01 }, target }
    case 'volume':
      return {
        position: { x: -mag * 0.9, y: mag * 0.7, z: mag * 0.9 },
        target,
      }
    case 'isometric':
    default:
      return {
        position: { x: -mag * 0.85, y: mag * 0.85, z: mag * 0.85 },
        target,
      }
  }
}

/** Build GridRuledLinesConfig from legacy layout params. */
export function gridRulerFromParams(layout: {
  gap?: number
  cellSize?: number
}): GridRuler3DConfig {
  return {
    ...DEFAULT_GRID_RULER,
    operandGap: layout.gap ?? DEFAULT_GRID_RULER.operandGap,
    cellSize: layout.cellSize ?? DEFAULT_GRID_RULER.cellSize,
  }
}

/**
 * Documented snap tolerance.
 *
 * Positions are built by repeated addition of `cellSize`, so f64 rounding can
 * leave a residue on the order of 1e-15 per operation. 1e-6 of a cell is well
 * above that and far below anything visible at any zoom level.
 */
export const GRID_SNAP_TOLERANCE = 1e-6

/**
 * A bound ruler: a config plus the operations that respect it.
 *
 * Prefer this over the free functions when several calls share one config —
 * it makes it impossible to snap against one cell size and place against
 * another.
 */
export class GridRuler3D {
  constructor(readonly config: GridRuler3DConfig = DEFAULT_GRID_RULER) {}

  static fromParams(layout: { gap?: number; cellSize?: number }): GridRuler3D {
    return new GridRuler3D(gridRulerFromParams(layout))
  }

  get cellSize(): number {
    return this.config.cellSize
  }

  snap(value: number): number {
    return snapToGrid(value, this.config.cellSize)
  }

  isSnapped(value: number, tol = GRID_SNAP_TOLERANCE): boolean {
    return isGridSnapped(value, this.config.cellSize, tol)
  }

  /** Throw if `value` is off-grid. Used at layout boundaries, not per cell. */
  assertSnapped(value: number, what = 'position'): number {
    if (!this.isSnapped(value)) {
      throw new Error(
        `${what} ${value} is not a multiple of cellSize ${this.config.cellSize} ` +
          `(tolerance ${GRID_SNAP_TOLERANCE})`,
      )
    }
    return value
  }

  /** Throw if any component of `v` is off-grid. */
  assertVecSnapped(v: Vec3, what = 'position'): Vec3 {
    this.assertSnapped(v.x, `${what}.x`)
    this.assertSnapped(v.y, `${what}.y`)
    this.assertSnapped(v.z, `${what}.z`)
    return v
  }

  cellCenter(i: number, j: number): Vec3 {
    return cellCenterLocal(i, j, this.config)
  }

  tensorExtent(rows: number, cols: number): TensorExtents {
    return localTensorExtent(rows, cols, this.config)
  }

  volumeExtent(m: number, k: number, n: number): Vec3 {
    return mulVolumeExtent(m, k, n, this.config)
  }

  place(m: number, k: number, n: number, hints?: PlacementHints) {
    return hints
      ? placeOperands(m, k, n, this.config, hints)
      : placeOperands(m, k, n, this.config)
  }
}

// --- back-compatible aliases -------------------------------------------------
// Previous names for this module. Kept so existing imports keep working; new
// code should use `GridRuler3D` / `GridRuler3DConfig` / `DEFAULT_GRID_RULER`.
export type GridRuledLinesConfig = GridRuler3DConfig
export type MarginGridConfig = GridRuler3DConfig
export const DEFAULT_GRID_RULED_LINES = DEFAULT_GRID_RULER
export const DEFAULT_MARGIN_GRID = DEFAULT_GRID_RULER
export const gridRuledLinesFromParams = gridRulerFromParams
export const marginGridFromParams = gridRulerFromParams

