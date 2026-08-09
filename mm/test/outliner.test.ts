//
// outliner.ts — the scene-tree panel.
//
// Built against a *real* SceneTree over a real viz.MatMul with init_viz off —
// the same recipe as viz.test.ts — so the paths, names and dims asserted
// below come from the actual walk, not from a hand-faked tree that could
// drift from it. What is worth pinning is the wiring: which SelectionManager
// call each gesture makes, that the eye button cannot disturb the selection,
// that collapse state outlives refresh() (a scene rebuild calls it), and
// that a name arriving from a URL cannot inject markup into the panel.
//
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import * as viz from '../src/viz.js'
import { SceneTree } from '../src/scenetree.js'
import { SelectionManager, VisibilityState } from '../src/selection.js'
import { createOutliner } from '../src/outliner.js'

const ctx = () => ({ raycaster: null, camera: null, pointer: null })

const lf = (name, h, w) => ({
  name, matmul: false, h, w, init: 'row major', url: '', expr: '', min: 0, max: 1, dropout: 0,
})

// Same OPTS shape as viz.test.ts — the params a MatMul needs to build headless.
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

// One 4×4 matmul: entities out, out/left, out/right, out/result. The root
// params node carries no `matmul` key — the invariant ensureChildCounts
// recognises the root by. '<i>' in the right leaf's name is the
// markup-injection probe for the textContent test below.
const buildTree = () => {
  const p = { ...OPTS(), name: 'out', left: lf('left', 4, 4), right: lf('w<i>', 4, 4) }
  return new SceneTree(new viz.MatMul(p, ctx(), false), 'out')
}

// Matmuls three deep, so an entity with children exists at depth 2 — where
// the depth < 2 default stops expanding.
const buildNestedTree = () => {
  const inner = { ...OPTS(), name: 'qk', matmul: true, left: lf('q', 4, 4), right: lf('k', 4, 4) }
  const mid = { ...OPTS(), name: 'attn', matmul: true, left: inner, right: lf('v', 4, 4) }
  const p = { ...OPTS(), name: 'out', left: mid, right: lf('w', 4, 4) }
  return new SceneTree(new viz.MatMul(p, ctx(), false), 'out')
}

let out = null

const build = (tree = buildTree()) => {
  const selection = new SelectionManager()
  const visibility = new VisibilityState()
  selection.setTree(tree)
  const focused: string[] = []
  out = createOutliner({
    selection, visibility,
    getTree: () => tree,
    focusEntity: p => focused.push(p),
  })
  document.body.appendChild(out.root)
  return { selection, visibility, focused, tree }
}

// Rows are rebuilt on every change, so tests re-query by path each time
// rather than holding an element across a refresh.
const row = (path: string) =>
  out.root.querySelector(`.qme-row[data-path="${path}"]`)

const click = (el, opts = {}) =>
  el.dispatchEvent(new MouseEvent('click', { bubbles: true, ...opts }))

beforeEach(() => { document.body.innerHTML = '' })
afterEach(() => { out && out.dispose(); out = null })

