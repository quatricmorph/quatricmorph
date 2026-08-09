//
// address.ts — the pure range algebra the whole editor stands on.
//
// Everything here is hand-computed: the disjointness invariant of the range
// set is what lets selection counts be sums and lets edits touch each cell
// exactly once, so the failure this file guards against is an overlap that
// double-counts or double-applies without anything on screen looking wrong.
//
import { describe, it, expect } from 'vitest'
import {
  cellRange, cellAt, fullRange, clampRange, rangeCount, rangesContain,
  subtractRange, addToRanges, subtractFromRanges, toggleRange, invertRanges,
  rangesCover, countCells, forEachCell, fmtValue, formatRange, formatRanges,
} from '../src/address.js'

describe('subtractRange', () => {
  it('returns the whole of a when b misses it', () => {
    const a = cellRange(0, 2, 0, 2)
    expect(subtractRange(a, cellRange(5, 6, 5, 6))).toEqual([a])
  })

  it('returns nothing when b covers a completely', () => {
    expect(subtractRange(cellRange(1, 2, 1, 2), cellRange(0, 4, 0, 4))).toEqual([])
  })

  it('decomposes a ring into 4 disjoint rects whose areas sum to 16 − 4 = 12', () => {
    // a = [0,4)×[0,4) is 16 cells, b = [1,3)×[1,3) is 4 cells strictly inside.
    const parts = subtractRange(cellRange(0, 4, 0, 4), cellRange(1, 3, 1, 3))
    expect(parts).toHaveLength(4)
    expect(parts.reduce((n, r) => n + rangeCount(r), 0)).toBe(12)
    // disjoint: no cell of b, every ring cell exactly once
    const seen = new Set<string>()
    for (const r of parts) {
      forEachCell([r], (i, j) => {
        const k = `${i},${j}`
        expect(seen.has(k)).toBe(false)
        seen.add(k)
      })
    }
    expect(seen.has('1,1')).toBe(false)
    expect(seen.has('0,0')).toBe(true)
  })
})

describe('addToRanges / countCells', () => {
  it('keeps the set disjoint under overlapping adds: 4 + 4 − 1 shared = 7 cells', () => {
    let rs = addToRanges([], cellRange(0, 2, 0, 2))
    rs = addToRanges(rs, cellRange(1, 3, 1, 3))
    expect(countCells(rs)).toBe(7)
    // and forEachCell visits each exactly once
    const seen = new Set<string>()
    forEachCell(rs, (i, j) => {
      const k = `${i},${j}`
      expect(seen.has(k)).toBe(false)
      seen.add(k)
    })
    expect(seen.size).toBe(7)
    expect(rangesContain(rs, 1, 1)).toBe(true)
    expect(rangesContain(rs, 3, 3)).toBe(false)
  })
})

describe('toggleRange', () => {
  it('carves out a fully-covered sub-range (count drops by its area)', () => {
    const rs = toggleRange([cellRange(0, 4, 0, 4)], cellRange(1, 3, 1, 3))
    expect(countCells(rs)).toBe(12)
    expect(rangesContain(rs, 2, 2)).toBe(false)
  })

  it('completes a half-covered range rather than carving it', () => {
    // [0,2)×[0,2) selected; toggling [0,2)×[1,3) is not fully covered → add.
    const rs = toggleRange([cellRange(0, 2, 0, 2)], cellRange(0, 2, 1, 3))
    expect(rangesCover(rs, cellRange(0, 2, 1, 3))).toBe(true)
    expect(countCells(rs)).toBe(6)   // [0,2)×[0,3)
  })
})

describe('invertRanges', () => {
  it('inverting the centre of a 4×4 leaves the 12-cell ring', () => {
    const inv = invertRanges([cellRange(1, 3, 1, 3)], 4, 4)
    expect(countCells(inv)).toBe(12)
    expect(rangesContain(inv, 1, 1)).toBe(false)
    expect(rangesContain(inv, 2, 2)).toBe(false)
    expect(rangesContain(inv, 0, 0)).toBe(true)
    expect(rangesContain(inv, 3, 3)).toBe(true)
  })
})

describe('rangesCover', () => {
  it('recognizes exact cover assembled from two adjacent pieces', () => {
    const rs = [cellRange(0, 2, 0, 1), cellRange(0, 2, 1, 2)]
    expect(rangesCover(rs, cellRange(0, 2, 0, 2))).toBe(true)
    expect(rangesCover(rs, cellRange(0, 2, 0, 3))).toBe(false)
  })
})

describe('clampRange', () => {
  it('orders reversed endpoints and clips to the matrix', () => {
    // reversed on both axes, extending past the 3×3 bounds
    expect(clampRange({ i: [5, -1], j: [4, 1] } as any, 3, 3))
      .toEqual(cellRange(0, 3, 1, 3))
  })

  it('returns null when nothing survives the clip', () => {
    expect(clampRange(cellRange(5, 7, 0, 2), 3, 3)).toBeNull()
  })
})

describe('formatting', () => {
  it('fmtValue keeps readable magnitudes exact and pushes the rest to exponential', () => {
    expect(fmtValue(0)).toBe('0')
    // 0.5 has 5 significant digits under toPrecision(5): '0.50000'
    expect(fmtValue(0.5)).toBe('0.50000')
    // ≥ 1e4 goes exponential with 3 fractional digits; 1.2345 rounds up
    expect(fmtValue(12345)).toBe('1.235e+4')
    // < 1e-3 likewise
    expect(fmtValue(0.0005)).toBe('5.000e-4')
    expect(fmtValue(NaN)).toBe('NaN')
    expect(fmtValue(Infinity)).toBe('Infinity')
  })

  it('formatRange prints single cells as [i, j] and runs as [a:b, c:d]', () => {
    expect(formatRange(cellAt(3, 5))).toBe('[3, 5]')
    expect(formatRange(cellRange(0, 2, 1, 4))).toBe('[0:2, 1:4]')
  })

  it('formatRanges elides beyond the limit and says how many were dropped', () => {
    const rs = [cellAt(0, 0), cellAt(1, 1)]
    expect(formatRanges(rs, 1)).toBe('[0, 0] ∪ …1 more')
  })

  it('fullRange of h×w counts h·w cells', () => {
    expect(rangeCount(fullRange(3, 5))).toBe(15)
  })
})
