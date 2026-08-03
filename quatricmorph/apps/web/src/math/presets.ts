/** Deterministic and random presets for A/B values. */

export type PresetName =
  | 'random'
  | 'identity'
  | 'sequential'
  | 'zeros'
  | 'ones'
  | 'small'

export function fillPreset(
  rows: number,
  cols: number,
  preset: PresetName,
  rng: () => number = Math.random,
): number[][] {
  const data: number[][] = []
  for (let i = 0; i < rows; i++) {
    const row: number[] = []
    for (let j = 0; j < cols; j++) {
      switch (preset) {
        case 'zeros':
          row.push(0)
          break
        case 'ones':
          row.push(1)
          break
        case 'identity':
          row.push(i === j ? 1 : 0)
          break
        case 'sequential':
          row.push(i * cols + j + 1)
          break
        case 'small':
          // Default MVP example pattern for 2×3 / 3×2 when shapes match.
          row.push(i * cols + j + 1)
          break
        case 'random':
        default:
          row.push(Math.round((rng() * 2 - 1) * 100) / 100)
          break
      }
    }
    data.push(row)
  }
  return data
}

/** Canonical default example: A 2×3, B 3×2 → C [[58,64],[139,154]]. */
export const DEFAULT_A: number[][] = [
  [1, 2, 3],
  [4, 5, 6],
]
export const DEFAULT_B: number[][] = [
  [7, 8],
  [9, 10],
  [11, 12],
]
export const DEFAULT_C: number[][] = [
  [58, 64],
  [139, 154],
]
