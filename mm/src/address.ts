"use strict"

//
// Tensor addressing: the logical vocabulary of the interaction system.
//
// A selection, a hover, a cursor and an edit all name the same thing: an
// entity in the scene tree (by stable path, never by THREE object identity —
// objects are disposed and rebuilt on every initViz) plus, below matrix level,
// a set of cell ranges inside that entity's Array2D.
//
// Everything here is pure index arithmetic over half-open ranges. No THREE, no
// DOM, no renderer: this module is the contract the rest of the editor shares,
// and it has to be testable by hand-computed cases alone.
//

/**
 * Sub-matrix granularity a pointer interaction resolves to. Ordered coarse →
 * fine; the keyboard cycles through them in this order.
 */
export const LEVELS = ['matrix', 'block', 'row', 'col', 'scalar'] as const
export type Level = typeof LEVELS[number]

/**
 * A rectangular, axis-aligned run of cells: rows [i0, i1) × cols [j0, j1).
 * Half-open on both axes, matching viz.ts's own `toRange` convention, so a
 * whole h×w matrix is exactly {i: [0, h], j: [0, w]} with no ±1 anywhere.
 */
export interface CellRange {
  i: [number, number]
  j: [number, number]
}

export const cellRange = (i0: number, i1: number, j0: number, j1: number): CellRange =>
  ({ i: [i0, i1], j: [j0, j1] })

/** The single cell (i, j) as a range. */
export const cellAt = (i: number, j: number): CellRange => cellRange(i, i + 1, j, j + 1)

export const fullRange = (h: number, w: number): CellRange => cellRange(0, h, 0, w)

export const isEmptyRange = (r: CellRange): boolean =>
  r.i[0] >= r.i[1] || r.j[0] >= r.j[1]

/** Order the endpoints and clip to the matrix. Returns null when nothing is left. */
export function clampRange(r: CellRange, h: number, w: number): CellRange | null {
  const lo = (x: number, y: number) => Math.min(x, y)
  const hi = (x: number, y: number) => Math.max(x, y)
  const c = cellRange(
    Math.max(0, lo(r.i[0], r.i[1])), Math.min(h, hi(r.i[0], r.i[1])),
    Math.max(0, lo(r.j[0], r.j[1])), Math.min(w, hi(r.j[0], r.j[1])))
  return isEmptyRange(c) ? null : c
}

export const rangeCount = (r: CellRange): number =>
  Math.max(0, r.i[1] - r.i[0]) * Math.max(0, r.j[1] - r.j[0])

export const rangeContains = (r: CellRange, i: number, j: number): boolean =>
  i >= r.i[0] && i < r.i[1] && j >= r.j[0] && j < r.j[1]

/** Does `outer` contain every cell of `inner`? */
export const rangeCovers = (outer: CellRange, inner: CellRange): boolean =>
  outer.i[0] <= inner.i[0] && outer.i[1] >= inner.i[1] &&
  outer.j[0] <= inner.j[0] && outer.j[1] >= inner.j[1]

export const rangesIntersect = (a: CellRange, b: CellRange): boolean =>
  a.i[0] < b.i[1] && b.i[0] < a.i[1] && a.j[0] < b.j[1] && b.j[0] < a.j[1]

export function intersectRange(a: CellRange, b: CellRange): CellRange | null {
  const r = cellRange(
    Math.max(a.i[0], b.i[0]), Math.min(a.i[1], b.i[1]),
    Math.max(a.j[0], b.j[0]), Math.min(a.j[1], b.j[1]))
  return isEmptyRange(r) ? null : r
}

/**
 * a − b as up to four disjoint rectangles (the band above b, the band below,
 * and the left/right flanks of the overlapping rows). The decomposition is
 * what lets a list of ranges stay disjoint under toggle/subtract, which in
 * turn is what makes counting cells a plain sum instead of an inclusion-
 * exclusion problem.
 */
