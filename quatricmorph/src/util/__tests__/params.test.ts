import { describe, expect, it } from 'vitest'
import {
  makeSearchParams,
  updateObjectFromSearchParams,
} from '../params.js'
import { flatten, unflatten, compress, uncompress } from '../objects.js'

describe('VIZ-08 URL params round-trip', () => {
  it('JSON params round-trip', () => {
    const params: Record<string, unknown> = {
      name: 'C',
      left: { h: 2, w: 3, name: 'A' },
      right: { h: 3, w: 2, name: 'B' },
      compress: false,
    }
    const sp = makeSearchParams(params)
    const target: Record<string, unknown> = {
      name: '',
      left: { h: 1, w: 1, name: '' },
      right: { h: 1, w: 1, name: '' },
      compress: false,
    }
    updateObjectFromSearchParams(target, sp)
    expect(target.name).toBe('C')
    expect((target.left as { h: number }).h).toBe(2)
    expect((target.right as { w: number }).w).toBe(2)
  })

  it('compressed flatten round-trip', () => {
    const obj = { a: 1, nested: { b: true, c: 'x' } }
    const flat = flatten(obj)
    const compressed = compress(flat)
    const restored = unflatten(uncompress(compressed))
    expect(restored).toEqual(obj)
  })

  it('invalid JSON does not throw', () => {
    const target: Record<string, unknown> = { name: 'keep' }
    const sp = new URLSearchParams({ params: '{not-json' })
    updateObjectFromSearchParams(target, sp)
    expect(target.name).toBe('keep')
  })
})
