//
// util.js — the params serialization layer.
//
// This is the module with the most to lose from a well-meaning refactor. The
// whole app state travels through flatten -> compress -> URL -> uncompress ->
// unflatten on every camera move, and every step is hand-rolled. A break here
// does not throw; it produces a URL that loads a *slightly different* scene, or
// silently turns a boolean into the string "false" (which is truthy).
//
// So the assertions below are weighted toward round-trip identities rather than
// line coverage: `unflatten(flatten(x)) === x` holds for any implementation that
// is correct and for almost none that is subtly wrong. The literal expectations
// that do appear (the compress key scheme) were derived by hand from the source,
// not captured from a run.
//
import { describe, it, expect } from 'vitest'
import * as util from '../src/util.js'

// Shaped like the real thing: nested sub-objects, mixed value types, the
// dotted-key hazard of duplicate leaf names at different depths.
const PARAMS = () => ({
  name: 'out',
  epilog: 'x/sqrt(k)',
  anim: { alg: 'none', speed: 16, 'hide inputs': false, spin: 0 },
  layout: { scheme: 'blocks', gap: 24, polarity: 'negative' },
  left: {
    epilog: 'none',
    anim: { alg: 'inherit', speed: 8, 'hide inputs': true, spin: 1 },
  },
  cam: { x: -400, y: 400, z: 400 },
  compress: true,
})

describe('flatten / unflatten', () => {
  it('flattens nested objects to dotted paths', () => {
    expect(util.flatten({ a: { b: 1 }, c: 2 })).toEqual({ 'a.b': 1, c: 2 })
  })

  it('flattens to depth, not just one level', () => {
    expect(util.flatten({ a: { b: { c: 3 } } })).toEqual({ 'a.b.c': 3 })
  })

  it('unflatten is the left inverse of flatten', () => {
    const p = PARAMS()
    expect(util.unflatten(util.flatten(p))).toEqual(p)
  })

  it('splits each dotted key at its FIRST dot, so deep paths survive', () => {
    // 'a.b.c' must become a.b.c, not {'a': {'b.c': …}} — getting this backwards
    // still round-trips through flatten, so only a depth-3 case catches it.
    expect(util.unflatten({ 'a.b.c': 3 })).toEqual({ a: { b: { c: 3 } } })
  })

  it('preserves value types — numbers stay numbers, booleans stay booleans', () => {
    const r: any = util.unflatten(util.flatten({ n: 1, s: 'x', b: false }))
    expect(typeof r.n).toBe('number')
    expect(typeof r.b).toBe('boolean')
    expect(r.b).toBe(false)
  })
})

describe('compress / uncompress', () => {
  // Hand-derived from the source. compress() interns every path segment as a
  // number, emits values under the interned path, then appends the dictionary.
  // Interning order is first-seen: a=0, b=1, c=2.
  it('interns path segments and appends the dictionary', () => {
    expect(util.compress({ 'a.b': 1, c: 2 }))
      .toEqual({ '0.1': 1, 2: 2, a: '0', b: '1', c: '2' })
  })

  it('uncompress inverts compress on a flattened tree', () => {
    const flat = util.flatten(PARAMS())
    expect(util.uncompress(util.compress(flat))).toEqual(flat)
  })

  it('reuses one dictionary entry for a segment repeated at different depths', () => {
    // 'anim' and 'speed' each appear twice in PARAMS (top level and under left).
    // The dictionary is keyed by segment, so each is interned exactly once —
    // that reuse is the entire point of the scheme, and a rewrite that interned
    // whole paths would still round-trip while making URLs much longer.
    const comp = util.compress(util.flatten(PARAMS()))
    const dict = Object.keys(comp).filter(k => +k != (k as any))
    expect(dict.filter(k => k === 'anim')).toHaveLength(1)
    expect(new Set(dict).size).toBe(dict.length)
  })

  it('distinguishes dictionary keys from value keys by numeric-ness', () => {
    // uncompress() classifies with `+k == k`. A segment literally named "0"
    // would be misread as a value slot; nothing in params is, but the
    // classification rule is load-bearing and worth stating.
    const comp: any = util.compress({ 'a.b': 7 })
    expect(comp['0.1']).toBe(7)
    expect(comp.a).toBe('0')
  })
})

describe('copyTree', () => {
  it('deep-copies, sharing no sub-object with the original', () => {
    const p = PARAMS()
    const c = util.copyTree(p)
    expect(c).toEqual(p)
    expect(c.anim).not.toBe(p.anim)
    c.anim.speed = 999
    expect(p.anim.speed).toBe(16)
  })
})

