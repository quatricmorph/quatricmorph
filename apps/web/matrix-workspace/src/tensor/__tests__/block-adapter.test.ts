import { describe, expect, it } from 'vitest'
import {
  assertBlockIsBounded,
  DaemonBlockSource,
  HandEnteredSource,
  type BlockRequest,
  type TensorBlockData,
} from '../block-adapter.js'

const request = (rows: number, cols: number): BlockRequest => ({
  modelId: 'm',
  address: 'model.layers[10].self_attention.query_projection.weight',
  rowStart: 0,
  rowEnd: rows,
  columnStart: 0,
  columnEnd: cols,
})

describe('GRID-004 tensor block adapter', () => {
  it('accepts a reasonable block', () => {
    expect(() => assertBlockIsBounded(request(256, 256), 1 << 18)).not.toThrow()
  })

  it('refuses a block that would pull a whole tensor into the browser', () => {
    expect(() => assertBlockIsBounded(request(4096, 4096), 1 << 18)).toThrow(
      /ARCHITECTURE.md §19/,
    )
  })

  it('refuses an empty or inverted block', () => {
    expect(() => assertBlockIsBounded(request(0, 4), 1 << 18)).toThrow(/empty block/)
    expect(() =>
      assertBlockIsBounded({ ...request(4, 4), rowEnd: 1, rowStart: 3 }, 1 << 18),
    ).toThrow(/empty block/)
  })

  it('serves hand-entered matrices and labels them', async () => {
    const data: TensorBlockData = {
      address: request(2, 2).address,
      rows: 2,
      columns: 2,
      values: Float32Array.from([1, 2, 3, 4]),
      fidelity: 'exact',
      bytesRead: 0,
    }
    const source = new HandEnteredSource(new Map([[data.address, data]]))
    await expect(source.fetchBlock(request(2, 2))).resolves.toBe(data)
    await expect(
      source.fetchBlock({ ...request(2, 2), address: 'nope' }),
    ).rejects.toThrow(/no hand-entered matrix/)
  })

  it('the daemon source refuses rather than returning plausible zeros', async () => {
    const source = new DaemonBlockSource('http://127.0.0.1:8080')
    await expect(source.fetchBlock(request(4, 4))).rejects.toThrow(/GRID-004/)
    // The ceiling is still enforced before the not-implemented error, so a
    // caller cannot learn "it would have worked for a huge block".
    await expect(source.fetchBlock(request(4096, 4096))).rejects.toThrow(
      /exceeds the .* ceiling/,
    )
  })
})
