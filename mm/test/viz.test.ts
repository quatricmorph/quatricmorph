//
// viz.js — the numerics under the picture.
//
// The rendering half of this module needs a GPU and is not tested here. What is
// tested is everything that decides *what number each element holds*: the
// initializers, the in-place epilogs (softmax, tril softmax, layernorm) and
// Array2D. Those are pure, and they are where a wrong answer is invisible —
// a subtly wrong softmax still draws a smooth colour ramp.
//
// Expected values are hand-computed (worked out in the comments) rather than
// captured from this code, so the assertions are independent of it.
//
import { describe, it, expect } from 'vitest'
import {
  INIT_FUNCS, INITS, useRange, useDropout, EPILOGS, Array2D, setElemSize,
} from '../src/viz.js'

const close = (a, b, p = 6) => expect(a).toBeCloseTo(b, p)

describe('INIT_FUNCS', () => {
  it('ramps rows from 0 to 1 down the rows', () => {
    const f = INIT_FUNCS['rows']
    expect(f(0, 0, 3)).toBe(0)
    close(f(1, 0, 3), 0.5)
    expect(f(2, 0, 3)).toBe(1)
  })

  it('ramps cols from 0 to 1 across the columns', () => {
    const f = INIT_FUNCS['cols']
    expect(f(0, 0, 3, 4)).toBe(0)
    expect(f(0, 3, 3, 4)).toBe(1)
  })

  it('avoids dividing by zero in a degenerate single row or column', () => {
    expect(INIT_FUNCS['rows'](0, 0, 1)).toBe(0)
    expect(INIT_FUNCS['cols'](0, 0, 1, 1)).toBe(0)
    expect(INIT_FUNCS['row major'](0, 0, 1, 1)).toBe(0)
    expect(INIT_FUNCS['col major'](0, 0, 1, 1)).toBe(0)
  })

  it('numbers row major across then down, col major down then across', () => {
    // 2x3, last index 5. row major (1,2) = (1*3+2)/5 = 1
    //                    col major (1,2) = (2*2+1)/5 = 1
    close(INIT_FUNCS['row major'](1, 2, 2, 3), 1)
    close(INIT_FUNCS['col major'](1, 2, 2, 3), 1)
    close(INIT_FUNCS['row major'](0, 1, 2, 3), 0.2)   // 1/5
    close(INIT_FUNCS['col major'](0, 1, 2, 3), 0.4)   // 2/5
  })

  it('builds the mask and structured initializers exactly', () => {
    expect(INIT_FUNCS['tril mask'](2, 1)).toBe(1)     // j <= i
    expect(INIT_FUNCS['tril mask'](1, 2)).toBe(0)
    expect(INIT_FUNCS['triu mask'](1, 2)).toBe(1)     // j >= i
    expect(INIT_FUNCS['eye'](2, 2)).toBe(1)
    expect(INIT_FUNCS['eye'](1, 2)).toBe(0)
    expect(INIT_FUNCS['diff'](2, 2)).toBe(1)          // i == j
    expect(INIT_FUNCS['diff'](2, 1)).toBe(-1)         // i == j+1
    expect(INIT_FUNCS['diff'](1, 2)).toBe(0)
  })

  it('keeps the random initializers in range', () => {
    for (let k = 0; k < 50; k++) {
      const x = INIT_FUNCS['uniform']()
      expect(x).toBeGreaterThanOrEqual(0)
      expect(x).toBeLessThan(1)
    }
  })
})

describe('the initializer menu', () => {
  it('offers every init func plus the two data-backed ones', () => {
    // 'url' is how the checkpoint pages get real weights in; losing it from
    // this list would break every example page.
    expect(INITS).toEqual([...Object.keys(INIT_FUNCS), 'url', 'expr'])
    expect(INITS).toContain('url')
    expect(INITS).toContain('expr')
  })

  it('applies min/max only to the initializers that span a range', () => {
    expect(useRange('uniform')).toBe(true)
    expect(useRange('gaussian')).toBe(true)
    expect(useRange('eye')).toBe(false)          // a mask has no range to scale
    expect(useRange('tril mask')).toBe(false)
    expect(useRange('pt linear')).toBe(false)
  })

  it('offers dropout on the range initializers and on pt linear', () => {
    expect(useDropout('pt linear')).toBe(true)   // the one that differs
    expect(useDropout('uniform')).toBe(true)
    expect(useDropout('eye')).toBe(false)
  })
})

