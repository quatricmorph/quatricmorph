/**
 * Parse matrix text without eval.
 * Accepts: "1, 2, 3\n4, 5, 6" or space/semicolon separators.
 */
export type ParseOk = { ok: true; rows: number; cols: number; data: number[][] }
export type ParseErr = { ok: false; message: string }
export type ParseResult = ParseOk | ParseErr

export function parseMatrixText(text: string): ParseResult {
  const raw = text.trim()
  if (!raw) {
    return { ok: false, message: 'Matrix text is empty.' }
  }

  const lines = raw
    .split(/\r?\n|;/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0)

  if (lines.length === 0) {
    return { ok: false, message: 'Matrix text has no rows.' }
  }

  const data: number[][] = []
  let cols = -1

  for (let r = 0; r < lines.length; r++) {
    const parts = lines[r]
      .split(/[,\s]+/)
      .map((p) => p.trim())
      .filter((p) => p.length > 0)

    if (parts.length === 0) {
      return { ok: false, message: `Row ${r} is empty.` }
    }

    const nums: number[] = []
    for (const p of parts) {
      const n = Number(p)
      if (!Number.isFinite(n)) {
        return { ok: false, message: `Invalid number "${p}" in row ${r}.` }
      }
      nums.push(n)
    }

    if (cols < 0) cols = nums.length
    else if (nums.length !== cols) {
      return {
        ok: false,
        message: `Row ${r} has ${nums.length} values, expected ${cols}.`,
      }
    }
    data.push(nums)
  }

  return { ok: true, rows: data.length, cols, data }
}

export function matrixToText(data: number[][]): string {
  return data.map((row) => row.join(', ')).join('\n')
}

export function flatFromRows(data: number[][]): Float32Array {
  const h = data.length
  const w = data[0]?.length ?? 0
  const out = new Float32Array(h * w)
  for (let i = 0; i < h; i++) {
    for (let j = 0; j < w; j++) {
      out[i * w + j] = data[i][j]
    }
  }
  return out
}
