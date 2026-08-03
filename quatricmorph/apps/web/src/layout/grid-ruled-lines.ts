/**
 * GridRuledLines3D — shared spatial layout authority (VIZ-03, VIZ-04).
 *
 * World axes: X → J (output cols), Y → I (output rows), Z → K (contraction).
 * Planes: A on I×K, B on K×J, C on I×J. Positions snap to cellSize multiples.
 */

export type Vec3 = { x: number; y: number; z: number }

export type GridRuledLinesConfig = {
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

export const DEFAULT_GRID_RULED_LINES: GridRuledLinesConfig = {
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
  config: GridRuledLinesConfig,
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
  config: GridRuledLinesConfig,
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
  config: GridRuledLinesConfig,
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
  config: GridRuledLinesConfig = DEFAULT_GRID_RULED_LINES,
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
export function gridRuledLinesFromParams(layout: {
  gap?: number
  cellSize?: number
}): GridRuledLinesConfig {
  return {
    ...DEFAULT_GRID_RULED_LINES,
    operandGap: layout.gap ?? DEFAULT_GRID_RULED_LINES.operandGap,
    cellSize: layout.cellSize ?? DEFAULT_GRID_RULED_LINES.cellSize,
  }
}

/** Product alias: MarginGrid3D ≡ GridRuledLines3D (VIZ-03). */
export type MarginGridConfig = GridRuledLinesConfig
export const DEFAULT_MARGIN_GRID = DEFAULT_GRID_RULED_LINES
export const marginGridFromParams = gridRuledLinesFromParams

