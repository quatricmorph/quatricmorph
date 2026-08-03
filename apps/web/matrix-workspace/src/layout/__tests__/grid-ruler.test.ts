import { describe, expect, it } from 'vitest'
import {
  DEFAULT_GRID_RULED_LINES,
  isGridSnapped,
  cellCenterLocal,
  placeOperands,
  mulVolumeExtent,
  buildTensorFrame,
  frameContainsPoint,
  cameraPresetPose,
  snapToGrid,
} from '../index.js'

describe('VIZ-03 / VIZ-04 GridRuledLines3D', () => {
  it('snaps positions to cellSize', () => {
    expect(snapToGrid(3.2, 1)).toBe(3)
    expect(isGridSnapped(4, 1)).toBe(true)
    expect(isGridSnapped(4.0000001, 1)).toBe(true)
    expect(isGridSnapped(4.1, 1, 1e-3)).toBe(false)
  })

  it('places A/B/C with I/J/K axis labels', () => {
    const { A, B, C, extent } = placeOperands(2, 3, 2, DEFAULT_GRID_RULED_LINES)
    expect(A.axes).toEqual({ h: 'I', w: 'K' })
    expect(B.axes).toEqual({ h: 'K', w: 'J' })
    expect(C.axes).toEqual({ h: 'I', w: 'J' })
    expect(isGridSnapped(A.position.x, 1)).toBe(true)
    expect(isGridSnapped(A.position.y, 1)).toBe(true)
    expect(isGridSnapped(A.position.z, 1)).toBe(true)
    expect(isGridSnapped(B.position.x, 1)).toBe(true)
    expect(isGridSnapped(C.position.z, 1)).toBe(true)
    expect(extent.z).toBe(mulVolumeExtent(2, 3, 2, DEFAULT_GRID_RULED_LINES).z)
  })

  it('aligns C and A on I (same local row axis) and C and B on J', () => {
    const { A, B, C } = placeOperands(4, 5, 6)
    expect(A.axes.h).toBe(C.axes.h) // I
    expect(B.axes.w).toBe(C.axes.w) // J
    expect(A.axes.w).toBe(B.axes.h) // K
  })

  it('cell centers are grid-snapped', () => {
    const c = cellCenterLocal(1, 2, DEFAULT_GRID_RULED_LINES)
    expect(isGridSnapped(c.x, DEFAULT_GRID_RULED_LINES.cellSize)).toBe(true)
    expect(isGridSnapped(c.y, DEFAULT_GRID_RULED_LINES.cellSize)).toBe(true)
  })

  it('tensor frames contain cell centers', () => {
    const frame = buildTensorFrame('A', 2, 3, DEFAULT_GRID_RULED_LINES)
    expect(frame.title).toBe('A [2 × 3]')
    const c = cellCenterLocal(0, 0, DEFAULT_GRID_RULED_LINES)
    expect(frameContainsPoint(frame, c)).toBe(true)
  })

  it('vectors and scalars use same frame system', () => {
    const col = buildTensorFrame('v', 3, 1, DEFAULT_GRID_RULED_LINES)
    const row = buildTensorFrame('u', 1, 3, DEFAULT_GRID_RULED_LINES)
    const sc = buildTensorFrame('s', 1, 1, DEFAULT_GRID_RULED_LINES)
    expect(col.cols).toBe(1)
    expect(row.rows).toBe(1)
    expect(sc.rows).toBe(1)
    expect(sc.cols).toBe(1)
  })

  it('camera presets return finite poses', () => {
    const ext = mulVolumeExtent(2, 3, 2, DEFAULT_GRID_RULED_LINES)
    for (const p of ['isometric', 'front', 'top', 'volume'] as const) {
      const pose = cameraPresetPose(p, ext)
      expect(Number.isFinite(pose.position.x)).toBe(true)
      expect(Number.isFinite(pose.target.y)).toBe(true)
    }
  })
})
