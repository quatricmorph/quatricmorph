/**
 * Animation state for output-cell dot-product walk (separate from matrix data).
 * Algorithm: for each C[i,j], walk k = 0..K-1 showing A[i,k]×B[k,j] then reveal C.
 */

export type AnimStatus = 'idle' | 'playing' | 'paused' | 'done'

export type AnimState = {
  status: AnimStatus
  /** Flat output-cell index in row-major order. */
  cellIndex: number
  /** Contraction index within current cell. */
  kIndex: number
  m: number
  n: number
  k: number
  runningSum: number
}

export function createAnimState(m: number, k: number, n: number): AnimState {
  return {
    status: 'idle',
    cellIndex: 0,
    kIndex: 0,
    m,
    n,
    k,
    runningSum: 0,
  }
}

export function cellCoords(state: AnimState): { i: number; j: number } {
  return {
    i: Math.floor(state.cellIndex / state.n),
    j: state.cellIndex % state.n,
  }
}

export function totalCells(state: AnimState): number {
  return state.m * state.n
}

export function resetAnim(state: AnimState): AnimState {
  return { ...state, status: 'idle', cellIndex: 0, kIndex: 0, runningSum: 0 }
}

/** Advance one micro-step (next k, or next cell). Deterministic. */
export function stepForward(
  state: AnimState,
  productAt: (i: number, j: number, k: number) => number,
): AnimState {
  if (state.status === 'done') return state
  const { i, j } = cellCoords(state)
  const term = productAt(i, j, state.kIndex)
  const runningSum = state.kIndex === 0 ? term : state.runningSum + term
  const nextK = state.kIndex + 1

  if (nextK < state.k) {
    return { ...state, status: state.status === 'idle' ? 'playing' : state.status, kIndex: nextK, runningSum }
  }

  const nextCell = state.cellIndex + 1
  if (nextCell >= totalCells(state)) {
    return { ...state, status: 'done', kIndex: state.k - 1, runningSum, cellIndex: state.cellIndex }
  }
  return {
    ...state,
    status: state.status === 'idle' ? 'playing' : state.status,
    cellIndex: nextCell,
    kIndex: 0,
    runningSum: 0,
  }
}

export function stepBackward(state: AnimState): AnimState {
  if (state.cellIndex === 0 && state.kIndex === 0) {
    return { ...state, status: 'paused', runningSum: 0 }
  }
  if (state.kIndex > 0) {
    return { ...state, status: 'paused', kIndex: state.kIndex - 1 }
  }
  return {
    ...state,
    status: 'paused',
    cellIndex: state.cellIndex - 1,
    kIndex: Math.max(0, state.k - 1),
    runningSum: 0,
  }
}