describe('the epilog menu', () => {
  it('starts at none and carries the attention-shaped softmaxes', () => {
    expect(EPILOGS[0]).toBe('none')
    for (const e of ['softmax', 'softmax(x/sqrt(k))', 'softmax(tril(x/sqrt(k)))',
      'layernorm', 'relu', 'gelu', 'x/sqrt(k)']) {
      expect(EPILOGS).toContain(e)
    }
  })
})

describe('Array2D', () => {
  const seq = (h, w) => Array2D.fromInit(h, w, (i, j) => i * w + j)

  it('lays elements out row major', () => {
    const a = seq(2, 3)
    expect(Array.from(a.data)).toEqual([0, 1, 2, 3, 4, 5])
    expect(a.addr(1, 2)).toBe(5)
    expect(a.get(1, 2)).toBe(5)
    expect(a.numel()).toBe(6)
  })

  it('truncates h and w to integers', () => {
    const a = new Array2D(2.9, 3.9, new Float32Array(12))
    expect(a.h).toBe(2)
    expect(a.w).toBe(3)
  })

  it('transposes', () => {
    const t = seq(2, 3).transpose()
    expect(t.h).toBe(3)
    expect(t.w).toBe(2)
    expect(t.get(2, 1)).toBe(5)
    expect(Array.from(t.data)).toEqual([0, 3, 1, 4, 2, 5])
  })

  it('slices a rectangular window', () => {
    // rows [1,2) of cols [1,3) of the 2x3 sequence -> [4, 5]
    const s = seq(2, 3).slice([1, 2], [1, 3])
    expect(s.h).toBe(1)
    expect(s.w).toBe(2)
    expect(Array.from(s.data)).toEqual([4, 5])
  })

  it('treats a bare index as a one-wide slice', () => {
    // toRange(x, n): undefined -> whole axis, a number -> [x, x+1]
    const row = seq(2, 3).slice(1)
    expect(row.h).toBe(1)
    expect(row.w).toBe(3)
    expect(Array.from(row.data)).toEqual([3, 4, 5])
  })

  it('finds the largest and smallest magnitudes, ignoring sign', () => {
    const a = new Array2D(1, 4, new Float32Array([-7, 2, -1, 5]))
    expect(a.absmax()).toBe(7)
    expect(a.absmin()).toBe(1)
  })

  it('maps elementwise', () => {
    const m = seq(2, 3).map(x => x * 2)
    expect(Array.from(m.data)).toEqual([0, 2, 4, 6, 8, 10])
    expect(m.h).toBe(2)
    expect(m.w).toBe(3)
  })

  it('adds elementwise', () => {
    const s = seq(2, 3).add(seq(2, 3))
    expect(Array.from(s.data)).toEqual([0, 2, 4, 6, 8, 10])
  })

  it('refuses to combine mismatched shapes', () => {
    expect(() => seq(2, 3).add(seq(3, 2))).toThrow(/shape error/)
  })

  it('reinits only the requested window', () => {
    const a = seq(2, 3)
    a.reinit(() => -1, undefined, 1)          // row 1 only
    expect(Array.from(a.data)).toEqual([0, 1, 2, -1, -1, -1])
  })
})

describe('softmax epilog', () => {
  it('normalises a row to sum 1', () => {
    // [1,2,3]: shifted by max 3 -> e^-2, e^-1, e^0 = .1353353, .3678794, 1
    // denom 1.5032147 -> .0900306, .2447285, .6652409
    const a = Array2D.fromInit(1, 3, (i, j) => j + 1, 'softmax')
    close(a.get(0, 0), 0.0900306)
    close(a.get(0, 1), 0.2447285)
    close(a.get(0, 2), 0.6652409)
    close(Array.from<number>(a.data).reduce((s, x) => s + x, 0), 1)
  })

  it('normalises each row independently', () => {
    // row 0 = [0,0] -> [.5,.5]; row 1 = [0,1] -> [.2689414,.7310586]
    const a = Array2D.fromInit(2, 2, (i, j) => i * j, 'softmax')
    close(a.get(0, 0), 0.5)
    close(a.get(0, 1), 0.5)
    close(a.get(1, 0), 0.2689414)
    close(a.get(1, 1), 0.7310586)
  })

  it('yields zeros instead of NaN when a row underflows', () => {
    // The row max is clamped at 0, so a row of large negatives shifts by 0,
    // every exp() underflows to 0 and the denominator is 0. The isNaN guard
    // turns 0/0 into 0 — the row does not sum to 1, but it is finite and the
    // scene still renders. Pinning this stops a "cleanup" from reintroducing
    // NaN, which propagates into the colour mapping and blanks the matrix.
    const a = Array2D.fromInit(1, 2, () => -1000, 'softmax')
    expect(Array.from(a.data)).toEqual([0, 0])
    expect(Array.from(a.data).every(Number.isFinite)).toBe(true)
  })
})

