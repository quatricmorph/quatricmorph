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

  it('imports gpt2page, whose CSS import Vite has to resolve', async () => {
    const page = await import('../src/gpt2page.js')
    expect(typeof page.mount).toBe('function')
  })
})
