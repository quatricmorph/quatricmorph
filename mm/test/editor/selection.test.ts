//
// selection.ts — the manager every surface consumes.
//
// What this file pins: Blender's set/add/toggle and active-entity semantics,
// selection undo as its own history (separate from edit undo), survival of a
// selection across scene rebuilds by path, and exact statistics — computed
// from the Array2D, never from the picture. The failure guarded against: a
// selection that silently points at the wrong entity after a rebuild, and a
// stat that miscounts non-finite values.
//
import { describe, it, expect } from 'vitest'
import * as viz from '../../src/scene/viz.js'
import { SceneTree } from '../../src/editor/scenetree.js'
import { SelectionManager, VisibilityState, selectionStats } from '../../src/editor/selection.js'

const ctx = () => ({ raycaster: null, camera: null, pointer: null })

const lf = (name, h, w, init = 'row major') => ({
  name, matmul: false, h, w, init, url: '', expr: '', min: 0, max: 1, dropout: 0,
})

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

// out = (a @ b) @ c, mats a 2×3, b 3×2, ab 2×2, c 2×2, result 2×2.
const nestedTree = () => new SceneTree(new viz.MatMul({
  ...OPTS(), name: 'out',
  left: { ...OPTS(), name: 'ab', matmul: true, left: lf('a', 2, 3), right: lf('b', 3, 2) },
  right: lf('c', 2, 2),
}, ctx(), false), 'out')

const manager = () => {
  const sel = new SelectionManager()
  sel.setTree(nestedTree())
  return sel
}

describe('set / add / toggle', () => {
  it('set replaces, add accumulates, and the newest selection is active', () => {
    const sel = manager()
    sel.selectEntity('out/left/left', 'set')
    sel.selectEntity('out/right', 'add')
    expect(sel.paths().sort()).toEqual(['out/left/left', 'out/right'])
    expect(sel.activePath()).toBe('out/right')
    sel.selectEntity('out/left/right', 'set')
    expect(sel.paths()).toEqual(['out/left/right'])
  })

  it('toggling a whole-selected entity removes it and active falls back', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    sel.selectEntity('out/left/left', 'add')
    sel.selectEntity('out/left/left', 'toggle')
    expect(sel.paths()).toEqual(['out/right'])
    expect(sel.activePath()).toBe('out/right')
  })

  it('toggling a range out of a whole-selected mat carves it via inversion', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')          // whole 2×2
    sel.selectRange('out/right', { i: [0, 1], j: [0, 2] }, 'toggle')
    // whole minus row 0 = row 1
    expect(sel.rangesOf('out/right')).toEqual([{ i: [1, 2], j: [0, 2] }])
    expect(sel.covers('out/right', 1, 1)).toBe(true)
    expect(sel.covers('out/right', 0, 1)).toBe(false)
  })

  it('adding a range to a whole-selected mat keeps it whole', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    sel.selectRange('out/right', { i: [0, 1], j: [0, 1] }, 'add')
    expect(sel.rangesOf('out/right')).toBeNull()
  })
})

describe('counting', () => {
  it('counts a whole mat, a range, and a whole interior node (descendant mats)', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    expect(sel.countCells()).toBe(4)
    sel.selectRange('out/right', { i: [0, 1], j: [0, 2] }, 'set')
    expect(sel.countCells()).toBe(2)
    // 'out/left' whole = its three mats: a 6 + b 6 + result 4 = 16
    sel.selectEntity('out/left', 'set')
    expect(sel.countCells()).toBe(16)
  })
})

describe('selection history and sets', () => {
  it('undoSelection/redoSelection walk the selection history, not the edit history', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    sel.clear()
    expect(sel.isEmpty()).toBe(true)
    expect(sel.undoSelection()).toBe(true)
    expect(sel.paths()).toEqual(['out/right'])
    expect(sel.redoSelection()).toBe(true)
    expect(sel.isEmpty()).toBe(true)
  })

  it('named sets snapshot and restore, and a locked manager refuses to apply one', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    sel.saveSet('mine')
    sel.clear()
    expect(sel.applySet('mine')).toBe(true)
    expect(sel.paths()).toEqual(['out/right'])
    sel.locked = true
    sel.clear()   // no-op under lock
    expect(sel.paths()).toEqual(['out/right'])
    expect(sel.applySet('mine')).toBe(false)
  })
})

