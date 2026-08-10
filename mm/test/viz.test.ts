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
import * as THREE from 'three'
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

//
// Node kinds beyond the matmul: the numerics, and the expression guard.
//
// These build real viz objects with `init_viz` off, which is the whole scene
// graph minus the geometry — so the arithmetic is reachable with no GPU, no
// camera and no network.
//
import {
  UnaryOp, AddOp, Stack, buildOpNode, genExpr, syncExpr, treeHasOps, describeTree,
  nodeHeight, nodeWidth, mergeVizSummaries, UNARY_FUNCS,
} from '../src/viz.js'

const ctx = () => ({ raycaster: null, camera: null, pointer: null })

// The layout tests below are the one place here that needs real geometry: stage
// positions only exist once initViz has run. No GPU is involved — the same
// context test/cameractl.test.ts builds a MatMul with.
const vizCtx = () => ({
  raycaster: new THREE.Raycaster(), camera: new THREE.PerspectiveCamera(45, 1, 0.1, 1000),
  pointer: new THREE.Vector2(),
})

// A leaf whose values are i*w + j, so every assertion below is hand-checkable.
const lf = (name, h, w, init = 'row major') => ({
  name, matmul: false, h, w, init, url: '', expr: '', min: 0, max: 1, dropout: 0,
})
// 'expr' initializers are evaluated, so this gives exact known values.
const ex = (name, h, w, e) => ({ ...lf(name, h, w, 'expr'), expr: e })

const OPTS = () => ({
  epilog: 'none',
  anim: { alg: 'none', speed: 16, fuse: 'none', 'hide inputs': false, spin: 0 },
  block: { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 },
  layout: {
    scheme: 'blocks', gap: 2, scatter: 0, molecule: 1, blast: 0,
    polarity: 'negative', 'left placement': 'left',
    'right placement': 'top', 'result placement': 'front',
  },
  deco: { legends: 0, shape: false, spotlight: 0, 'row guides': 0, 'flow guides': 0, grid: 0 },
  viz: {
    sensitivity: 'local', 'min size': 0.05, 'min light': 0.2, 'max light': 0.9,
    'elem scale': 2, 'zero hue': 0.75, 'hue gap': 0.75, 'hue spread': 0.03,
    'render mode': 'spheres', 'heatmap encoding': 'magnitude',
    'heatmap filter': 'nearest', 'lod reduce': 'maxAbs', 'texel budget': 0,
  },
})

describe('UnaryOp', () => {
  it('materializes f(input) as a second matrix and leaves the input alone', () => {
    // The difference from an in-place epilog, which is the point of the kind:
    // `applyInPlaceEpilog_` mutates the parent's result buffer, so the
    // pre-epilog matrix stops existing. Here both are on screen.
    const p = { ...OPTS(), name: 'g', op: 'unary', fn: 'relu', input: ex('x', 2, 2, 'i*2+j-2') }
    const u = new UnaryOp(p, ctx(), false)
    // x = [[-2, -1], [0, 1]]  ->  relu = [[0, 0], [0, 1]]
    expect(Array.from(u.input.getDataArray())).toEqual([-2, -1, 0, 1])
    expect(Array.from(u.getDataArray())).toEqual([0, 0, 0, 1])
  })

  it('applies a row-wise stage, not just a pointwise one', () => {
    // softmax(tril(x)) over [[0,0],[0,0]]: row 0 is [1, 0] (masked), row 1 is
    // [0.5, 0.5]. Hand-computed, same as the in-place epilog's own test above.
    const p = { ...OPTS(), name: 's', op: 'unary', fn: 'softmax(tril(x))', input: ex('x', 2, 2, '0') }
    const u = new UnaryOp(p, ctx(), false)
    const d = Array.from(u.getDataArray()) as number[]
    close(d[0], 1); expect(d[1]).toBe(0); close(d[2], 0.5); close(d[3], 0.5)
  })

  it('refuses an unknown stage function rather than drawing zeros', () => {
    const p = { ...OPTS(), name: 'g', op: 'unary', fn: 'nope', input: lf('x', 2, 2) }
    expect(() => new UnaryOp(p, ctx(), false)).toThrow(/unknown unary stage function 'nope'/)
    expect(UNARY_FUNCS).toContain('gelu')
    expect(UNARY_FUNCS).toContain('softmax(tril(x))')
  })

  it('takes its shape from its input', () => {
    const p = { name: 'g', op: 'unary', fn: 'relu', input: lf('x', 3, 5) }
    expect([nodeHeight(p), nodeWidth(p)]).toEqual([3, 5])
  })
})

