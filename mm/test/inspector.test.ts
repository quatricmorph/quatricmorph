//
// inspector.ts — the tensor inspector panel.
//
// Runs under jsdom against the *real* SelectionManager, EditStack and
// VisibilityState, so what is pinned is the panel's contract with the
// managers, not a mock of it. Only the scene tree is faked, and only down to
// the surface the inspector and the managers actually touch (get / resolve /
// mats / entities plus a root for EditStack's recompute walk). The fake mats
// deliberately carry no `points`, so every heatmap-info access has to survive
// the guard — a fake with getHeatmapInfo would hide a missing one.
//
// Deliberately untested: appearance (the stylesheet is pinned only as
// injected-once-and-shared), the collapse cosmetics beyond the display
// toggle, and anything owned by the real SceneTree walk or the renderer —
// picking, highlights and framing live elsewhere and never under jsdom.
//
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { createInspector } from '../src/inspector.js'
import { SelectionManager, VisibilityState } from '../src/selection.js'
import { EditStack } from '../src/editops.js'
import { Array2D } from '../src/viz.js'

//
// The fake tree: just enough SceneTree for the inspector and the managers.
//

const fakeMat = (name: string, h: number, w: number, values: number[]): any => ({
  H: h, W: w,
  data: new Array2D(h, w, new Float32Array(values)),
  params: { name },
  getBlockInfo: () => ({ i: { n: 1, size: h, max: h }, j: { n: 1, size: w, max: w } }),
  // NO `points` and NO `getHeatmapInfo`: the inspector must guard on
  // mat.points before asking about the heatmap, or these fakes throw.
})

const matEntity = (path: string, mat: any): any => ({
  path, role: path.split('/').pop(), name: mat.params.name, kind: 'mat',
  depth: 1, parent: null, children: [], node: mat, mat, stage: null,
})

class FakeTree {
  root: any
  entities: any[]
  private byPath = new Map<string, any>()

  constructor(mats: Record<string, any>) {
    const children = Object.entries(mats).map(([path, mat]) => matEntity(path, mat))
    // A 'stack' root: EditStack.recomputeAll walks root.children (applying
    // ops to each mat) and then stops at the stack boundary, so no fake
    // matmul recompute is ever attempted.
    this.root = {
      path: 'root', role: 'root', name: 'root', kind: 'stack', depth: 0,
      parent: null, children, node: {}, mat: null, stage: null,
    }
    this.entities = [this.root, ...children]
    for (const e of this.entities) this.byPath.set(e.path, e)
  }

  get(path: string) { return this.byPath.get(path) || null }

  resolve(path: string) {
    const segs = path.split('/')
    while (segs.length) {
      const e = this.byPath.get(segs.join('/'))
      if (e) return e
      segs.pop()
    }
    return null
  }

  mats() { return this.entities.filter(e => e.kind === 'mat') }
}

//
// Harness: real managers wired the way the interaction controller wires them.
//

let tree: FakeTree
let selection: SelectionManager
let visibility: VisibilityState
let edits: EditStack
let applySpy: any
let focusSpy: any
let cursor: { path: string, i: number, j: number, value: number } | null
let made: any[] = []

// Returns `any` for the same reason `q` below is any: the suite reaches for
// .click()/.value on looked-up elements throughout.
const build = (mats: Record<string, any> = { X: fakeMat('X', 2, 2, [1, 2, 3, 4]) }): any => {
  tree = new FakeTree(mats)
  selection = new SelectionManager()
  selection.setTree(tree as any)
  visibility = new VisibilityState()
  edits = new EditStack(() => tree as any)
  applySpy = vi.fn()
  focusSpy = vi.fn()
  cursor = null
  const insp = createInspector({
    selection, edits, visibility,
    getTree: () => tree as any,
    getCursor: () => cursor,
    getLevel: () => 'matrix',
    focusEntity: focusSpy,
    applyOpToSelection: applySpy,
  })
  document.body.appendChild(insp.root)
  made.push(insp)
  return insp
}

// Typed `any` deliberately, mirroring gpt2page.ts's `$`: everything looked up
// is an <input>, <select>, <button> or <textarea>, and the alternative is a
// cast at every call site (querySelector returns Element, which has no
// .click() or .value).
const q = (insp: any, sel: string): any => insp.root.querySelector(sel)
const qa = (insp: any, sel: string): any => insp.root.querySelectorAll(sel)
const text = (insp: any, sel: string): string => q(insp, sel).textContent

beforeEach(() => { document.body.innerHTML = ''; made = [] })
afterEach(() => { made.forEach(m => m.dispose()); made = [] })

