//
// The precondition every other spec in this directory depends on.
//
// Three of the four src modules do real work at *import* time, before any test
// calls anything: util builds a `NodeMaterial` and parses a Three.js typeface
// from assets/, points builds another NodeMaterial plus its TSL `Fn()` shader
// graphs, and viz imports points. If constructing those headless throws, every
// downstream spec fails with the same unrelated stack and the actual assertion
// is never reached.
//
// So this file asserts almost nothing. Its job is to make that failure mode
// legible: if this spec is red, the problem is the test environment (the
// three/webgpu alias, jsdom, a missing WebGPU global) and not the module under
// test. If it is green, importing anything in src/ headless is known-safe.
//
import { describe, it, expect } from 'vitest'

describe('headless import of every src module', () => {
  it('imports util without touching a GPU', async () => {
    const util = await import('../src/util.js')
    expect(typeof util.flatten).toBe('function')
  })

  it('imports points, whose NodeMaterial and TSL graphs build at module scope', async () => {
    const points = await import('../src/points.js')
    expect(points.MATERIAL).toBeDefined()
  })

  it('imports viz, which pulls points in with it', async () => {
    const viz = await import('../src/viz.js')
    expect(Array.isArray(viz.INITS)).toBe(true)
  })

  it('imports colormap and heatmap, which are deliberately THREE-free', async () => {
    // These two are pure arithmetic so a jsdom test can reach every decision
    // the heatmap shader acts on. If either ever grows a THREE import, the
    // page bundle grows with it and this stops being the reason they exist.
    const colormap = await import('../src/colormap.js')
    const heatmap = await import('../src/heatmap.js')
    expect(colormap.COLORMAP_STOPS).toHaveLength(7)
    expect(typeof heatmap.chooseLodFactor).toBe('function')
  })

  it('imports heatmapmesh, whose per-block TSL graphs build on construction', async () => {
    const { HeatmapMesh } = await import('../src/heatmapmesh.js')
    const info = { i: { n: 1, size: 2, max: 2 }, j: { n: 1, size: 2, max: 2 }, gap: 0 }
    expect(new HeatmapMesh(2, 2, info, { lod: 1 }).blocks).toHaveLength(1)
  })

  it('imports gpt2page, whose CSS import Vite has to resolve', async () => {
    const page = await import('../src/gpt2page.js')
    expect(typeof page.mount).toBe('function')
  })

  it('imports the editor primitives, which are deliberately THREE-free', async () => {
    // address/scenetree/selection are the logical layer of the tensor editor:
    // pure index arithmetic and state. If any of them ever grows a THREE
    // import it stops being importable in the leanest contexts, and this
    // stops being the reason the layering exists. (scenetree reaches only
    // into heatmap.js, itself pinned THREE-free above.)
    const address = await import('../src/address.js')
    const scenetree = await import('../src/scenetree.js')
    const selection = await import('../src/selection.js')
    expect(address.LEVELS).toContain('scalar')
    expect(typeof scenetree.SceneTree).toBe('function')
    expect(typeof selection.SelectionManager).toBe('function')
  })

  it('imports the editor render/controller modules, whose shared materials build at module scope', async () => {
    // highlight builds its shared Line/MeshBasicMaterials and unit geometries
    // at import; picking/cameractl/interaction pull THREE in. Same contract
    // as points above: constructing these headless must not need a GPU.
    const picking = await import('../src/picking.js')
    const highlight = await import('../src/highlight.js')
    const cameractl = await import('../src/cameractl.js')
    const editops = await import('../src/editops.js')
    const interaction = await import('../src/interaction.js')
    expect(typeof picking.Picker).toBe('function')
    expect(typeof highlight.HighlightRenderer).toBe('function')
    expect(typeof cameractl.CameraRig).toBe('function')
    expect(typeof editops.EditStack).toBe('function')
    expect(typeof interaction.createEditor).toBe('function')
  })

  it('imports the editor panels, which are DOM-only by design', async () => {
    const inspector = await import('../src/inspector.js')
    const outliner = await import('../src/outliner.js')
    expect(typeof inspector.createInspector).toBe('function')
    expect(typeof outliner.createOutliner).toBe('function')
  })
})