export function subtractRange(a: CellRange, b: CellRange): CellRange[] {
  const x = intersectRange(a, b)
  if (!x) return [a]
  const out: CellRange[] = []
  if (a.i[0] < x.i[0]) out.push(cellRange(a.i[0], x.i[0], a.j[0], a.j[1]))
  if (x.i[1] < a.i[1]) out.push(cellRange(x.i[1], a.i[1], a.j[0], a.j[1]))
  if (a.j[0] < x.j[0]) out.push(cellRange(x.i[0], x.i[1], a.j[0], x.j[0]))
  if (x.j[1] < a.j[1]) out.push(cellRange(x.i[0], x.i[1], x.j[1], a.j[1]))
  return out
}

//
// RangeSet: a list of ranges kept pairwise disjoint by construction.
//
// Disjointness is the invariant every consumer leans on: `count` can sum
// areas, `forEachCell` visits each cell exactly once (edits must not apply an
// op twice), and the highlight pass draws each region once with no z-fighting
// between overlapping quads.
//

export function addToRanges(ranges: CellRange[], r: CellRange): CellRange[] {
  // Keep the existing ranges; add only the parts of `r` not already covered.
  let parts = [r]
  for (const q of ranges) {
    const next: CellRange[] = []
    for (const p of parts) next.push(...subtractRange(p, q))
    parts = next
    if (!parts.length) break
  }
  return ranges.concat(parts)
}

export function subtractFromRanges(ranges: CellRange[], r: CellRange): CellRange[] {
  const out: CellRange[] = []
  for (const q of ranges) out.push(...subtractRange(q, r))
  return out
}

/** Is every cell of `r` already inside the set? */
export function rangesCover(ranges: CellRange[], r: CellRange): boolean {
  let parts = [r]
  for (const q of ranges) {
    const next: CellRange[] = []
    for (const p of parts) next.push(...subtractRange(p, q))
    parts = next
    if (!parts.length) return true
  }
  return parts.length === 0
}

/**
 * Blender-style toggle: a fully-selected region deselects, anything else
 * selects the missing part.
 */
export function toggleRange(ranges: CellRange[], r: CellRange): CellRange[] {
  return rangesCover(ranges, r) ? subtractFromRanges(ranges, r) : addToRanges(ranges, r)
}

export function invertRanges(ranges: CellRange[], h: number, w: number): CellRange[] {
  let parts: CellRange[] = [fullRange(h, w)]
  for (const q of ranges) {
    const next: CellRange[] = []
    for (const p of parts) next.push(...subtractRange(p, q))
    parts = next
  }
  return parts
}

export const countCells = (ranges: CellRange[]): number =>
  ranges.reduce((n, r) => n + rangeCount(r), 0)

export const rangesContain = (ranges: CellRange[], i: number, j: number): boolean =>
  ranges.some(r => rangeContains(r, i, j))

/** Visit every cell of a disjoint set exactly once. Row-major within a range. */
export function forEachCell(ranges: CellRange[], f: (i: number, j: number) => void) {
  for (const r of ranges) {
    for (let i = r.i[0]; i < r.i[1]; i++) {
      for (let j = r.j[0]; j < r.j[1]; j++) {
        f(i, j)
      }
    }
  }
}

//
// Formatting. One place, so the tooltip, the inspector, the outliner and the
// breadcrumb print an address the same way.
//

/** viz.ts's fmt, shared: exact for readable magnitudes, exponential outside. */
export const fmtValue = (x: number): string =>
  !isFinite(x) ? String(x)
    : x === 0 ? '0'
      : Math.abs(x) >= 1e4 || Math.abs(x) < 1e-3 ? x.toExponential(3) : x.toPrecision(5)

export const formatCell = (name: string, i: number, j: number): string =>
  `${name}[${i}, ${j}]`

export function formatRange(r: CellRange): string {
  const seg = (a: number, b: number, n?: number) =>
    b - a === 1 ? String(a) : `${a}:${b}`
  return `[${seg(r.i[0], r.i[1])}, ${seg(r.j[0], r.j[1])}]`
}

export function formatRanges(ranges: CellRange[], limit = 3): string {
  const parts = ranges.slice(0, limit).map(formatRange)
  const more = ranges.length - limit
  return parts.join(' ∪ ') + (more > 0 ? ` ∪ …${more} more` : '')
}

/** `out / attn / K_t / result` from a path's segments, for the breadcrumb. */
export const formatPath = (path: string): string => path.split('/').join(' / ')