describe('createInspector', () => {
  it('says nothing is selected when the selection is empty', () => {
    const insp = build()
    expect(text(insp, '.qme-selection')).toContain('nothing selected')
    expect(text(insp, '.qme-selection')).toContain('level: matrix')
  })

  it('shows hand-computed exact statistics for a fully selected 2×2 matrix, without an explicit refresh call', () => {
    // [1 2; 3 4] selected whole. Every expected string is hand-computed
    // through address.ts's fmtValue (toPrecision(5) at these magnitudes):
    //   mean = (1+2+3+4)/4 = 2.5              → '2.5000'
    //   min  = 1, max = 4                     → '1.0000', '4.0000'
    //   L1   = 1+2+3+4 = 10                   → '10.000'
    //   L2   = sqrt(1+4+9+16) = sqrt(30)      → 5.47722… → '5.4772'
    //   std  = sqrt(30/4 − 2.5²) = sqrt(1.25) → 1.11803… → '1.1180'
    // No insp.refresh() anywhere: selectEntity fires onChange, which is the
    // subscription the panel lives on.
    const insp = build()
    selection.selectEntity('X')
    const t = text(insp, '.qme-stats')
    expect(t).toContain('cells 4, finite 4')
    expect(t).toContain('min 1.0000')
    expect(t).toContain('max 4.0000')
    expect(t).toContain('mean 2.5000')
    expect(t).toContain('std 1.1180')
    expect(t).toContain('L1 10.000')
    expect(t).toContain('L2 5.4772')
    expect(t).toContain('zeros 0')
  })

  it('labels the statistics exact, because they come from the Array2D and not the picture', () => {
    const insp = build()
    selection.selectEntity('X')
    expect(insp.root.querySelector('.qme-stats .qme-badge').textContent).toBe('exact')
  })

  it('counts the selection in entities and cells', () => {
    const insp = build()
    selection.selectEntity('X')
    expect(text(insp, '.qme-selection')).toContain('1 entity, 4 cells')
  })

  it('describes the active mat as picture: elements when it has no points, without touching getHeatmapInfo', () => {
    // The fake mat defines no getHeatmapInfo at all: if the guard on
    // mat.points were missing, this render would throw instead of printing.
    const insp = build()
    selection.selectEntity('X')
    const t = text(insp, '.qme-active')
    expect(t).toContain('X')
    expect(t).toContain('kind: mat')
    expect(t).toContain('shape: 2 × 2')
    expect(t).toContain('picture: elements')
    expect(t).toContain('values: exact FP32 in memory')
  })

  it('frames the active entity through focusEntity when the breadcrumb is clicked', () => {
    const insp = build()
    selection.selectEntity('X')
    insp.root.querySelector('.qme-breadcrumb').click()
    expect(focusSpy).toHaveBeenCalledWith('X')
  })

  it('prints the hovered cell as name[i, j] = value with its display block coordinates', () => {
    // data.get(1, 0) of [1 2; 3 4] is 3; fmtValue(3) = '3.0000'. One block
    // per axis (size = H, W), so (1, 0) sits in block [0, 0].
    const insp = build()
    insp.setHover({ path: 'X', name: 'X', i: 1, j: 0, value: 3 })
    const t = text(insp, '.qme-hover')
    expect(t).toContain('X[1, 0] = 3.0000')
    expect(t).toContain('block [0, 0]')
  })

  it('clears the hover section when the pointer leaves', () => {
    const insp = build()
    insp.setHover({ path: 'X', name: 'X', i: 0, j: 0, value: 1 })
    insp.setHover(null)
    expect(text(insp, '.qme-hover')).toBe('Hover')   // only the heading remains
  })

  it('renders tensor names as text, so markup in a checkpoint name cannot become DOM', () => {
    const insp = build()
    insp.setHover({ path: 'X', name: '<b>sneaky</b>', i: 0, j: 0, value: 1 })
    expect(insp.root.querySelector('b')).toBeNull()
    expect(text(insp, '.qme-hover')).toContain('<b>sneaky</b>[0, 0]')
  })

  it('shows the cursor hint until a cursor is set, then the pinned cell', () => {
    const insp = build()
    expect(text(insp, '.qme-cursor')).toContain('not set (press C over a cell)')
    cursor = { path: 'X', i: 0, j: 1, value: 2 }
    insp.refresh()
    expect(text(insp, '.qme-cursor')).toContain('X[0, 1] = 2.0000')
  })

  it('applies the toolbar op to the selection with the parsed numeric value', () => {
    const insp = build()
    selection.selectEntity('X')
    const kind = insp.root.querySelector('.qme-kind')
    kind.value = 'scale'
    kind.dispatchEvent(new Event('change'))
    insp.root.querySelector('.qme-value').value = '2'
    insp.root.querySelector('.qme-apply').click()
    expect(applySpy).toHaveBeenCalledTimes(1)
    expect(applySpy).toHaveBeenCalledWith('scale', { value: 2 })
  })

  it('refuses to apply an op whose value does not parse to a number', () => {
    // An empty value field parses to NaN; a NaN would poison every
    // downstream product, so nothing may reach the controller.
    const insp = build()
    selection.selectEntity('X')
    const kind = insp.root.querySelector('.qme-kind')
    kind.value = 'scale'
    kind.dispatchEvent(new Event('change'))
    insp.root.querySelector('.qme-value').value = ''
    insp.root.querySelector('.qme-apply').click()
    expect(applySpy).not.toHaveBeenCalled()
  })

  it('sends min and max, and only those that parse, for a clamp op', () => {
    const insp = build()
    const kind = insp.root.querySelector('.qme-kind')
    kind.value = 'clamp'
    kind.dispatchEvent(new Event('change'))
    insp.root.querySelector('.qme-min').value = '-1'
    insp.root.querySelector('.qme-max').value = '1'
    insp.root.querySelector('.qme-apply').click()
    expect(applySpy).toHaveBeenCalledWith('clamp', { min: -1, max: 1 })
  })

  it('lists an op in stack order and drops it from the DOM after removeOp + refresh', () => {
    const insp = build()
    const op = edits.addOp('X', null, 'scale', { value: 2 })
    expect(op).toBeTruthy()
    expect(text(insp, '.qme-ops')).toContain('#1 scale(value=2.0000) on X [whole]')
    edits.removeOp(op.id)
    insp.refresh()
    expect(text(insp, '.qme-ops')).not.toContain('#1 scale')
    expect(text(insp, '.qme-ops')).toContain('no edits')
  })

  it('shows the two honesty notes only while the stack is non-empty', () => {
    const insp = build()
    expect(text(insp, '.qme-edits')).not.toContain('colour range pinned at load')
    edits.addOp('X', null, 'zero', {})
    const t = text(insp, '.qme-edits')
    expect(t).toContain('colour range pinned at load; stats are exact')
    expect(t).toContain('edits propagate within a stage, not across stack stages')
  })

  it('toggles an op through EditStack.setEnabled via its checkbox, never directly', () => {
    const insp = build()
    edits.addOp('X', null, 'zero', {})
    insp.root.querySelector('.qme-op-enabled').click()
    expect(edits.ops[0].enabled).toBe(false)
  })

  it('moves an op down the stack with the ▼ button', () => {
    const insp = build()
    edits.addOp('X', null, 'scale', { value: 2 })
    edits.addOp('X', null, 'add', { value: 1 })
    insp.root.querySelectorAll('.qme-op')[0].querySelector('.qme-op-down').click()
    expect(edits.ops.map((o: any) => o.id)).toEqual([2, 1])
  })

  it('reveals the serialized stack in a textarea instead of reaching for the clipboard', () => {
    const insp = build()
    edits.addOp('X', null, 'fill', { value: 7 })
    const out = insp.root.querySelector('.qme-export-out')
    expect(out.style.display).toBe('none')
    insp.root.querySelector('.qme-export').click()
    expect(out.style.display).toBe('block')
    const parsed = JSON.parse(out.value)
    expect(parsed.version).toBe(1)
    expect(parsed.ops).toHaveLength(1)
    expect(parsed.ops[0].kind).toBe('fill')
  })

  it('saves and re-applies a named selection set through the manager', () => {
    const insp = build()
    selection.selectEntity('X')
    insp.root.querySelector('.qme-set-name').value = 'mine'
    insp.root.querySelector('.qme-set-save').click()
    selection.clear()
    expect(selection.has('X')).toBe(false)
    const sel = insp.root.querySelector('.qme-set-select')
    expect([...sel.options].map((o: any) => o.value)).toContain('mine')
    sel.value = 'mine'
    insp.root.querySelector('.qme-set-apply').click()
    expect(selection.has('X')).toBe(true)
  })

  it('collapses the body behind the header toggle and reopens it', () => {
    const insp = build()
    const toggle = insp.root.querySelector('.qme-toggle')
    const body = insp.root.querySelector('.qme-body')
    toggle.click()
    expect(body.style.display).toBe('none')
    expect(toggle.textContent).toBe('▸')
    toggle.click()
    expect(body.style.display).toBe('')
    expect(toggle.textContent).toBe('▾')
  })

  it('notes hidden entities after an explicit refresh — visibility has no subscription', () => {
    const insp = build()
    selection.selectEntity('X')
    visibility.hide(['X'])
    insp.refresh()
    expect(text(insp, '.qme-selection')).toContain('1 hidden')
  })

  it('injects one shared stylesheet for any number of inspectors and leaves it behind on dispose', () => {
    const a = build()
    const b = build()
    expect(document.querySelectorAll('#qme-inspector-style')).toHaveLength(1)
    a.dispose()
    b.dispose()
    expect(document.getElementById('qme-inspector-style')).toBeTruthy()
  })

  it('stops listening after dispose: a later edit no longer re-renders the panel', () => {
    const insp = build()
    insp.dispose()
    expect(document.body.contains(insp.root)).toBe(false)
    edits.addOp('X', null, 'zero', {})
    // The detached root still exists; had the subscription survived, the ops
    // list would now show '#1 zero'.
    expect(text(insp, '.qme-ops')).toContain('no edits')
  })
})
