/** Persistent C[i,j] path selection (VIZ selection acceptance). */

export type Selection =
  | { kind: 'none' }
  | { kind: 'output'; i: number; j: number }

export function clearSelection(): Selection {
  return { kind: 'none' }
}

export function selectOutput(i: number, j: number, m: number, n: number): Selection {
  if (i < 0 || j < 0 || i >= m || j >= n) return { kind: 'none' }
  return { kind: 'output', i, j }
}

export type PathHighlight = {
  aRow: number
  bCol: number
  cCell: { i: number; j: number }
}

export function pathFromSelection(sel: Selection): PathHighlight | null {
  if (sel.kind !== 'output') return null
  return { aRow: sel.i, bCol: sel.j, cCell: { i: sel.i, j: sel.j } }
}
