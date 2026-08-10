//
// highlight.ts — what the selection looks like.
//
// What this file pins: rangeRect's display-space arithmetic (hand-computed,
// including a range spanning a block gap), and the structural contract of
// the overlay group — one fill + one outline per selected range, baked
// world matrices (matrixAutoUpdate off) equal to inner_group.matrixWorld
// composed with the translate/scale rangeRect dictates, and clean teardown.
// Deliberately untested: colours on screen (nothing renders here).
//
import { describe, it, expect } from 'vitest'
import * as THREE from 'three'
import * as viz from '../../src/scene/viz.js'
import { SceneTree } from '../../src/editor/scenetree.js'
import { SelectionManager } from '../../src/editor/selection.js'
import { HighlightRenderer, rangeRect } from '../../src/editor/highlight.js'

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

function built() {
  const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  const ctx = { raycaster: new THREE.Raycaster(), camera, pointer: new THREE.Vector2() }
  const mm = new viz.MatMul({
    ...OPTS(), name: 'out', left: lf('L', 4, 4), right: lf('R', 4, 4),
  }, ctx, true)
  mm.group.updateMatrixWorld(true)
  return { mm, tree: new SceneTree(mm, 'out') }
}

describe('rangeRect', () => {
  // A fake mat suffices: rangeRect is display arithmetic over block info.
  const fake = (si: number, sj: number, gap: number) => ({
    getBlockInfo: () => ({ i: { size: si }, j: { size: sj } }),
    params: { layout: { gap } },
  })

  it('covers a plain 2×2 corner range: centre (0.5, 0.5), size 2×2', () => {
    const r = rangeRect(fake(4, 4, 4), { i: [0, 2], j: [0, 2] })
    expect(r).toEqual({ cx: 0.5, cy: 0.5, w: 2, h: 2 })
  })

  it('spans a block gap: columns [3,5) run from x=3 to x=8, so w = 6', () => {
    // element x: j=3 → 3, j=4 → 4 + 1·4 = 8 (si=sj=4, gap 4)
    const r = rangeRect(fake(4, 4, 4), { i: [0, 1], j: [3, 5] })
    expect(r.w).toBe(6)
    expect(r.cx).toBe(5.5)
    expect(r.h).toBe(1)
    expect(r.cy).toBe(0)
  })
})

describe('HighlightRenderer', () => {
  it('draws one fill and one outline per selected mat, with a baked world matrix', () => {
    const { mm, tree } = built()
    const sel = new SelectionManager()
    sel.setTree(tree)
    sel.selectEntity('out/result', 'set')

    const h = new HighlightRenderer()
    h.refresh(tree, sel)
    const sel_group = h.group.children[0]
    expect(sel_group.children).toHaveLength(2)

    const [fill, line] = sel_group.children as any[]
    expect(fill.isMesh).toBe(true)
    expect(line.isLineSegments).toBe(true)
    expect(fill.matrixAutoUpdate).toBe(false)

    // expected matrix, built independently from the same hand numbers:
    // full 4×4 single-block range → rect centre (1.5, 1.5), size 4×4
    const rect = rangeRect(mm.result, { i: [0, 4], j: [0, 4] })
    expect(rect).toEqual({ cx: 1.5, cy: 1.5, w: 4, h: 4 })
    const want = mm.result.inner_group.matrixWorld.clone()
      .multiply(new THREE.Matrix4().makeTranslation(1.5, 1.5, 0))
      .multiply(new THREE.Matrix4().makeScale(4, 4, 1))
    want.elements.forEach((e, k) => expect(fill.matrix.elements[k]).toBeCloseTo(e, 6))
  })

  it('marks the active entity with different materials than the merely selected', () => {
    const { tree } = built()
    const sel = new SelectionManager()
    sel.setTree(tree)
    sel.selectEntity('out/left', 'set')
    sel.selectEntity('out/result', 'add')   // result is now active

    const h = new HighlightRenderer()
    h.refresh(tree, sel)
    const kids = h.group.children[0].children as any[]
    expect(kids).toHaveLength(4)
    const mats = new Set(kids.map(k => k.material))
    expect(mats.size).toBe(4)   // sel fill+line, active fill+line — all distinct
  })

  it('outlines a whole interior entity as a 12-edge box', () => {
    const { tree } = built()
    const sel = new SelectionManager()
    sel.setTree(tree)
    sel.selectEntity('out', 'set')   // the matmul, not a mat

    const h = new HighlightRenderer()
    h.refresh(tree, sel)
    const kids = h.group.children[0].children as any[]
    expect(kids).toHaveLength(1)
    expect(kids[0].isLineSegments).toBe(true)
    expect(kids[0].geometry.attributes.position.count).toBe(24)
  })

  it('hover shows one outline and clears; the cursor shows a cell plus crosshair arms', () => {
    const { mm, tree } = built()
    const h = new HighlightRenderer()
    const hover_group = h.group.children[1]
    const cursor_group = h.group.children[2]

    h.setHover({ mat: mm.result, range: { i: [1, 2], j: [0, 4] } })
    expect(hover_group.children).toHaveLength(1)
    h.setHover(null)
    expect(hover_group.children).toHaveLength(0)

    h.setCursor({ mat: mm.result, i: 2, j: 3 })
    expect(cursor_group.children).toHaveLength(3)
    h.setCursor(null)
    expect(cursor_group.children).toHaveLength(0)
  })

  it('refresh replaces overlays rather than accumulating them', () => {
    const { tree } = built()
    const sel = new SelectionManager()
    sel.setTree(tree)
    sel.selectEntity('out/result', 'set')
    const h = new HighlightRenderer()
    h.refresh(tree, sel)
    h.refresh(tree, sel)
    expect(h.group.children[0].children).toHaveLength(2)
    h.dispose()
    expect(h.group.children[0].children).toHaveLength(0)
  })
})
