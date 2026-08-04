import { describe, expect, it } from 'vitest'
import { blockInfo, gridSpans, scatterFromCount } from '../blocking.js'

describe('GRID-02 block decomposition', () => {
  it('divides evenly when the request divides the axis', () => {
    const info = blockInfo(8, 8, 8, { i: 2, k: 4, j: 1 })
    expect(info.i).toEqual({ n: 2, size: 4, max: 8 })
    expect(info.k).toEqual({ n: 4, size: 2, max: 8 })
    expect(info.j).toEqual({ n: 1, size: 8, max: 8 })
  })

  it('clamps a request for more blocks than elements', () => {
    // Asking for 16 blocks of a 4-row tensor must give 4 blocks of 1 row,
    // not 16 blocks of which 12 are empty.
    const info = blockInfo(4, 4, 4, { i: 16, k: 16, j: 16 })
    expect(info.i).toEqual({ n: 4, size: 1, max: 4 })
  })

  it('never produces zero blocks', () => {
    const info = blockInfo(4, 4, 4, { i: 0, k: -3, j: 1 })
    expect(info.i.n).toBe(1)
    expect(info.k.n).toBe(1)
  })

  it('covers every element exactly once along an axis', () => {
    const info = blockInfo(7, 1, 1, { i: 3, k: 1, j: 1 })
    const spans = gridSpans(info, 'i').map(([s]) => s)
    const covered: number[] = []
    for (const s of spans) {
      for (let x = s.start; x < s.end; x++) covered.push(x)
    }
    expect(covered).toEqual([0, 1, 2, 3, 4, 5, 6])
  })

  it('skips a dead trailing block when size * n overruns the axis', () => {
    // 4 elements in 3 blocks => size 2, so blocks start at 0, 2, 4; the third
    // starts past the end and must not be yielded.
    const info = blockInfo(4, 1, 1, { i: 3, k: 1, j: 1 })
    expect(info.i).toEqual({ n: 3, size: 2, max: 4 })
    const spans = gridSpans(info, 'i')
    expect(spans.length).toBe(2)
    expect(spans.map(([s]) => [s.start, s.end])).toEqual([
      [0, 2],
      [2, 4],
    ])
  })

  it('iterates multiple axes in row-major order', () => {
    const info = blockInfo(2, 2, 2, { i: 2, k: 1, j: 2 })
    const seen = gridSpans(info, 'ij').map(([i, j]) => [i.index, j.index])
    expect(seen).toEqual([
      [0, 0],
      [0, 1],
      [1, 0],
      [1, 1],
    ])
  })

  it('honours the axis order given in dims', () => {
    const info = blockInfo(2, 3, 4, { i: 2, k: 3, j: 4 })
    const [first] = gridSpans(info, 'kij')[0]
    expect(first.end - first.start).toBe(1) // the k axis leads
  })
})

describe('scatter multiplier', () => {
  it('is zero below the molecule threshold', () => {
    expect(scatterFromCount(1, 10, { scatter: 5, molecule: 4, blast: 1 })).toBe(0)
  })

  it('grows with count for a positive blast', () => {
    const layout = { scatter: 2, molecule: 0, blast: 1 }
    expect(scatterFromCount(3, 10, layout)).toBe(6)
    expect(scatterFromCount(5, 10, layout)).toBe(10)
  })

  it('inverts against the total for a negative blast', () => {
    // blast = -1 => (total - count) ** 1
    expect(scatterFromCount(4, 10, { scatter: 1, molecule: 0, blast: -1 })).toBe(6)
  })
})