describe('createOutliner', () => {
  it('renders one row per scene entity, with the root expanded by default', () => {
    const { tree } = build()
    expect(tree.entities.length).toBe(4)   // out + left + right + result
    expect(out.root.querySelectorAll('.qme-row').length).toBe(4)
    for (const p of ['out', 'out/left', 'out/right', 'out/result']) {
      expect(row(p)).toBeTruthy()
    }
  })

  it('suffixes each mat row with its H×W and gives interior rows none', () => {
    build()
    expect(row('out/left').querySelector('.qme-dims').textContent).toBe('4×4')
    expect(row('out').querySelector('.qme-dims')).toBeNull()
  })

  it('selects out/left whole on a plain click and marks the row qme-sel and qme-active', () => {
    const { selection } = build()
    click(row('out/left'))
    expect(selection.paths()).toEqual(['out/left'])
    expect(selection.rangesOf('out/left')).toBe(null)   // whole, not partial
    expect(row('out/left').classList.contains('qme-sel')).toBe(true)
    expect(row('out/left').classList.contains('qme-active')).toBe(true)
    expect(row('out/right').classList.contains('qme-sel')).toBe(false)
  })

  it('adds a second entity on shift-click instead of replacing the first', () => {
    const { selection } = build()
    click(row('out/left'))
    click(row('out/right'), { shiftKey: true })
    expect(new Set(selection.paths())).toEqual(new Set(['out/left', 'out/right']))
    expect(selection.activePath()).toBe('out/right')
    expect(row('out/left').classList.contains('qme-sel')).toBe(true)
    expect(row('out/right').classList.contains('qme-active')).toBe(true)
  })

  it('hands the path to focusEntity on double-click', () => {
    const { focused } = build()
    row('out/result').dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))
    expect(focused).toEqual(['out/result'])
  })

  it('toggles visibility from the eye button without touching the selection', () => {
    const { selection, visibility } = build()
    expect(row('out/right').querySelector('.qme-eye').textContent).toBe('👁')
    click(row('out/right').querySelector('.qme-eye'))
    expect(visibility.isHidden('out/right')).toBe(true)
    expect(selection.isEmpty()).toBe(true)   // stopPropagation kept the row click out
    // the visibility change refreshed the panel: the rebuilt row shows hidden
    expect(row('out/right').querySelector('.qme-eye').textContent).toBe('–')
    click(row('out/right').querySelector('.qme-eye'))
    expect(visibility.isHidden('out/right')).toBe(false)
  })

  it('filters to matching rows plus their ancestors, case-insensitively', () => {
    build()
    const filter = out.root.querySelector('.qme-outliner-filter')
    filter.value = 'RESULT'   // matches path 'out/result' only, and only ignoring case
    filter.dispatchEvent(new Event('input', { bubbles: true }))
    expect(row('out/result')).toBeTruthy()
    expect(row('out')).toBeTruthy()          // non-matching ancestor kept for readability
    expect(row('out/left')).toBeNull()
    expect(row('out/right')).toBeNull()
    filter.value = ''
    filter.dispatchEvent(new Event('input', { bubbles: true }))
    expect(row('out/left')).toBeTruthy()     // clearing the filter restores the tree
  })

  it('matches the filter against names as well as paths', () => {
    build()
    const filter = out.root.querySelector('.qme-outliner-filter')
    filter.value = 'w<i'   // the right leaf's *name*; no path contains this
    filter.dispatchEvent(new Event('input', { bubbles: true }))
    expect(row('out/right')).toBeTruthy()
    expect(row('out/left')).toBeNull()
  })

  it('collapses a subtree from its toggle and keeps that across refresh()', () => {
    build()
    click(row('out').querySelector('.qme-toggle'))
    expect(row('out/left')).toBeNull()
    expect(row('out')).toBeTruthy()
    out.refresh()                            // what a scene rebuild calls
    expect(row('out/left')).toBeNull()       // the user's collapse survived it
  })

  it('expands entities below depth 2 by default and leaves deeper subtrees collapsed', () => {
    build(buildNestedTree())
    expect(row('out/left')).toBeTruthy()            // depth 1: expanded
    expect(row('out/left/left')).toBeTruthy()       // visible under it
    expect(row('out/left/left/left')).toBeNull()    // depth-2 subtree: collapsed
  })

  it('revealPath expands collapsed ancestors so the row exists in the DOM, and marks it', () => {
    build()
    click(row('out').querySelector('.qme-toggle'))
    expect(row('out/result')).toBeNull()
    out.revealPath('out/result')
    expect(row('out/result')).toBeTruthy()
    expect(row('out/result').classList.contains('qme-reveal')).toBe(true)
  })

  it('revealPath is a no-op for a path the tree does not have', () => {
    build()
    expect(() => out.revealPath('out/nope')).not.toThrow()
    expect(out.root.querySelectorAll('.qme-row').length).toBe(4)
  })

  it('renders a name containing <i> as text, never as markup', () => {
    build()
    const name = row('out/right').querySelector('.qme-name')
    expect(name.textContent).toBe('w<i>')
    expect(out.root.querySelector('i')).toBeNull()
  })

  it('hides the body behind the header toggle and brings it back', () => {
    build()
    const header = out.root.querySelector('.qme-outliner-header')
    const body = out.root.querySelector('.qme-outliner-body')
    click(header)
    expect(body.style.display).toBe('none')
    click(header)
    expect(body.style.display).not.toBe('none')
  })

  it('injects its style tag once, and dispose() leaves it for the next panel', () => {
    build()
    out.dispose()
    build()
    expect(document.querySelectorAll('#qme-outliner-style').length).toBe(1)
  })

  it('dispose() removes the panel and unhooks it from selection changes', () => {
    const { selection } = build()
    const el = out.root
    out.dispose()
    expect(document.body.contains(el)).toBe(false)
    // Were the subscription still live, this select would refresh the
    // detached panel and mark the row qme-sel.
    selection.selectEntity('out/left')
    expect(el.querySelector('[data-path="out/left"]').classList.contains('qme-sel')).toBe(false)
  })

  it('renders empty rather than throwing while there is no tree yet', () => {
    out = createOutliner({
      selection: new SelectionManager(),
      visibility: new VisibilityState(),
      getTree: () => null,
      focusEntity: () => { },
    })
    document.body.appendChild(out.root)
    expect(out.root.querySelectorAll('.qme-row').length).toBe(0)
    expect(() => out.revealPath('out')).not.toThrow()
  })
})
