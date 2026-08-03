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

// --- GridRuler3D bound API (GRID-001) ---------------------------------------

import { GridRuler3D, GRID_SNAP_TOLERANCE } from '../grid-ruler.js'

describe('GRID-001 GridRuler3D', () => {
  it('exposes every layout parameter the workspace needs', () => {
    const r = new GridRuler3D()
    for (const key of [
      'cellSize',
      'minorGridSpacing',
      'majorGridInterval',
      'tensorPadding',
      'labelMargin',
      'framePadding',
      'operandGap',
      'axisMargin',
      'depthSpacing',
      'origin',
    ] as const) {
      expect(r.config[key]).toBeDefined()
    }
  })

  it('snaps positions to cellSize multiples', () => {
    const r = new GridRuler3D({ ...new GridRuler3D().config, cellSize: 2 })
    expect(r.snap(3.4)).toBe(4)
    expect(r.snap(2.9)).toBe(2)
    expect(r.isSnapped(4)).toBe(true)
    expect(r.isSnapped(3)).toBe(false)
  })

  it('tolerates float accumulation within the documented tolerance', () => {
    const r = new GridRuler3D()
    // 0.1 summed ten times is 0.9999999999999999, not 1.
    let acc = 0
    for (let n = 0; n < 10; n++) acc += 0.1
    expect(acc).not.toBe(1)
    expect(r.isSnapped(acc)).toBe(true)
    expect(GRID_SNAP_TOLERANCE).toBe(1e-6)
  })

  it('assertSnapped names the offending value', () => {
    const r = new GridRuler3D()
    expect(() => r.assertSnapped(0.5, 'A.position.x')).toThrow(/A.position.x 0.5/)
    expect(r.assertSnapped(3)).toBe(3)
  })

  it('every operand placement it produces is on-grid', () => {
    const r = new GridRuler3D()
    const { A, B, C } = r.place(6, 4, 5)
    for (const [name, t] of [['A', A], ['B', B], ['C', C]] as const) {
      expect(() => r.assertVecSnapped(t.position, `${name}.position`)).not.toThrow()
    }
  })

  it('cell centres advance by exactly one cell', () => {
    const r = new GridRuler3D()
    const a = r.cellCenter(0, 0)
    const b = r.cellCenter(0, 1)
    expect(b.x - a.x).toBeCloseTo(r.cellSize, 12)
    expect(r.cellCenter(1, 0).y - a.y).toBeCloseTo(r.cellSize, 12)
  })
})
