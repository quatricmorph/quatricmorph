import { describe, expect, it } from 'vitest'
import { Array2D, toRange } from '../array2d.js'

describe('VIZ-02 Array2D basics', () => {
  it('stores and reads values by row-major address', () => {
    const a = Array2D.fromInit(2, 3, (i, j) => i * 10 + j)
    expect(a.get(0, 0)).toBe(0)
    expect(a.get(0, 2)).toBe(2)
    expect(a.get(1, 1)).toBe(11)
    expect(a.addr(1, 2)).toBe(5)
  })

  it('transposes correctly', () => {
    const a = Array2D.fromInit(2, 3, (i, j) => i * 10 + j)
    const t = a.transpose()
    expect(t.h).toBe(3)
    expect(t.w).toBe(2)
    expect(t.get(2, 1)).toBe(12)
  })

  it('adds matching shapes', () => {
    const a = Array2D.fromInit(2, 2, () => 1)
    const b = Array2D.fromInit(2, 2, () => 2)
    const c = a.add(b)
    expect(c.get(0, 0)).toBe(3)
    expect(c.get(1, 1)).toBe(3)
  })

  it('rejects add on mismatched shapes', () => {
    const a = Array2D.fromInit(2, 2, () => 1)
    const b = Array2D.fromInit(2, 3, () => 1)
    expect(() => a.add(b)).toThrow(/shape error/)
  })
})

describe('toRange', () => {
  it('covers full extent when undefined', () => {
    expect(toRange(undefined, 4)).toEqual([0, 4])
  })

  it('treats scalar as single index span', () => {
    expect(toRange(2, 4)).toEqual([2, 3])
  })

  it('passes through explicit ranges', () => {
    expect(toRange([1, 3], 4)).toEqual([1, 3])
  })
})

/** Reference multiply for VIZ-02 contract tests. */
function matmul(a: InstanceType<typeof Array2D>, b: InstanceType<typeof Array2D>) {
  if (a.w !== b.h) {
    throw new Error(`dims: A.cols ${a.w} !== B.rows ${b.h}`)
  }
  return Array2D.fromInit(a.h, b.w, (i, j) => {
    let s = 0
    for (let k = 0; k < a.w; k++) {
      s += a.get(i, k) * b.get(k, j)
    }
    return s
  })
}

describe('VIZ-01 / VIZ-02 matrix multiply contract', () => {
  it('computes deterministic A @ B', () => {
    const a = Array2D.fromInit(2, 3, (i, j) => i + j + 1)
    const b = Array2D.fromInit(3, 2, (i, j) => (i + 1) * (j + 1))
    const c = matmul(a, b)
    // Hand-checked:
    // A = [[1,2,3],[2,3,4]]
    // B = [[1,2],[2,4],[3,6]]
    // C[0,0] = 1+4+9 = 14
    // C[0,1] = 2+8+18 = 28
    // C[1,0] = 2+6+12 = 20
    // C[1,1] = 4+12+24 = 40
    expect(c.get(0, 0)).toBe(14)
    expect(c.get(0, 1)).toBe(28)
    expect(c.get(1, 0)).toBe(20)
    expect(c.get(1, 1)).toBe(40)
  })

  it('rejects incompatible dimensions', () => {
    const a = Array2D.fromInit(2, 3, () => 1)
    const b = Array2D.fromInit(2, 2, () => 1)
    expect(() => matmul(a, b)).toThrow(/dims/)
  })
})
