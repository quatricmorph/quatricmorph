/** Shape-inferred tensor kinds for MVP (same cell/frame systems for all). */

export type TensorKind = 'matrix' | 'column' | 'row' | 'scalar'

export function inferTensorKind(rows: number, cols: number): TensorKind {
  if (rows === 1 && cols === 1) return 'scalar'
  if (cols === 1) return 'column'
  if (rows === 1) return 'row'
  return 'matrix'
}

export function shapeLabel(name: string, rows: number, cols: number): string {
  return `${name} [${rows} × ${cols}]`
}