describe('tril softmax epilog', () => {
  it('zeroes the strict upper triangle and normalises what is left', () => {
    // 2x2 [[1,2],[3,4]]
    // row 0 sees j<=0: [1] -> [1, 0]
    // row 1 sees j<=1: [3,4] shifted by 4 -> e^-1, e^0 over 1.3678794
    //                  -> .2689414, .7310586
    const a = Array2D.fromInit(2, 2, (i, j) => i * 2 + j + 1, 'softmax(tril(x/sqrt(k)))')
    close(a.get(0, 0), 1)
    expect(a.get(0, 1)).toBe(0)          // masked, exactly zero
    close(a.get(1, 0), 0.2689414)
    close(a.get(1, 1), 0.7310586)
  })

  it('leaves every causal row summing to 1', () => {
    const n = 5
    const a = Array2D.fromInit(n, n, (i, j) => (i + j) % 3, 'softmax(tril(x/sqrt(k)))')
    for (let i = 0; i < n; i++) {
      let s = 0
      for (let j = 0; j < n; j++) s += a.get(i, j)
      close(s, 1, 5)
    }
  })

  it('masks strictly above the diagonal, keeping the diagonal itself', () => {
    const n = 4
    const a = Array2D.fromInit(n, n, () => 1, 'softmax(tril(x/sqrt(k)))')
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        if (j > i) expect(a.get(i, j)).toBe(0)
        else close(a.get(i, j), 1 / (i + 1))     // uniform over i+1 visible cells
      }
    }
  })
})

describe('layernorm epilog', () => {
  it('normalises over the WHOLE matrix, not per row', () => {
    // This is surprising and deliberate-looking, so it is pinned: mean and
    // variance are taken over every element. A "fix" to per-row statistics
    // would change every layernorm picture in the app.
    //
    // [1,2,3,4]: mean 2.5, mean2 7.5, var 1.25, denom sqrt(1.25 + 1e-5)
    //         -> -1.3416367, -.4472122, .4472122, 1.3416367
    const a = Array2D.fromInit(2, 2, (i, j) => i * 2 + j + 1, 'layernorm')
    close(a.get(0, 0), -1.3416367, 5)
    close(a.get(0, 1), -0.4472122, 5)
    close(a.get(1, 0), 0.4472122, 5)
    close(a.get(1, 1), 1.3416367, 5)

    // per-row would have given [-1, 1] twice
    expect(a.get(0, 0)).not.toBeCloseTo(-1, 2)
  })

  it('leaves the whole matrix zero-mean and unit-variance', () => {
    const a = Array2D.fromInit(4, 4, (i, j) => (i * 7 + j * 3) % 11, 'layernorm')
    const xs = Array.from<number>(a.data)
    const mean = xs.reduce((s, x) => s + x, 0) / xs.length
    const varr = xs.reduce((s, x) => s + (x - mean) ** 2, 0) / xs.length
    close(mean, 0, 5)
    close(varr, 1, 4)
  })

  it('does not divide by zero on a constant matrix', () => {
    // variance 0, denom sqrt(1e-5) — the epsilon is what keeps this finite
    const a = Array2D.fromInit(2, 2, () => 3, 'layernorm')
    expect(Array.from(a.data).every(Number.isFinite)).toBe(true)
    expect(Array.from(a.data)).toEqual([0, 0, 0, 0])
  })
})

describe('epilog dispatch', () => {
  it('leaves data untouched for none and for the pointwise epilogs', () => {
    // Only softmax/layernorm are applied in place at init; relu and friends are
    // applied later, in Mat. Asserting that keeps the two paths from being
    // "unified" without noticing they run at different times.
    const plain = Array2D.fromInit(1, 3, (i, j) => j - 1)
    const none = Array2D.fromInit(1, 3, (i, j) => j - 1, 'none')
    const relu = Array2D.fromInit(1, 3, (i, j) => j - 1, 'relu')
    expect(Array.from(none.data)).toEqual(Array.from(plain.data))
    expect(Array.from(relu.data)).toEqual([-1, 0, 1])
  })
})

describe('setElemSize', () => {
  it('accepts a scale and pixel ratio without a renderer', () => {
    expect(() => setElemSize({ x: 1, y: 1 }, 2)).not.toThrow()
  })
})
