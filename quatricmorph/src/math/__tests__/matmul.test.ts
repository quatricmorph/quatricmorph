import { describe, expect, it } from 'vitest'
import { Array2D } from '../../viz/array2d.js'
import { matmul, dotprodCell } from '../matmul.js'
import { validateMatmulDims } from '../validate.js'
import { inferTensorKind, shapeLabel } from '../shape.js'
import { parseMatrixText, matrixToText } from '../parse.js'
import { DEFAULT_A, DEFAULT_B, DEFAULT_C, fillPreset } from '../presets.js'

describe('VIZ-01 validateMatmulDims', () => {
  it('accepts compatible shapes', () => {
    const r = validateMatmulDims(2, 3, 3, 2)
    expect(r.ok).toBe(true)
    if (r.ok) {
      expect(r.m).toBe(2)
      expect(r.k).toBe(3)
      expect(r.n).toBe(2)
    }
  })

  it('rejects 2×3 @ 2×2 with clear message', () => {
    const r = validateMatmulDims(2, 3, 2, 2)
    expect(r.ok).toBe(false)
    if (!r.ok) {
      expect(r.message).toMatch(/A\.columns \(3\) must equal B\.rows \(2\)/)
    }
  })
})

describe('VIZ-02 matmul cases', () => {
  it('2×3 @ 3×2 → 2×2 default example', () => {
    const a = Array2D.fromInit(2, 3, (i, j) => DEFAULT_A[i][j])
    const b = Array2D.fromInit(3, 2, (i, j) => DEFAULT_B[i][j])
    const c = matmul(a, b)
    expect(c.h).toBe(2)
    expect(c.w).toBe(2)
    expect(c.get(0, 0)).toBe(DEFAULT_C[0][0])
    expect(c.get(0, 1)).toBe(DEFAULT_C[0][1])
    expect(c.get(1, 0)).toBe(DEFAULT_C[1][0])
    expect(c.get(1, 1)).toBe(DEFAULT_C[1][1])
  })

  it('3×3 @ 3×1 → 3×1 column', () => {
    const a = Array2D.fromInit(3, 3, (i, j) => i * 3 + j + 1)
    const b = Array2D.fromInit(3, 1, (i) => i + 1)
    const c = matmul(a, b)
    expect(c.h).toBe(3)
    expect(c.w).toBe(1)
    expect(inferTensorKind(c.h, c.w)).toBe('column')
    expect(c.get(0, 0)).toBe(1 * 1 + 2 * 2 + 3 * 3)
  })

  it('1×3 @ 3×2 → 1×2 row', () => {
    const a = Array2D.fromInit(1, 3, (_i, j) => j + 1)
    const b = Array2D.fromInit(3, 2, (i, j) => (i + 1) * (j + 1))
    const c = matmul(a, b)
    expect(c.h).toBe(1)
    expect(c.w).toBe(2)
    expect(inferTensorKind(c.h, c.w)).toBe('row')
  })

  it('1×3 @ 3×1 → 1×1 scalar', () => {
    const a = Array2D.fromInit(1, 3, (_i, j) => [1, 2, 3][j])
    const b = Array2D.fromInit(3, 1, (i) => [4, 5, 6][i])
    const c = matmul(a, b)
    expect(c.h).toBe(1)
    expect(c.w).toBe(1)
    expect(inferTensorKind(c.h, c.w)).toBe('scalar')
    expect(c.get(0, 0)).toBe(1 * 4 + 2 * 5 + 3 * 6)
  })

  it('1×1 @ 1×1 → 1×1', () => {
    const a = Array2D.fromInit(1, 1, () => 3)
    const b = Array2D.fromInit(1, 1, () => 4)
    const c = matmul(a, b)
    expect(c.get(0, 0)).toBe(12)
  })

  it('throws on invalid 2×3 @ 2×2', () => {
    const a = Array2D.fromInit(2, 3, () => 1)
    const b = Array2D.fromInit(2, 2, () => 1)
    expect(() => matmul(a, b)).toThrow(/Incompatible/)
  })

  it('handles negatives, zeros, decimals', () => {
    const a = Array2D.fromInit(2, 2, (i, j) => [[-1, 0.5], [0, 2]][i][j])
    const b = Array2D.fromInit(2, 2, (i, j) => [[3, -2], [4, 0.25]][i][j])
    const c = matmul(a, b)
    expect(c.get(0, 0)).toBeCloseTo(-1 * 3 + 0.5 * 4)
    expect(c.get(0, 1)).toBeCloseTo(-1 * -2 + 0.5 * 0.25)
    expect(c.get(1, 0)).toBe(8)
    expect(c.get(1, 1)).toBeCloseTo(0.5)
  })

  it('dotprodCell matches matmul entry', () => {
    const a = Array2D.fromInit(2, 3, (i, j) => DEFAULT_A[i][j])
    const b = Array2D.fromInit(3, 2, (i, j) => DEFAULT_B[i][j])
    const c = matmul(a, b)
    expect(dotprodCell(a, b, 1, 1)).toBe(c.get(1, 1))
  })
})

describe('shape labels', () => {
  it('formats titles', () => {
    expect(shapeLabel('A', 2, 3)).toBe('A [2 × 3]')
  })

  it('infers kinds', () => {
    expect(inferTensorKind(4, 4)).toBe('matrix')
    expect(inferTensorKind(4, 1)).toBe('column')
    expect(inferTensorKind(1, 4)).toBe('row')
    expect(inferTensorKind(1, 1)).toBe('scalar')
  })
})

describe('parseMatrixText', () => {
  it('parses comma/newline grids', () => {
    const r = parseMatrixText('1, 2, 3\n4, 5, 6')
    expect(r.ok).toBe(true)
    if (r.ok) {
      expect(r.rows).toBe(2)
      expect(r.cols).toBe(3)
      expect(r.data[1][2]).toBe(6)
    }
  })

  it('rejects malformed rows', () => {
    const r = parseMatrixText('1, 2\n3, 4, 5')
    expect(r.ok).toBe(false)
  })

  it('rejects non-numbers', () => {
    const r = parseMatrixText('1, two\n3, 4')
    expect(r.ok).toBe(false)
  })

  it('round-trips via matrixToText', () => {
    const text = matrixToText(DEFAULT_A)
    const r = parseMatrixText(text)
    expect(r.ok).toBe(true)
    if (r.ok) expect(r.data).toEqual(DEFAULT_A)
  })
})

describe('presets', () => {
  it('identity and zeros', () => {
    expect(fillPreset(2, 2, 'identity')).toEqual([[1, 0], [0, 1]])
    expect(fillPreset(2, 2, 'zeros')).toEqual([[0, 0], [0, 0]])
    expect(fillPreset(1, 3, 'ones')).toEqual([[1, 1, 1]])
  })
})
