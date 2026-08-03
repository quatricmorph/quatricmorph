/** Dimension validation for A @ B (VIZ-01). */

export type DimError = {
  ok: false
  message: string
  aCols: number
  bRows: number
}

export type DimOk = {
  ok: true
  m: number
  k: number
  n: number
}

export type DimResult = DimOk | DimError

export function validateMatmulDims(
  aRows: number,
  aCols: number,
  bRows: number,
  bCols: number,
): DimResult {
  if (!(aRows > 0 && aCols > 0 && bRows > 0 && bCols > 0)) {
    return {
      ok: false,
      message: 'All matrix dimensions must be positive integers.',
      aCols,
      bRows,
    }
  }
  if (aCols !== bRows) {
    return {
      ok: false,
      message:
        `Incompatible dimensions for A @ B: A is ${aRows}×${aCols}, B is ${bRows}×${bCols}. ` +
        `A.columns (${aCols}) must equal B.rows (${bRows}).`,
      aCols,
      bRows,
    }
  }
  return { ok: true, m: aRows, k: aCols, n: bCols }
}