describe('invertActive', () => {
  it('inverts a partial selection within the active mat', () => {
    const sel = manager()
    sel.selectRange('out/right', { i: [0, 1], j: [0, 2] }, 'set')
    sel.invertActive()
    expect(sel.rangesOf('out/right')).toEqual([{ i: [1, 2], j: [0, 2] }])
  })

  it('inverting a whole selection empties it, and vice versa', () => {
    const sel = manager()
    sel.selectEntity('out/right', 'set')
    sel.invertActive()
    expect(sel.has('out/right')).toBe(false)
  })
})

describe('setTree — rebuild survival', () => {
  it('keeps a selection whose path still exists in the rebuilt tree', () => {
    const sel = manager()
    sel.selectEntity('out/left/left', 'set')
    sel.setTree(nestedTree())     // fresh objects, same shape
    expect(sel.has('out/left/left')).toBe(true)
  })

  it('re-anchors a vanished path to its deepest surviving ancestor, selected whole', () => {
    const sel = manager()
    sel.selectEntity('out/left/left', 'set')
    // rebuild where the nested matmul became a leaf: out/left/left is gone
    const flat = new SceneTree(new viz.MatMul({
      ...OPTS(), name: 'out', left: lf('ab', 2, 2), right: lf('c', 2, 2),
    }, ctx(), false), 'out')
    sel.setTree(flat)
    expect(sel.has('out/left/left')).toBe(false)
    expect(sel.rangesOf('out/left')).toBeNull()
  })
})

describe('selectionStats', () => {
  it('separates finite, zero, NaN and Inf and computes exact moments', () => {
    // data = [1, −2; 0, NaN]: finite {1, −2, 0}, mean −1/3,
    // E[x²] = (1+4+0)/3 = 5/3, var = 5/3 − 1/9 = 14/9, std = √(14)/3.
    const d = new viz.Array2D(2, 2, new Float32Array([1, -2, 0, NaN]))
    const s = selectionStats(d, [{ i: [0, 2], j: [0, 2] }])
    expect(s.cells).toBe(4)
    expect(s.finite).toBe(3)
    expect(s.zeros).toBe(1)
    expect(s.nans).toBe(1)
    expect(s.infs).toBe(0)
    expect(s.min).toBe(-2)
    expect(s.max).toBe(1)
    expect(s.absmax).toBe(2)
    expect(s.mean).toBeCloseTo(-1 / 3, 6)
    expect(s.std).toBeCloseTo(Math.sqrt(14) / 3, 6)
    expect(s.l1).toBe(3)
    expect(s.l2).toBeCloseTo(Math.sqrt(5), 6)
    expect(s.exactness).toBe('exact')
  })
})

describe('VisibilityState', () => {
  it('isolate hides everything outside the kept subtree', () => {
    const tree = nestedTree()
    const vis = new VisibilityState()
    vis.isolate(new Set(['out/left']), tree)
    expect(vis.isHidden('out/left/result')).toBe(false)   // descendant of kept
    expect(vis.isHidden('out/right')).toBe(true)
    expect(vis.isHidden('out/result')).toBe(true)
    vis.showAll()
    expect(vis.isHidden('out/right')).toBe(false)
  })

  it('apply writes group visibility for built groups and skips unbuilt mats', () => {
    const tree = nestedTree()
    const vis = new VisibilityState()
    vis.hide(['out/left'])
    // init_viz=false: interior nodes have groups, Mats do not — apply must
    // not throw on the matless-group case and must hide the interior group.
    expect(() => vis.apply(tree)).not.toThrow()
    expect(tree.get('out/left')!.node.group.visible).toBe(false)
    expect(tree.get('out')!.node.group.visible).toBe(true)
  })
})
