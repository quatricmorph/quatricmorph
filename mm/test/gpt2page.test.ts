//
// gpt2page.js — the params-tree builder shared by the three checkpoint pages.
//
// Everything here guards against the same class of failure: a tree that is
// *plausible but wrong*. mm does not validate what it is handed. A leaf whose
// declared shape disagrees with its CSV is tiled (`data[i % data.length]`), a
// root that carries `matmul: true` silently loses its element counts, and an
// anim preset addressing a path the tree lacks would create a phantom node and
// just not animate. None of those throw; all of them draw a picture.
//
// The expected values below are hand-computed from the recurrences in the
// source, not captured from a run.
//
import { describe, it, expect } from 'vitest'
import {
  abs, esc, L, A, B, leaf, inner, node, root, BASE,
  countPoints, bbox, height, width, merge, mount,
} from '../src/gpt2page.js'

// Stand-ins for /api/specs.json entries.
const spec = (h, w, url = '/api/m.csv') => ({ h, w, url })

describe('abs', () => {
  it('makes a root-relative server URL absolute on the page origin', () => {
    // Cross-origin is the failure this prevents: the CSVs must be fetched from
    // the page's own origin, whether that is vite's proxy or gpt2_server.py.
    const u = abs('/api/matrix.csv?kind=wq')
    expect(u.startsWith(location.origin)).toBe(true)
    expect(u.endsWith('/api/matrix.csv?kind=wq')).toBe(true)
  })
})

describe('esc', () => {
  it('escapes the three characters that could break out of the status bar', () => {
    expect(esc('<script> & "x"')).toBe('&lt;script&gt; &amp; "x"')
  })

  it('coerces non-strings rather than throwing', () => {
    expect(esc(42)).toBe('42')
  })
})

describe('layout and anim shorthands', () => {
  it('L names the four placement keys mm expects', () => {
    expect(L('positive', 'left', 'bottom', 'back')).toEqual({
      'polarity': 'positive', 'left placement': 'left',
      'right placement': 'bottom', 'result placement': 'back',
    })
  })

  it('A defaults to no animation', () => {
    expect(A()).toEqual({ alg: 'none' })
    expect(A('vmprod')).toEqual({ alg: 'vmprod' })
  })

  it('B sets a single j block', () => {
    expect(B()).toEqual({ 'j blocks': 1 })
  })
})

describe('leaf', () => {
  it('takes h and w from the spec and never from a literal', () => {
    const l = leaf('wQ', spec(768, 64, '/api/m.csv?kind=wq'))
    expect(l.h).toBe(768)
    expect(l.w).toBe(64)
    expect(l.name).toBe('wQ')
    expect(l.matmul).toBe(false)
    expect(l.init).toBe('url')
    expect(l.url).toBe(abs('/api/m.csv?kind=wq'))
  })

  it('spans the full [-1, 1] range with no dropout', () => {
    // A nonzero dropout would blank real checkpoint elements at random, which
    // would look like sparsity in the weights.
    const l = leaf('wQ', spec(4, 4))
    expect(l.min).toBe(-1)
    expect(l.max).toBe(1)
    expect(l.dropout).toBe(0)
  })
})

describe('inner and root', () => {
  it('inner marks itself as a matmul so viz.js recurses into it', () => {
    const i = inner('attn', leaf('Q', spec(2, 3)), leaf('K', spec(3, 4)))
    expect(i.matmul).toBe(true)
    expect(i.epilog).toBe('none')
    expect(i.anim).toEqual({ alg: 'none' })
    expect(i.block).toEqual({ 'j blocks': 1 })
  })

  it('root does NOT carry a matmul key', () => {
    // The invariant the source comment calls out: ensureChildCounts recognises
    // the root by `matmul === undefined` and only then propagates `total`. A
    // root with matmul:true leaves `total` unset through the entire tree.
    const r = root('out', leaf('a', spec(2, 3)), leaf('b', spec(3, 4)))
    expect('matmul' in r).toBe(false)
    expect(r.epilog).toBe('none')
    expect(r.left.name).toBe('a')
    expect(r.right.name).toBe('b')
  })

  it('node builds an interior matmul over two leaves', () => {
    const n = node('Q', 'input', spec(6, 768), 'wQ', spec(768, 64), 'none',
      L('positive', 'left', 'bottom', 'back'))
    expect(n.matmul).toBe(true)
    expect(n.left).toMatchObject({ name: 'input', h: 6, w: 768, matmul: false })
    expect(n.right).toMatchObject({ name: 'wQ', h: 768, w: 64, matmul: false })
    expect(n.layout.polarity).toBe('positive')
  })
})

describe('BASE', () => {
  it('returns a fresh tree each call', () => {
    // mount() merges anim presets into the result in place. A shared BASE would
    // let one page load contaminate the next.
    const a = BASE(), b = BASE()
    expect(a).not.toBe(b)
    expect(a.anim).not.toBe(b.anim)
    a.anim.speed = 999
    expect(b.anim.speed).toBe(16)
  })

  it('starts unanimated and unblocked', () => {
    const p = BASE()
    expect(p.anim.alg).toBe('none')
    expect(p.block).toEqual({ 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 })
  })
})