describe('AddOp', () => {
  it('sums elementwise — and is never a matmul', () => {
    const p = {
      ...OPTS(), name: 'x + y', op: 'add',
      left: ex('x', 2, 3, 'i*3+j'), right: ex('y', 2, 3, '10'),
    }
    const a = new AddOp(p, ctx(), false)
    expect(Array.from(a.getDataArray())).toEqual([10, 11, 12, 13, 14, 15])
    // three drawn matrices, not two operands folded into a product
    expect(a.childNodes()).toHaveLength(3)
    expect(a.H).toBe(2)
    expect(a.W).toBe(3)
  })

  it('refuses mismatched operands instead of tiling the shorter one', () => {
    const p = { ...OPTS(), name: 'a', op: 'add', left: lf('x', 2, 3), right: lf('y', 2, 4) }
    expect(() => new AddOp(p, ctx(), false)).toThrow(/elementwise sum needs one shape/)
  })
})

describe('Stack', () => {
  const stackParams = () => ({
    ...OPTS(), name: 'model', op: 'stack',
    stages: {
      s0: { ...OPTS(), name: 'p', matmul: true, left: lf('a', 2, 3), right: lf('b', 3, 2) },
      s1: { ...OPTS(), name: 'g', op: 'unary', fn: 'relu', input: ex('x', 2, 2, 'i*2+j-2') },
      s2: { ...OPTS(), name: 's', op: 'add', left: ex('u', 2, 2, '1'), right: ex('v', 2, 2, '2') },
    },
  })

  it('keeps its stages in forward-pass order and names their kinds', () => {
    const st = new Stack(stackParams(), ctx(), false)
    expect(st.stageList().map(s => `${s.name}:${s.kind}`)).toEqual(['p:matmul', 'g:unary', 's:add'])
  })

  it('computes nothing of its own — its data is the last stage\'s', () => {
    const st = new Stack(stackParams(), ctx(), false)
    expect(Array.from(st.getDataArray())).toEqual([3, 3, 3, 3])
  })

  it('divides the scene texel budget among its stages', () => {
    // C3's rule, made a number: adding a stage lowers every other stage's
    // resolution rather than growing the upload.
    const st = new Stack(stackParams(), ctx(), false)
    const b3 = st.stageBudget()
    const more = stackParams()
    for (let i = 3; i < 30; i++) more.stages[`s${i}`] = { ...OPTS(), ...lf(`m${i}`, 2, 2), matmul: false }
    expect(new Stack(more, ctx(), false).stageBudget()).toBeLessThan(b3)
  })

  it('gives every stage a heatmap under auto, and the active one the sphere path', () => {
    // C3's budget rule: full-resolution texels and spheres are spent on the
    // active stage only. It is what `auto` does, not an invariant of the kind.
    const p: any = stackParams()
    p.viz = { ...p.viz, 'render mode': 'auto' }
    const st = new Stack(p, ctx(), false)
    expect(st.stageRenderMode(false)).toBe('heatmap')
    expect(st.stageRenderMode(true)).toBe('auto')
  })

  it('honours an explicit render mode on every stage, active or not', () => {
    // Without this the model view forces heatmap everywhere and the Render
    // control silently does nothing — the exact failure "no method may
    // silently no-op" is about, one level up.
    for (const want of ['spheres', 'heatmap']) {
      const p: any = stackParams()
      p.viz = { ...p.viz, 'render mode': want }
      const st = new Stack(p, ctx(), false)
      expect(st.stageRenderMode(false)).toBe(want)
      expect(st.stageRenderMode(true)).toBe(want)
    }
  })

  describe('row flow — how the layers are arranged', () => {
    // Two rows: s0 and s1 together, then s2 on its own. `row flow` decides
    // which way the rows advance and which way stages run inside one.
    const laid = (flow: string | undefined) => {
      const p: any = stackParams()
      p.layout = { ...p.layout, ...(flow ? { 'row flow': flow } : {}) }
      p.stages.s0.row = 0
      p.stages.s1.row = 0
      p.stages.s2.row = 1
      const st = new Stack(p, vizCtx(), true)
      return { st, pos: st.stages.map(s => s.obj.group.position) }
    }
    const margin = 2 * 4        // OPTS' gap of 2, times layoutStages' 4

    it('vertical stacks rows in y and runs stages across a row in x', () => {
      const { st, pos } = laid('vertical')
      expect([pos[0].x, pos[0].y]).toEqual([0, 0])
      expect(pos[1].x).toBeGreaterThan(0)         // second stage of row 0
      expect(pos[1].y).toBe(0)                    // …same row, same y
      expect(pos[2].x).toBe(0)                    // row 1 restarts at x = 0
      expect(pos[2].y).toBeGreaterThan(0)         // …one row up
      const e = st.stages.map(s => s.obj.getExtent())
      expect(st.getExtent().x).toBeCloseTo(
        Math.max(e[0].x + margin + e[1].x, e[2].x), 6)      // widest row
      expect(st.getExtent().y).toBeCloseTo(
        Math.max(e[0].y, e[1].y) + margin + e[2].y, 6)      // rows summed
    })

    it('horizontal advances rows in x and stacks stages within a row in y', () => {
      const { st, pos } = laid('horizontal')
      expect([pos[0].x, pos[0].y]).toEqual([0, 0])
      expect(pos[1].x).toBe(0)                    // second stage of row 0…
      expect(pos[1].y).toBeGreaterThan(0)         // …stacked above it
      expect(pos[2].x).toBeGreaterThan(0)         // row 1 is the next column
      expect(pos[2].y).toBe(0)
      const e = st.stages.map(s => s.obj.getExtent())
      expect(st.getExtent().y).toBeCloseTo(
        Math.max(e[0].y + margin + e[1].y, e[2].y), 6)      // tallest column
      expect(st.getExtent().x).toBeCloseTo(
        Math.max(e[0].x, e[1].x) + margin + e[2].x, 6)      // columns summed
    })

    it('is vertical when the scene does not say — a params tree built before it', () => {
      const { pos } = laid(undefined)
      expect(pos[1].y).toBe(0)
      expect(pos[2].x).toBe(0)
      expect(pos[2].y).toBeGreaterThan(0)
    })

    it('never turns a stage on its side: the same stage keeps its own extent', () => {
      const v = laid('vertical').st, h = laid('horizontal').st
      for (let i = 0; i < 3; i++) {
        expect(h.stages[i].obj.getExtent()).toEqual(v.stages[i].obj.getExtent())
      }
    })
  })

  it('refuses an empty stack rather than drawing an empty scene', () => {
    expect(() => new Stack({ ...OPTS(), name: 'm', op: 'stack', stages: {} }, ctx(), false))
      .toThrow(/no stages/)
  })

  it('is built by buildOpNode, which is what main.js dispatches on', () => {
    expect(buildOpNode(stackParams(), ctx(), false)).toBeInstanceOf(Stack)
    expect(() => buildOpNode({ op: 'nope' }, ctx(), false)).toThrow(/unknown node op/)
  })
})

