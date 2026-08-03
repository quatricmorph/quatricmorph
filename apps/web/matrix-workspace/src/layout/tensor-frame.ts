import type { GridRuledLinesConfig, Vec3 } from './grid-ruler.js'
import { localTensorExtent } from './grid-ruler.js'

/**
 * TensorMarginFrame metrics — outer frame, title/shape label margins (VIZ-05).
 */
export type TensorMarginFrame = {
  name: string
  rows: number
  cols: number
  title: string
  /** Outer AABB in local tensor coords. */
  outerMin: Vec3
  outerMax: Vec3
  /** Inner content region (cells). */
  innerMin: Vec3
  innerMax: Vec3
}

export function buildTensorFrame(
  name: string,
  rows: number,
  cols: number,
  config: GridRuledLinesConfig,
): TensorMarginFrame {
  const ext = localTensorExtent(rows, cols, config)
  const { tensorPadding, labelMargin, framePadding } = config
  const outerPad = framePadding + labelMargin
  return {
    name,
    rows,
    cols,
    title: `${name} [${rows} × ${cols}]`,
    outerMin: { x: -outerPad, y: -outerPad, z: 0 },
    outerMax: { x: ext.x + outerPad, y: ext.y + outerPad, z: 0 },
    innerMin: { x: tensorPadding, y: tensorPadding, z: 0 },
    innerMax: {
      x: tensorPadding + (cols - 1) * config.cellSize,
      y: tensorPadding + (rows - 1) * config.cellSize,
      z: 0,
    },
  }
}

export function frameContainsPoint(
  frame: TensorMarginFrame,
  p: Vec3,
  tol = 1e-6,
): boolean {
  return (
    p.x >= frame.outerMin.x - tol &&
    p.x <= frame.outerMax.x + tol &&
    p.y >= frame.outerMin.y - tol &&
    p.y <= frame.outerMax.y + tol
  )
}
