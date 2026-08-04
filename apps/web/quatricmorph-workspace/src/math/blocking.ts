/**
 * Block decomposition — **pure math, no Three.js**.
 *
 * Extracted from `viz/matmul.ts` (`MatMul.getBlockInfo`, `MatMul.grid`) and
 * `viz/sizing.ts` (`grid`), which mixed this index arithmetic with scene state.
 * Keeping it separate is what lets the same decomposition drive a Three.js
 * animation, a `.qtile` pyramid, and a `q-weightql` block query without three
 * copies of the loop.
 *
 * Corresponds to `q_tensor_runtime::BlockExtent` on the Rust side: both answer
 * "which sub-rectangles does this tensor decompose into", and both must agree
 * or a clicked cell will not address the tensor the viewer thinks it does.
 */

/** One axis of a block decomposition. */
export type AxisBlocking = {
  /** Number of blocks along this axis. */
  n: number
  /** Elements per block (the last block may be shorter). */
  size: number
  /** Axis length. */
  max: number
}

export type BlockInfo = {
  i: AxisBlocking
  k: AxisBlocking
  j: AxisBlocking
}

/** One block's span along one axis, as yielded by {@link gridIterate}. */
export type BlockSpan = {
  index: number
  start: number
  end: number
  extent: number
}

/**
 * Decompose an m×k×n multiplication into blocks.
 *
 * A request for more blocks than there are elements is clamped: asking for 16
 * blocks of a 4-row tensor gives 4 blocks of 1 row, not 16 blocks of which 12
 * are empty.
 */
export function blockInfo(
  m: number,
  k: number,
  n: number,
  request: { i: number; k: number; j: number },
): BlockInfo {
  const axis = (blocks: number, max: number): AxisBlocking => {
    const nb = Math.max(1, Math.min(Math.floor(blocks), max))
    return { n: nb, size: Math.ceil(max / nb), max }
  }
  return { i: axis(request.i, m), k: axis(request.k, k), j: axis(request.j, n) }
}

/**
 * Iterate the blocks of the named axes in row-major order.
 *
 * `dims` selects and orders the axes, e.g. `'ikj'`, `'i'`, `'ij'`. The callback
 * receives one {@link BlockSpan} per named axis.
 *
 * Trailing blocks that would start past the axis length are skipped — that is
 * the `start < max` guard from the original `grid()`, and it matters when
 * `size * n > max` (e.g. 3 blocks of a 4-element axis: sizes 2, 2, and a dead
 * third).
 */
export function gridIterate(
  info: BlockInfo,
  dims: string,
  f: (...spans: BlockSpan[]) => void,
): void {
  const axes = Array.from(dims).map((d) => info[d as keyof BlockInfo])
  const loop = (args: BlockSpan[], rest: AxisBlocking[]): void => {
    if (rest.length === 0) {
      f(...args)
      return
    }
    const [head, ...tail] = rest
    for (let index = 0; index < head.n; index++) {
      const start = index * head.size
      if (start >= head.max) continue
      const end = Math.min(start + head.size, head.max)
      loop([...args, { index, start, end, extent: end - start }], tail)
    }
  }
  loop([], axes)
}

/** Collect the spans {@link gridIterate} would visit. */
export function gridSpans(info: BlockInfo, dims: string): BlockSpan[][] {
  const out: BlockSpan[][] = []
  gridIterate(info, dims, (...spans) => out.push(spans))
  return out
}

/**
 * Scatter multiplier for an operand, from the legacy `scatterFromCount`.
 *
 * Pure: a function of counts and layout parameters only.
 */
export function scatterFromCount(
  count: number,
  total: number,
  layout: { scatter: number; molecule: number; blast: number },
): number {
  const { scatter, molecule, blast } = layout
  const mult =
    count < molecule ? 0 : blast >= 0 ? count ** blast : (total - count) ** -blast
  return scatter * mult
}