//
// The tree used for the counting assertions below:
//
//        R = (A @ B) @ C
//       /            \
//   L = A @ B         C 4x5
//    /      \
//  A 2x3   B 3x4
//
const A_ = () => leaf('A', spec(2, 3))
const B_ = () => leaf('B', spec(3, 4))
const C_ = () => leaf('C', spec(4, 5))
const TREE = () => root('R', inner('L', A_(), B_()), C_())

describe('countPoints', () => {
  it('counts a bare leaf as h*w', () => {
    expect(countPoints(A_())).toEqual({ h: 2, w: 3, n: 6 })
  })

  it('counts one matmul as both inputs plus the result', () => {
    // 2x3 + 3x4 + 2x4 = 6 + 12 + 8 = 26
    expect(countPoints(inner('L', A_(), B_()))).toEqual({ h: 2, w: 4, n: 26 })
  })

  it('counts intermediates, not just leaves', () => {
    // 26 (the subtree) + 4x5 = 20 + result 2x5 = 10  ->  56
    // A leaf-only sum would say 6 + 12 + 20 = 38 and badly understate the scene.
    expect(countPoints(TREE())).toEqual({ h: 2, w: 5, n: 56 })
  })
})

describe('bbox / height / width', () => {
  it('reads a leaf shape straight off the node', () => {
    expect(height(A_())).toBe(2)
    expect(width(A_())).toBe(3)
    expect(bbox(A_())).toEqual({ h: 2, w: 3, d: 0 })
  })

  it('takes height down the left spine and width down the right spine', () => {
    const t = TREE()
    expect(height(t)).toBe(2)   // via L, via A
    expect(width(t)).toBe(5)    // via C
  })

  it('accumulates depth as nesting, not as the largest single matrix', () => {
    // bbox(L) = { h: height(A)=2, w: width(B)=4, d: width(A)=3 }
    // bbox(R): child = bbox(L).d = 3 (C is a leaf, contributes 0)
    //          d = width(L) + child = width(B) + 3 = 4 + 3 = 7
    expect(bbox(inner('L', A_(), B_()))).toEqual({ h: 2, w: 4, d: 3 })
    expect(bbox(TREE())).toEqual({ h: 2, w: 5, d: 7 })
  })

  it('gives a deeper tree a bigger box than its biggest matrix', () => {
    // This is the property the camera distance depends on: main.js frames the
    // whole scene, and a deep tree is far larger than any matrix in it.
    const b = bbox(TREE())
    expect(b.h + b.w + b.d).toBeGreaterThan(5)
  })
})

describe('merge', () => {
  it('deep-merges a preset into a params tree', () => {
    const dst = { anim: { alg: 'none', speed: 16 }, left: { anim: { alg: 'none' } } }
    merge(dst, { anim: { alg: 'vmprod' }, left: { anim: { alg: 'mvprod' } } })
    expect(dst.anim).toEqual({ alg: 'vmprod', speed: 16 })   // speed survives
    expect(dst.left.anim.alg).toBe('mvprod')
  })

  it('creates a missing leaf value', () => {
    const dst: any = { anim: { alg: 'none' } }
    merge(dst, { anim: { fuse: 'sync' } })
    expect(dst.anim.fuse).toBe('sync')
  })

  it('refuses to create a subtree the params tree does not have', () => {
    // The regression this exists for is real: attnqkov once carried attngpt2's
    // presets, whose `left.left.left` is a node that tree does not have. Without
    // the throw, params grows a phantom {anim:{…}} node, the animation silently
    // does not happen, and nothing reports it.
    const dst = { anim: { alg: 'none' } }
    expect(() => merge(dst, { left: { anim: { alg: 'vmprod' } } }))
      .toThrow(/addresses 'left'/)
  })

  it('names the full path of the node it could not find', () => {
    const dst = { left: { anim: { alg: 'none' } } }
    expect(() => merge(dst, { left: { left: { anim: { alg: 'vmprod' } } } }))
      .toThrow(/addresses 'left\.left'/)
  })

  it('assigns arrays rather than recursing into them', () => {
    const dst = { kinds: ['a'] }
    merge(dst, { kinds: ['b', 'c'] })
    expect(dst.kinds).toEqual(['b', 'c'])
  })

  it('is what makes a preset applicable twice without drift', () => {
    // mount() copies the preset before merging because merge assigns
    // sub-objects by reference. Applying the same preset object to two trees
    // must leave the second unaffected by edits to the first.
    const preset = { anim: { alg: 'vmprod' } }
    const a = { anim: { alg: 'none' } }
    const b = { anim: { alg: 'none' } }
    merge(a, JSON.parse(JSON.stringify(preset)))
    merge(b, JSON.parse(JSON.stringify(preset)))
    a.anim.alg = 'mutated'
    expect(b.anim.alg).toBe('vmprod')
    expect(preset.anim.alg).toBe('vmprod')
  })
})

describe('mount', () => {
  it('is the module entry point the example pages import', () => {
    // mount() itself needs a live /gpt2 server and is exercised by loading a
    // page, not by this suite. Everything it composes is tested above.
    expect(typeof mount).toBe('function')
  })
})
