//
// scenetree.ts — the logical index over a built viz tree.
//
// What this file pins: that paths are a pure function of the params tree's
// shape (so selections can survive rebuilds), that every node kind is walked,
// and that the display↔cell coordinate inverses agree with elementPosition —
// the same arithmetic emptyPoints/blockQuad are pinned to in points.test.ts.
// The failure guarded against: a pick or overlay landing one cell (or one
// block gap) off, which looks fine until a gap is crossed.
//
import { describe, it, expect } from 'vitest'
import * as viz from '../src/viz.js'
import { elementPosition } from '../src/heatmap.js'
import {
  SceneTree, nodeKind, matLayoutInfo, cellLocal, localToCell, dispToCell,
} from '../src/scenetree.js'

const ctx = () => ({ raycaster: null, camera: null, pointer: null })

const lf = (name, h, w, init = 'row major') => ({
  name, matmul: false, h, w, init, url: '', expr: '', min: 0, max: 1, dropout: 0,
})

const OPTS = () => ({
  epilog: 'none',
  anim: { alg: 'none', speed: 16, fuse: 'none', 'hide inputs': false, spin: 0 },
  block: { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 },
  layout: {
    scheme: 'blocks', gap: 4, scatter: 0, molecule: 1, blast: 0,
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

// out = (a @ b) @ c — a nested matmul, the shape the path scheme must survive.
const nested = () => new viz.MatMul({
  ...OPTS(), name: 'out',
  left: { ...OPTS(), name: 'ab', matmul: true, left: lf('a', 2, 3), right: lf('b', 3, 2) },
  right: lf('c', 2, 2),
}, ctx(), false)

describe('SceneTree paths', () => {
  it('derives stable role-chain paths for every node of a nested matmul', () => {
    const t = new SceneTree(nested(), 'out')
    expect([...t.byPath.keys()].sort()).toEqual([
      'out', 'out/left', 'out/left/left', 'out/left/result', 'out/left/right',
      'out/result', 'out/right',
    ].sort())
    expect(t.get('out')!.kind).toBe('matmul')
    expect(t.get('out/left')!.kind).toBe('matmul')
    expect(t.get('out/left/left')!.kind).toBe('mat')
    expect(t.get('out/left/left')!.name).toBe('a')
    expect(t.mats().map(e => e.path).sort()).toEqual([
      'out/left/left', 'out/left/result', 'out/left/right', 'out/result', 'out/right',
    ].sort())
  })

  it('resolve falls back to the deepest surviving ancestor, never to an unrelated node', () => {
    const t = new SceneTree(nested(), 'out')
    expect(t.resolve('out/left/left/bogus')!.path).toBe('out/left/left')
    expect(t.resolve('nowhere')).toBeNull()
  })

  it('walks a stack into per-stage subtrees keyed by stage key', () => {
    const p = {
      ...OPTS(), name: 'model', op: 'stack',
      stages: {
        s0: { ...lf('emb', 2, 2), row: 0 },
        s1: {
          ...OPTS(), name: 'qk', matmul: true, row: 1,
          left: lf('q', 2, 2), right: lf('k', 2, 2),
        },
      },
    }
    const st = viz.buildOpNode(p, ctx(), false)
    const t = new SceneTree(st, 'model')
    expect(t.get('model')!.kind).toBe('stack')
    expect(t.get('model/s0')!.kind).toBe('mat')
    expect(t.get('model/s0')!.stage.name).toBe('emb')
    expect(t.get('model/s1')!.kind).toBe('matmul')
    expect(t.get('model/s1/result')).toBeTruthy()
  })

  it('registers no pickable objects before initViz builds any', () => {
    const t = new SceneTree(nested(), 'out')
    expect(t.byObject.size).toBe(0)
    expect(t.entityForObject({})).toBeNull()
  })

  it('nodeKind refuses an unrecognizable object, naming its keys', () => {
    expect(() => nodeKind({ foo: 1 })).toThrow(/unrecognized node kind/)
  })
})

describe('display ↔ cell coordinates', () => {
  // An 8×8 mat split 2×2 blocks (si = sj = 4) with gap 4: display x of
  // column j is j + floor(j/4)·4 — 0,1,2,3 then 8,9,10,11 across the gap.
  const blockedMat = () => {
    const p = {
      ...OPTS(), name: 'g',
      block: { 'i blocks': 2, 'k blocks': 2, 'j blocks': 2 },
      left: lf('a', 8, 8), right: lf('b', 8, 8),
    }
    return new viz.MatMul(p, ctx(), false).left
  }

  it('cellLocal is elementPosition — one arithmetic, not a restatement', () => {
    const mat = blockedMat()
    const info = matLayoutInfo(mat)
    expect(info.i.size).toBe(4)
    expect(info.gap).toBe(4)
    for (const [i, j] of [[0, 0], [3, 3], [4, 0], [7, 7], [2, 5]]) {
      const want = elementPosition(i, j, info)
      const got = cellLocal(mat, i, j)
      expect(got.x).toBe(want.x)
      expect(got.y).toBe(want.y)
      expect(got.z).toBe(0)
    }
    // hand-check the gap jump itself: j=3 → 3, j=4 → 8
    expect(cellLocal(mat, 0, 3).x).toBe(3)
    expect(cellLocal(mat, 0, 4).x).toBe(8)
  })

  it('dispToCell inverts elementPosition for every cell, across the gap', () => {
    const mat = blockedMat()
    const info = matLayoutInfo(mat)
    for (let j = 0; j < 8; j++) {
      const { x } = elementPosition(0, j, info)
      expect(dispToCell(x, 4, 4, 8)).toBe(j)
    }
  })

  it('a coordinate in a block gap snaps into the block the stride puts it in', () => {
    // x = 5.9 lies in the gap after block 0 (content ends at 3, block 1
    // starts at 8). floor(5.9/8) = 0, round(5.9) = 6 clamps to the block's
    // last cell: 3. Deterministic, not nearest-across-the-gap — pinned so a
    // future 'improvement' has to say so.
    expect(dispToCell(5.9, 4, 4, 8)).toBe(3)
    expect(dispToCell(8.2, 4, 4, 8)).toBe(4)
    // clamped at both ends of the axis
    expect(dispToCell(-3, 4, 4, 8)).toBe(0)
    expect(dispToCell(1e6, 4, 4, 8)).toBe(7)
  })

  it('localToCell maps a local point to (i, j) with y as the row axis', () => {
    const mat = blockedMat()
    expect(localToCell(mat, 8.2, 0.4)).toEqual({ i: 0, j: 4 })
    // y = 9.1: block 1 spans display 8..11, round(9.1 − 8) = 1 → row 5
    expect(localToCell(mat, 0, 9.1)).toEqual({ i: 5, j: 0 })
  })
})
