//
// gui.js — the lil-gui control panel.
//
// Widget cosmetics are not worth pinning. What is worth pinning is that the
// panel *builds at all* against the real default params, and that its controls
// are wired to the same param paths the rest of the app reads. gui.js reaches
// deep into params (`p.deco['lens size']`, `p.viz['elem scale']`, the left/right
// matmul spine), so a rename anywhere in the params tree breaks it -- and it
// breaks at panel-construction time, which is before the scene draws.
//
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { initGui } from '../src/gui.js'
import { defaultParams } from '../src/params.js'

// Stands in for the live viz.Mat. Controls call it back by name
// (`getObj().setLegends(x)`, `.setRowGuides(x)`, …), so rather than enumerate
// those names -- and have the fake go stale the moment one is added -- any
// method access returns a recording stub. `params` is the one real field: set()
// compares against it to decide whether the change is a no-op.
const fakeObj = (): any => {
  const calls = {}
  const target = { params: {}, calls }
  return new Proxy(target, {
    get: (t, k) => k in t ? t[k] : (t[k] = vi.fn()),
  })
}

const callbacks = (obj = fakeObj()) => ({
  initObj: vi.fn(),
  getObj: vi.fn(() => obj),
  saveUrl: vi.fn(),
  updateTitle: vi.fn(),
  animPause: vi.fn(),
  animStep: vi.fn(),
})

const info = () => ({
  url_info: { json: '', url: '', compressed: '', search_params: '' },
  render_info: { geometries: 0 },
})

let gui = null

beforeEach(() => { document.body.innerHTML = '' })
afterEach(() => { gui && gui.destroy(); gui = null })

const build = (params = defaultParams()) => (gui = initGui(params, callbacks(), info()))

describe('initGui', () => {
  it('builds the whole panel against the default params', () => {
    expect(() => build()).not.toThrow()
    expect(gui).toBeTruthy()
  })

  it('titles the panel and attaches it to the document', () => {
    build()
    expect(gui._title).toBe('mm')
    expect(document.body.contains(gui.domElement)).toBe(true)
  })

  it('opens the top-level folders the default params ask for', () => {
    const p = defaultParams()
    expect(p.folder).toBe('open')
    build(p)
    expect(gui._closed).toBe(false)
  })

  it('creates a folder for each params section', () => {
    build()
    const titles = gui.folders.map(f => f._title)
    for (const t of ['left', 'right', 'deco', 'colors and sizes', 'diag']) {
      expect(titles).toContain(t)
    }
  })

  it('binds the deco controls to the deco params, spelling included', () => {
    // These keys have spaces in them and are read back by main.js and viz.js.
    // A rename on either side silently unbinds the control.
    build()
    const deco = gui.folders.find(f => f._title === 'deco')
    const props = deco.controllers.map(c => c.property)
    for (const k of ['legends', 'shape', 'row guides', 'flow guides', 'spotlight',
      'lens size', 'magnification', 'interior spotlight']) {
      expect(props).toContain(k)
    }
  })

  it('binds the colour controls to the viz params', () => {
    build()
    const viz = gui.folders.find(f => f._title === 'colors and sizes')
    const props = viz.controllers.map(c => c.property)
    for (const k of ['sensitivity', 'min size', 'min light', 'max light',
      'elem scale', 'zero hue', 'hue gap', 'hue spread']) {
      expect(props).toContain(k)
    }
  })

  it('writes changes back into the params object it was given', () => {
    const p = defaultParams()
    build(p)
    const deco = gui.folders.find(f => f._title === 'deco')
    const legends = deco.controllers.find(c => c.property === 'legends')
    legends.setValue(3)
    expect(p.deco.legends).toBe(3)
  })

  it('pushes the change on to the live object as well as into params', () => {
    // Both halves matter: params is what gets serialised into the URL, and the
    // object is what is on screen. A control that updates only one of them
    // produces a scene that disagrees with its own link.
    const obj = fakeObj()
    const p = defaultParams()
    gui = initGui(p, callbacks(obj), info())
    const deco = gui.folders.find(f => f._title === 'deco')
    deco.controllers.find(c => c.property === 'legends').setValue(4)
    expect(p.deco.legends).toBe(4)
    expect(obj.setLegends).toHaveBeenCalledWith(4)
  })

  it('replaces the previous panel rather than stacking a second one', () => {
    // gui.js keeps the GUI in a module-level variable and destroys it on
    // reinit. main.js calls initGui again on every params reload, so a leak
    // here would put a new panel on screen for each one.
    build()
    const first = gui.domElement
    build()
    expect(document.body.contains(first)).toBe(false)
    expect(document.body.contains(gui.domElement)).toBe(true)
  })
})

describe('defaultParams', () => {
  it('describes the four-matmul attention head the app opens on', () => {
    const p = defaultParams()
    expect(p.name).toBe('out')
    expect(p.left.name).toBe('attn @ V')
    expect(p.left.left.name).toBe('attn')
    expect(p.left.left.left.name).toBe('Q')
    expect(p.right.name).toBe('wO')
  })

  it('marks every interior node as a matmul and leaves the root unmarked', () => {
    // Same invariant gpt2page relies on: the root is recognised by
    // `matmul === undefined`.
    const p = defaultParams()
    expect('matmul' in p).toBe(false)
    expect(p.left.matmul).toBe(true)
    expect(p.left.left.matmul).toBe(true)
    expect(p.left.left.left.matmul).toBe(true)
    expect(p.right.matmul).toBe(false)     // a leaf
  })

  it('agrees with the expression string it ships with', () => {
    const p = defaultParams()
    for (const name of ['out', 'attn', 'Q', 'K_t', 'V', 'wO', 'input', 'wQ']) {
      expect(p.expr).toContain(name)
    }
  })

  it('hands out a fresh tree each call', () => {
    // params is mutated in place throughout main.js and gui.js; a shared
    // literal would carry one session's edits into the next reset.
    const a = defaultParams(), b = defaultParams()
    expect(a).toEqual(b)
    expect(a).not.toBe(b)
    expect(a.deco).not.toBe(b.deco)
    a.deco.legends = 99
    expect(b.deco.legends).toBe(6)
  })

  it('carries every section gui.js builds a folder from', () => {
    const p = defaultParams()
    for (const k of ['expr', 'name', 'epilog', 'left', 'right', 'anim', 'block',
      'layout', 'deco', 'viz', 'diag', 'cam', 'folder', 'compress']) {
      expect(p).toHaveProperty(k)
    }
  })
})
