//
// editops.ts — the non-destructive edit stack and its recompute propagation.
//
// The arithmetic heart of the editor. Every expectation is hand-computed
// from known initializers ('eye' is the identity; 'expr' leaves evaluate an
// exact expression), so a wrong recompute order or a stale product cannot
// hide behind a plausible picture. What is deliberately untested: visuals
// (refreshTouched needs built geometry; the recompute itself must not).
//
import { describe, it, expect } from 'vitest'
import * as viz from '../src/viz.js'
import { SceneTree } from '../src/scenetree.js'
import { EditStack, applyOpToData } from '../src/editops.js'

const ctx = () => ({ raycaster: null, camera: null, pointer: null })

const lf = (name, h, w, init = 'row major') => ({
  name, matmul: false, h, w, init, url: '', expr: '', min: 0, max: 1, dropout: 0,
})
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

// out = I @ R with R = [[0, 1], [2, 3]] (expr 'i*2+j'), so result === R and
// every downstream expectation is a two-digit integer away from wrong.
const identityParams = () => ({
  ...OPTS(), name: 'out',
  left: lf('L', 2, 2, 'eye'),
  right: ex('R', 2, 2, 'i*2+j'),
})

const build = () => {
  const mm = new viz.MatMul(identityParams(), ctx(), false)
  let tree = new SceneTree(mm, 'out')
  const stack = new EditStack(() => tree)
  return {
    mm, stack,
    retree: (m: any) => { tree = new SceneTree(m, 'out') },
  }
}

const vals = (mat: any) => Array.from(mat.getDataArray())

describe('applyOpToData', () => {
  // one 2×2 [1,2;3,4] per case, every output written out
  const d = () => new viz.Array2D(2, 2, new Float32Array([1, 2, 3, 4]))
  const op = (kind, params = {}, ranges = null) =>
    ({ id: 1, path: 'x', ranges, kind, params, enabled: true }) as any

  it('zero blanks only the addressed range', () => {
    const a = d()
    applyOpToData(a, op('zero', {}, [{ i: [0, 1], j: [0, 2] }]))
    expect(Array.from(a.data)).toEqual([0, 0, 3, 4])
  })

  it('fill, scale, add and clamp do exactly their names, whole-matrix', () => {
    const f = d(); applyOpToData(f, op('fill', { value: 7 }))
    expect(Array.from(f.data)).toEqual([7, 7, 7, 7])
    const s = d(); applyOpToData(s, op('scale', { value: 2 }))
    expect(Array.from(s.data)).toEqual([2, 4, 6, 8])
    const a = d(); applyOpToData(a, op('add', { value: -1 }))
    expect(Array.from(a.data)).toEqual([0, 1, 2, 3])
    const c = d(); applyOpToData(c, op('clamp', { min: 2, max: 3 }))
    expect(Array.from(c.data)).toEqual([2, 2, 3, 3])
  })

  it('ignores range rows beyond the matrix rather than writing out of bounds', () => {
    const a = d()
    applyOpToData(a, op('zero', {}, [{ i: [1, 5], j: [1, 5] }]))
    expect(Array.from(a.data)).toEqual([1, 2, 3, 0])
  })
})