describe('the expression grammar refuses what it cannot spell', () => {
  // The landmine C1 names. `parseExpr` maps '@' and nothing else, and
  // `genExpr`/`syncExpr` round-trip a tree back into that notation. Handed a
  // tree with an `add` in it they would print `x @ y` where an add is drawn —
  // a matmul claimed for something that is not one. Both refuse instead.
  const addTree = () => ({
    name: 'x + y', op: 'add', left: lf('x', 2, 2), right: lf('y', 2, 2),
  })
  const matmulTree = () => ({
    name: 'p', left: lf('a', 2, 3), right: lf('b', 3, 2), expr: '',
  })

  it('still round-trips an ordinary matmul tree', () => {
    expect(genExpr(matmulTree())).toBe('p = a @ b')
    expect(treeHasOps(matmulTree())).toBe(false)
  })

  it('never prints an @ for a tree containing an add', () => {
    const e = genExpr(addTree())
    expect(e).not.toContain('@')
    expect(e).toContain('not an expression')
    expect(e).toContain('1 add')
  })

  it('never prints an @ for a matmul tree with an add buried inside it', () => {
    // The dangerous shape: `left`/`right` are both present, so nothing about
    // the node's *structure* says it is not a matmul.
    const buried = { name: 'p', left: addTree(), right: lf('b', 2, 2), expr: '' }
    expect(treeHasOps(buried)).toBe(true)
    expect(genExpr(buried)).not.toContain('@')
  })

  it('describes a stack rather than pretending to spell it', () => {
    const st = {
      name: 'model', op: 'stack',
      stages: { s0: matmulTree(), s1: addTree() },
    }
    const d = describeTree(st)
    expect(d).toContain('2 stages')
    expect(d).toContain('1 add')
    expect(d).not.toContain('@')
  })

  it('syncExpr refuses rather than rebuilding the tree as matmuls', () => {
    // childParams() rebuilds every node as a matmul or a leaf, so parsing over
    // this tree would silently turn the add into a matmul.
    const p: any = { ...addTree(), expr: 'x @ y' }
    expect(syncExpr(p)).toBe(false)
    expect(p.op).toBe('add')        // untouched
    expect(p.left.name).toBe('x')
  })

  it('syncExpr still works on a tree it can spell', () => {
    const p: any = { ...matmulTree(), expr: 'c @ d', layout: { scheme: 'blocks' } }
    expect(syncExpr(p)).toBe(true)
    expect(p.left.name).toBe('c')
  })
})

