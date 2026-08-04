import { Array2D } from '../viz/array2d.js'
import { validateMatmulDims } from './validate.js'

/** Pure matrix multiply C = A @ B with C[i,j] = Σ_k A[i,k] * B[k,j] (VIZ-02). */
export function matmul(
  a: { h: number; w: number; get: (i: number, j: number) => number },
  b: { h: number; w: number; get: (i: number, j: number) => number },
) {
  const check = validateMatmulDims(a.h, a.w, b.h, b.w)
  if (check.ok === false) {
    throw new Error(check.message)
  }
  return Array2D.fromInit(a.h, b.w, (i, j) => {
    let s = 0
    for (let k = 0; k < a.w; k++) {
      s += a.get(i, k) * b.get(k, j)
    }
    return s
  })
}

/** Dot product for a single output cell. */
export function dotprodCell(
  a: { h: number; w: number; get: (i: number, j: number) => number },
  b: { h: number; w: number; get: (i: number, j: number) => number },
  i: number,
  j: number,
): number {
  if (a.w !== b.h) {
    throw new Error(`dims: A.cols ${a.w} !== B.rows ${b.h}`)
  }
  let s = 0
  for (let k = 0; k < a.w; k++) {
    s += a.get(i, k) * b.get(k, j)
  }
  return s
}