describe('EditStack — recompute from pristine', () => {
  it('an operand edit recomputes the product: 2·I @ R = 2·R', () => {
    const { mm, stack } = build()
    expect(vals(mm.result)).toEqual([0, 1, 2, 3])   // I @ R = R, the baseline
    stack.addOp('out/left', null, 'scale', { value: 2 })
    expect(vals(mm.result)).toEqual([0, 2, 4, 6])
    expect(stack.lastTouched.has('out/left')).toBe(true)
    expect(stack.lastTouched.has('out/result')).toBe(true)
  })

  it('disabling the op restores the original exactly — pristine, not inverse arithmetic', () => {
    const { mm, stack } = build()
    const op = stack.addOp('out/left', null, 'scale', { value: 3 })!
    stack.setEnabled(op.id, false)
    expect(vals(mm.left)).toEqual([1, 0, 0, 1])
    expect(vals(mm.result)).toEqual([0, 1, 2, 3])
  })

  it('an op on the result survives the recompute an operand op forces (post-order)', () => {
    const { mm, stack } = build()
    stack.addOp('out/result', null, 'add', { value: 1 })
    stack.addOp('out/left', null, 'scale', { value: 2 })
    // recompute: result ← 2·R, then +1 re-applied ⇒ [1, 3, 5, 7]
    expect(vals(mm.result)).toEqual([1, 3, 5, 7])
    // removing the operand op leaves result = R + 1
    stack.removeOp(stack.ops.find(o => o.path === 'out/left')!.id)
    expect(vals(mm.result)).toEqual([1, 2, 3, 4])
  })

  it('stack order matters and moveOp swaps it: (I+1)·2 ≠ I·2+1', () => {
    const { mm, stack } = build()
    stack.addOp('out/left', null, 'add', { value: 1 })
    const scale = stack.addOp('out/left', null, 'scale', { value: 2 })!
    expect(vals(mm.left)).toEqual([4, 2, 2, 4])
    stack.moveOp(scale.id, -1)
    expect(vals(mm.left)).toEqual([3, 1, 1, 3])
  })

  it('undo/redo restore both the op list and the data', () => {
    const { mm, stack } = build()
    stack.addOp('out/left', null, 'scale', { value: 2 })
    stack.addOp('out/left', null, 'add', { value: 1 })
    expect(vals(mm.left)).toEqual([3, 1, 1, 3])
    expect(stack.undo()).toBe(true)
    expect(stack.ops).toHaveLength(1)
    expect(vals(mm.left)).toEqual([2, 0, 0, 2])
    expect(stack.redo()).toBe(true)
    expect(vals(mm.left)).toEqual([3, 1, 1, 3])
  })

  it('refuses NaN parameters and non-mat targets before they enter the stack', () => {
    const { stack } = build()
    expect(stack.addOp('out/left', null, 'scale', { value: NaN })).toBeNull()
    expect(stack.addOp('out', null, 'zero', {})).toBeNull()
    expect(stack.ops).toHaveLength(0)
  })

  it('ops reapply as descriptions onto a rebuilt scene (onTreeRebuilt)', () => {
    const { stack, retree } = build()
    stack.addOp('out/left', null, 'scale', { value: 2 })
    const mm2 = new viz.MatMul(identityParams(), ctx(), false)
    retree(mm2)
    const touched = stack.onTreeRebuilt()
    expect(touched.has('out/left')).toBe(true)
    expect(vals(mm2.left)).toEqual([2, 0, 0, 2])
    expect(vals(mm2.result)).toEqual([0, 2, 4, 6])
  })
})

describe('propagation through the other node kinds', () => {
  it('unary: negating relu input recomputes the materialized result', () => {
    // input = [[−2, −1], [0, 1]] → relu [[0,0],[0,1]]. Scale input by −1:
    // input [[2, 1], [0, −1]] → relu [[2, 1], [0, 0]].
    const p = { ...OPTS(), name: 'g', op: 'unary', fn: 'relu', input: ex('x', 2, 2, 'i*2+j-2') }
    // `any`: buildOpNode's union includes Stack, which has no `result`
    const u: any = viz.buildOpNode(p, ctx(), false)
    const tree = new SceneTree(u, 'g')
    const stack = new EditStack(() => tree)
    stack.addOp('g/input', null, 'scale', { value: -1 })
    expect(vals(u.result)).toEqual([2, 1, 0, 0])
  })

  it('add: editing one operand recomputes the elementwise sum', () => {
    const p = {
      ...OPTS(), name: 's', op: 'add',
      left: ex('l', 2, 2, '1'), right: ex('r', 2, 2, '2'),
    }
    const a: any = viz.buildOpNode(p, ctx(), false)
    const tree = new SceneTree(a, 's')
    const stack = new EditStack(() => tree)
    expect(vals(a.result)).toEqual([3, 3, 3, 3])
    stack.addOp('s/left', null, 'add', { value: 1 })
    expect(vals(a.result)).toEqual([4, 4, 4, 4])
  })

  it('stack stages are isolated: editing s0 leaves s1 untouched — the documented boundary', () => {
    const p = {
      ...OPTS(), name: 'st', op: 'stack',
      stages: {
        s0: { ...ex('m0', 2, 2, '5'), row: 0 },
        s1: { ...ex('m1', 2, 2, '7'), row: 0 },
      },
    }
    const st = viz.buildOpNode(p, ctx(), false)
    const tree = new SceneTree(st, 'st')
    const stack = new EditStack(() => tree)
    stack.addOp('st/s0', null, 'fill', { value: 9 })
    expect(vals(tree.get('st/s0')!.mat)).toEqual([9, 9, 9, 9])
    expect(vals(tree.get('st/s1')!.mat)).toEqual([7, 7, 7, 7])
  })
})