describe('search params round trip', () => {
  it('carries an uncompressed tree through as JSON', () => {
    const p = { ...PARAMS(), compress: false }
    const sp = util.makeSearchParams(p)
    expect(sp.get('params')).toBe(JSON.stringify(p))

    const obj = PARAMS()
    util.updateObjectFromSearchParams(obj, sp)
    expect(obj).toEqual(p)
    expect(obj.compress).toBe(false)
  })

  it('carries a compressed tree through and restores every value type', () => {
    // The compressed path is the one that loses types: URLSearchParams values
    // are all strings, and castToType has to recover number/boolean/string from
    // the *target* object's existing types.
    const p = PARAMS()
    const sp = util.makeSearchParams(p)
    expect(sp.get('params')).toBe(null)      // not the JSON form

    const obj = PARAMS()
    obj.anim.speed = 1
    obj.anim['hide inputs'] = true
    obj.cam.x = 0
    obj.name = 'other'

    util.updateObjectFromSearchParams(obj, sp)
    expect(obj).toEqual(p)
    expect(typeof obj.anim.speed).toBe('number')
    expect(obj.anim['hide inputs']).toBe(false)   // boolean false, not "false"
    expect(typeof obj.cam.x).toBe('number')
    expect(obj.cam.x).toBe(-400)
  })

  it('leaves the object untouched when there are no search params', () => {
    // Guard on a real regression shape: an empty query must not switch
    // compression on, which would rewrite the URL of a plain visit.
    const obj = { ...PARAMS(), compress: false }
    util.updateObjectFromSearchParams(obj, new URLSearchParams(''))
    expect(obj.compress).toBe(false)
    expect(obj).toEqual({ ...PARAMS(), compress: false })
  })

  it('survives malformed JSON in ?params= without throwing', () => {
    const obj = PARAMS()
    util.updateObjectFromSearchParams(obj, new URLSearchParams('params=not-json'))
    expect(obj).toEqual(PARAMS())
  })
})

describe('object helpers', () => {
  it('updateProps assigns top level only, replacing sub-objects wholesale', () => {
    const o = { a: { x: 1, y: 2 }, b: 1 }
    util.updateProps(o, { a: { x: 9 } })
    expect(o).toEqual({ a: { x: 9 }, b: 1 })      // y is gone
  })

  it('updatePropsRec merges into sub-objects instead of replacing them', () => {
    const o = { a: { x: 1, y: 2 }, b: 1 }
    util.updatePropsRec(o, { a: { x: 9 } })
    expect(o).toEqual({ a: { x: 9, y: 2 }, b: 1 })  // y survives
  })

  it('deleteProps removes exactly the named keys and returns the object', () => {
    const o = { a: 1, b: 2, c: 3 }
    expect(util.deleteProps(o, ['a', 'c'])).toBe(o)
    expect(o).toEqual({ b: 2 })
  })

  it('syncProp reads on undefined and writes otherwise', () => {
    const o: any = { a: 1 }
    expect(util.syncProp(o, 'a')).toBe(1)
    expect(util.syncProp(o, 'a', 5)).toBe(5)
    expect(o.a).toBe(5)
    // 0 and false are writes, not reads — a truthiness check here would be a bug
    expect(util.syncProp(o, 'a', 0)).toBe(0)
    expect(o.a).toBe(0)
  })
})

describe('geometry helpers', () => {
  it('bbhwd measures a box as height/width/depth', () => {
    const bb = { min: { x: -1, y: -2, z: -3 }, max: { x: 1, y: 2, z: 3 } }
    expect(util.bbhwd(bb)).toEqual({ h: 4, w: 2, d: 6 })
  })

  it('center halves the span', () => {
    expect(util.center(10)).toBe(5)
    expect(util.center(10, 4)).toBe(3)
  })
})

describe('three.js constructions', () => {
  it('builds three axis lines from the origin', () => {
    const g = util.axes()
    expect(g.children).toHaveLength(3)
  })

  it('builds a row guide as a group of line segments', () => {
    const g = util.rowGuide(8, 8)
    expect(g.children.length).toBeGreaterThan(0)
    expect(g.children.every((c: any) => c.isLine)).toBe(true)
  })

  it('builds flow guide arrows as two coloured triangles', () => {
    const layout = {
      left: 1, right: 1, result: 1, gap: 2, left_scatter: 0, right_scatter: 0,
    }
    const g = util.flowGuide(4, 4, 4, layout)
    expect(g.children).toHaveLength(2)
    for (const mesh of g.children as any[]) {
      // 3 vertices, and the colour attribute the WebGPU NodeMaterial reads
      expect(mesh.geometry.attributes.position.count).toBe(3)
      expect(mesh.geometry.attributes.color.normalized).toBe(true)
    }
  })

  it('renders text to a mesh with real geometry', () => {
    // Also pins that the KaTeX typeface in assets/ parsed at import time.
    const mesh = util.getText('mm')
    expect(mesh.geometry.attributes.position.count).toBeGreaterThan(0)
  })

  it('disposeAndClear empties a group and disposes its geometry', () => {
    const g = util.axes()
    util.disposeAndClear(g)
    expect(g.children).toHaveLength(0)
  })
})
