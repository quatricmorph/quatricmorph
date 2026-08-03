/**
 * Tensor-block adapter — **typed stub** (`GRID-004`).
 *
 * The matrix workspace currently renders hand-entered matrices. This is the
 * seam through which it will instead render a real block of a real checkpoint,
 * fetched from `q-daemon`.
 *
 * Nothing here fetches anything. `HandEnteredSource` is the implementation that
 * ships today; `DaemonBlockSource` exists so the shape of the eventual wiring
 * is fixed, and it **throws** rather than returning plausible zeros — a grid of
 * zeros would render perfectly and be indistinguishable from real weights.
 *
 * Data plane: consumes the **Tensor Tile Plane** (block statistics, sampled
 * values) and the **Artifact Plane** (exact values on selection). It must never
 * request a whole tensor: ARCHITECTURE.md §19, "do not send the entire tensor
 * into the browser".
 */

/** A canonical address, e.g. `model.layers[10].self_attention.query_projection.weight`. */
export type CanonicalAddress = string

/** Half-open 2-D window of a tensor. */
export type BlockRequest = {
  modelId: string
  address: CanonicalAddress
  rowStart: number
  rowEnd: number
  columnStart: number
  columnEnd: number
}

/**
 * How the returned values were obtained.
 *
 * Mirrors `q_source::ResultFidelity`. The workspace must display this: a
 * sampled tile and an exact block look identical once rendered, and
 * ARCHITECTURE.md §18 AC-010 requires the difference to be visible.
 */
export type Fidelity = 'exact' | 'sampled' | 'approximate'

export type TensorBlockData = {
  address: CanonicalAddress
  rows: number
  columns: number
  /** Row-major, `rows * columns` entries. */
  values: Float32Array
  fidelity: Fidelity
  /** Bytes actually read to produce this block, for the I/O readout. */
  bytesRead: number
}

/** Supplies block data to the workspace. */
export interface TensorBlockSource {
  readonly id: string
  /** Largest block this source will return, in elements. */
  readonly maxBlockElements: number
  fetchBlock(request: BlockRequest): Promise<TensorBlockData>
}

/** Refuse a request that would pull an unreasonable amount into the browser. */
export function assertBlockIsBounded(
  request: BlockRequest,
  maxElements: number,
): void {
  const rows = request.rowEnd - request.rowStart
  const columns = request.columnEnd - request.columnStart
  if (rows <= 0 || columns <= 0) {
    throw new Error(
      `empty block [${request.rowStart}:${request.rowEnd}, ` +
        `${request.columnStart}:${request.columnEnd}] for ${request.address}`,
    )
  }
  if (rows * columns > maxElements) {
    throw new Error(
      `block of ${rows * columns} elements exceeds the ${maxElements}-element ceiling ` +
        `for ${request.address}. Select a smaller region — sending a whole tensor to ` +
        `the browser is exactly what ARCHITECTURE.md §19 forbids.`,
    )
  }
}

/** The source that ships today: values typed into the GUI. */
export class HandEnteredSource implements TensorBlockSource {
  readonly id = 'hand-entered'
  readonly maxBlockElements = 1 << 16

  constructor(private readonly values: Map<CanonicalAddress, TensorBlockData>) {}

  async fetchBlock(request: BlockRequest): Promise<TensorBlockData> {
    assertBlockIsBounded(request, this.maxBlockElements)
    const found = this.values.get(request.address)
    if (!found) {
      throw new Error(`no hand-entered matrix named ${request.address}`)
    }
    return found
  }
}

/**
 * The eventual source: `GET /v1/tensors/{id}/blocks` on `q-daemon`.
 *
 * **Not implemented** (`GRID-004`). The daemon route exists and works; what is
 * missing is the workspace-side wiring — address resolution, block cache, and
 * the LOD policy that decides when to request exact values instead of a
 * summary tile.
 */
export class DaemonBlockSource implements TensorBlockSource {
  readonly id = 'q-daemon'
  readonly maxBlockElements = 1 << 18

  constructor(readonly baseUrl: string) {}

  async fetchBlock(request: BlockRequest): Promise<TensorBlockData> {
    assertBlockIsBounded(request, this.maxBlockElements)
    throw new Error(
      `[GRID-004] the matrix workspace is not wired to q-daemon yet, so ` +
        `${request.address} was not fetched from ${this.baseUrl}. Returning zeros ` +
        `would render as a perfectly plausible tensor, which is why nothing is ` +
        `returned. See ARCHITECTURE.md §14.2.`,
    )
  }
}