describe('mergeVizSummaries', () => {
  const s = (o = {}) => ({
    absmin: 0, absmax: 1, mats: 1, encoding: 'magnitude', reducer: 'maxAbs',
    lod: 1, texels: 10, elements: 0, heatmaps: 1, ...o,
  })

  it('reports the coarsest level anything on screen is at, never the average', () => {
    // What a viewer needs to know is whether *anything* is reduced.
    expect(mergeVizSummaries([s(), s({ lod: 8 }), s()]).lod).toBe(8)
  })

  it('says "mixed" rather than picking one encoding and implying it holds', () => {
    expect(mergeVizSummaries([s(), s({ encoding: 'signed' })]).encoding).toBe('mixed')
    expect(mergeVizSummaries([s(), s()]).encoding).toBe('magnitude')
  })

  it('leaves the encoding null when nothing is a heatmap at all', () => {
    expect(mergeVizSummaries([s({ encoding: null }), s({ encoding: null })]).encoding).toBe(null)
  })

  it('adds up the counts and widens the range', () => {
    const m = mergeVizSummaries([s({ absmax: 3 }), s({ absmin: -1, texels: 5, heatmaps: 0 })])
    expect([m.absmin, m.absmax, m.mats, m.texels, m.heatmaps]).toEqual([-1, 3, 2, 15, 1])
  })
})
