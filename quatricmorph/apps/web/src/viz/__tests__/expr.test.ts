import { describe, expect, it } from 'vitest'
import { defaultLeft, defaultRight, default_dims } from '../defaults.js'
import { genExpr } from '../expr.js'

describe('defaults', () => {
  it('keeps left/right contraction dimension aligned', () => {
    const left = defaultLeft()
    const right = defaultRight()
    expect(left.w).toBe(right.h)
    expect(left.name).toBe('A')
    expect(right.name).toBe('B')
    expect(default_dims.i).toBe(2)
  })
})

describe('genExpr', () => {
  it('renders simple L @ R when names match expansion', () => {
    const expr = genExpr({
      name: 'L @ R',
      left: { name: 'L', matmul: false },
      right: { name: 'R', matmul: false },
    })
    expect(expr).toBe('L @ R')
  })

  it('prefixes assignment when root name differs', () => {
    const expr = genExpr({
      name: 'out',
      left: { name: 'L', matmul: false },
      right: { name: 'R', matmul: false },
    })
    expect(expr).toBe('out = L @ R')
  })

  it('renders MVP C = A @ B', () => {
    const expr = genExpr({
      name: 'C',
      left: { name: 'A', matmul: false },
      right: { name: 'B', matmul: false },
    })
    expect(expr).toBe('C = A @ B')
  })
})
