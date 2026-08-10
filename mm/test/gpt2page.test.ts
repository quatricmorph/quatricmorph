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
  dataClaim, productClaim, renderClaim, checkShapes, adoptRenderMode,
} from '../src/gpt2page.js'
import { flatten, unflatten } from '../src/util.js'

// Stand-ins for /api/specs.json entries.
const spec = (h, w, url = '/api/m.csv') => ({ h, w, url })

// …and for the fuller entries the status bar reads. `augment` is the server's
// descriptor of the row or column it appended to draw the bias.
const sp = (h, w, extra = {}) => ({ h, w, fidelity: 'exact', augment: null, ...extra })
const aug = (vector, axis, tensor = null) => ({ vector, axis, tensor })

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

  it('refuses a missing spec rather than emitting an undefined shape', () => {
    // The flags are part of the key. A view whose kinds say 'ln_1:w' but whose
    // build says m['ln_1'] gets undefined, and a leaf with h: undefined draws
    // as an empty matrix with nothing reported.
    expect(() => leaf('input|1', undefined)).toThrow(/is not in this view's kinds list/)
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

describe('checkShapes', () => {
  it('accepts a tree whose every matmul contracts over a shared extent', () => {
    expect(checkShapes(TREE())).toEqual({ h: 2, w: 5 })
  })

  it('rejects a left operand wider than its right operand is tall', () => {
    // The augmentation mistake this exists for: `[X | 1] @ W` instead of
    // `[X | 1] @ [W ; b]`. mm tiles the 768-row weight up to 769 with
    // `data[i % data.length]` and draws a picture nobody can tell is wrong.
    const t = root('qkv', leaf('ln_1|1', spec(6, 769)), leaf('c_attn', spec(768, 2304)))
    expect(() => checkShapes(t)).toThrow(/769 ≠ 768/)
  })

  it('names the node and both operands so the bad flag is findable', () => {
    const t = root('qkv', leaf('ln_1', spec(6, 768)), leaf('c_attn;b', spec(769, 2304)))
    expect(() => checkShapes(t)).toThrow(/'qkv'.*'ln_1' 6×768.*'c_attn;b' 769×2304/)
  })

  it('checks interior matmuls, not only the root', () => {
    const bad = inner('L', leaf('A', spec(2, 3)), leaf('B', spec(4, 5)))
    expect(() => checkShapes(root('R', bad, leaf('C', spec(5, 6))))).toThrow(/root\.left/)
  })
})

//
// The status bar makes two claims, and both of them are the sort that stays
// technically true while becoming misleading. These pin the ways they must not
// drift: "exact" standing alone over a synthetic column, or "complete" printed
// next to a known omission.
//
describe('dataClaim', () => {
  it('says exact when nothing is sampled and nothing is synthetic', () => {
    const c = dataClaim({ 'ln_1': sp(6, 768), 'wo': sp(64, 768) }, 1)
    expect(c).toContain('exact')
    expect(c).toContain("every element is the checkpoint's own")
    expect(c).not.toContain('synthetic')
  })

  it('never lets "exact" stand alone over a synthetic ones column', () => {
    // The ones column is the one number in an augmented leaf the model did not
    // supply. It is what carries the bias, so it belongs — but unqualified
    // "exact" over it is the claim this repo does not make.
    const c = dataClaim({ 'ln_1:w': sp(6, 769, { augment: aug('ones', 'col') }) }, 1)
    expect(c).toContain('exact')
    expect(c).toContain('synthetic all-ones column on ln_1')
    expect(c).toContain('the constant 1 the bias multiplies')
  })

  it('groups a matrix augmented both ways by axis', () => {
    // The attention head takes ln_1 as a left operand and its transpose as a
    // right one, so one kind carries a ones column *and* a ones row. Reading
    // the axis off the first entry would report both as whichever came first.
    const c = dataClaim({
      'ln_1:w': sp(6, 769, { augment: aug('ones', 'col') }),
      'ln_1:th': sp(769, 6, { augment: aug('ones', 'row') }),
    }, 1)
    expect(c).toContain('all-ones column on ln_1 and all-ones row on ln_1')
  })

  it('names the tensor the appended bias row actually holds', () => {
    const c = dataClaim({
      'wq:h': sp(769, 64, {
        augment: aug('bias', 'row', 'transformer.h.1.attn.c_attn.bias'),
      }),
    }, 1)
    expect(c).toContain('bias row on wq')
    expect(c).toContain('transformer.h.1.attn.c_attn.bias')
  })

  it('reads the strided axis off the flags, not the kind name', () => {
    // 'mlp_c_proj' contains an 'r'. Only the flags after the colon say which
    // axis was decimated, and calling a column-strided matrix row-strided would
    // describe the wrong half of the picture.
    const c = dataClaim({
      'mlp_c_proj:ch': sp(3073, 192, {
        fidelity: 'sampled',
        augment: aug('bias', 'row', 'transformer.h.0.mlp.c_proj.bias'),
      }),
    }, 4)
    expect(c).toContain('every 4th mlp_c_proj column')
    expect(c).toContain('contracted axes are never decimated')
    expect(c).toContain('strided with the output axis it indexes')
  })
})

describe('productClaim', () => {
  it('names the bias drawn and the augmentation that drew it', () => {
    const c = productClaim({ bias: 'c_attn.bias', gap: null })
    expect(c).toContain('includes c_attn.bias')
    expect(c).toContain('[X | 1] @ [W ; b]')
  })

  it('claims complete when there is no gap, whether or not there was a bias', () => {
    expect(productClaim({ bias: 'mlp.c_proj.bias', gap: null })).toContain('complete')
    // the logits view: GPT-2's tied LM head has no bias to draw
    const none = productClaim({ bias: null, gap: null })
    expect(none).toContain('this step has no bias term')
    expect(none).toContain('complete')
  })

  it('never claims complete beside a gap', () => {
    // The per-head views: c_attn.bias is drawn, attn.c_proj.bias cannot be —
    // GPT-2 adds it once to the sum over heads, so it is not a term of the
    // matmul drawn. Both facts have to survive into the same line.
    const c = productClaim({
      bias: 'c_attn.bias on Q, K and V',
      gap: 'attn.c_proj.bias on out — added once to the sum over all heads',
    })
    expect(c).toContain('includes c_attn.bias on Q, K and V')
    expect(c).toContain('attn.c_proj.bias on out')
    expect(c).not.toContain('complete')
  })

  it('escapes a gap that contains markup', () => {
    expect(productClaim({ bias: null, gap: '<b>x</b>' }))
      .toContain('&lt;b&gt;x&lt;/b&gt;')
  })
})

//
// The render mode is the one piece of state the page and the viewer both own:
// the header's `Render` selector and the viewer's own panel write it, and
// `refresh` pushes the selector's value back on every rebuild. So a change made
// inside the viewer has to come back, or it survives until the next layer
// change and is then silently undone -- a picture that quietly stops being the
// one that was asked for, which is the failure this whole file exists to catch.
//
describe('adoptRenderMode', () => {
  it('adopts a mode the viewer reports and the page does not have', () => {
    expect(adoptRenderMode('auto', 'spheres')).toBe('spheres')
    expect(adoptRenderMode('heatmap', 'spheres')).toBe('spheres')
    expect(adoptRenderMode('spheres', 'heatmap')).toBe('heatmap')
  })

  it('adopts auto, which no measurement of the built scene could recover', () => {
    // The reason `mode` is reported rather than inferred: a scene where auto
    // chose heatmap for every matrix is indistinguishable from an explicit
    // 'heatmap' in the summary, and auto is a different setting -- per matrix,
    // not for all of them.
    expect(adoptRenderMode('spheres', 'auto')).toBe('auto')
  })

  it('says nothing when there is nothing to change', () => {
    expect(adoptRenderMode('heatmap', 'heatmap')).toBe(null)
    expect(adoptRenderMode('auto', 'auto')).toBe(null)
  })

  it('says nothing when the viewer sent no mode at all', () => {
    // An older viewer, or the `{render: …}` summary before this field existed.
    // Leaving the selector alone keeps the page's own value authoritative.
    expect(adoptRenderMode('auto', undefined)).toBe(null)
    expect(adoptRenderMode('auto', null)).toBe(null)
    expect(adoptRenderMode('auto', '')).toBe(null)
  })

  it('refuses a mode the selector has no option for', () => {
    // Assigning an absent value to a <select> blanks it, and the next refresh
    // would push that empty string into params as the render mode.
    expect(adoptRenderMode('auto', 'elements', ['auto', 'spheres', 'heatmap'])).toBe(null)
    expect(adoptRenderMode('auto', 'spheres', ['auto', 'heatmap'])).toBe(null)
  })
})

describe('mount', () => {
  it('is the module entry point the example pages import', () => {
    // mount() itself needs a live /gpt2 server and is exercised by loading a
    // page, not by this suite. Everything it composes is tested above.
    expect(typeof mount).toBe('function')
  })
})

//
// Node kinds beyond the matmul, and the whole-model tree they build.
//
// Same class of failure as everything else in this file: a tree that is
// plausible but wrong. An add over two differently-sampled residual streams
// contracts nothing, so mm would draw it happily and tile the shorter operand;
// a stage list whose camera is computed from the old matmul-only bbox draws
// fine and frames nothing.
//
import { unary, add, stack } from '../src/gpt2page.js'

// A 64-token, 768-wide model at stride 16, shaped like the real one.
const S = 16
const M = {
  'ln_1:w': sp(64, 769), 'ln_1:th': sp(769, 64), 'wq:h': sp(769, 64),
  'wk_t:w': sp(64, 769), 'wv:h': sp(769, 64), 'resid:c': sp(64, 768 / S),
  'attn_out:w': sp(64, 769), 'attn_c_proj:ch': sp(769, 768 / S),
  'ln_2:w': sp(64, 769), 'c_fc:ch': sp(769, 3072 / S),
  'mlp_h:w': sp(64, 3073), 'mlp_c_proj:ch': sp(3073, 768 / S),
  'resid_mid:c': sp(64, 768 / S),
}
const headStage = (m = M) => inner('head',
  unary('softmax', 'softmax(tril(x))',
    inner('QKt',
      node('Q', 'ln_1|1', m['ln_1:w'], 'wQ;b', m['wq:h']),
      node('Kt', 'wK_t|b', m['wk_t:w'], 'ln_1t;1', m['ln_1:th']), 'x/sqrt(k)')),
  node('V', 'ln_1|1', m['ln_1:w'], 'wV;b', m['wv:h']))
const attnAdd = (m = M) => add('x + attn', leaf('resid', m['resid:c']),
  inner('heads @ c_proj', leaf('heads|1', m['attn_out:w']), leaf('c_proj;b', m['attn_c_proj:ch'])))
const mlpStage = (m = M) => unary('gelu(h)', 'gelu',
  inner('ln_2 @ c_fc', leaf('ln_2|1', m['ln_2:w']), leaf('c_fc;b', m['c_fc:ch'])))
const mlpAdd = (m = M) => add('x + mlp', leaf('resid_mid', m['resid_mid:c']),
  inner('h @ c_proj', leaf('gelu|1', m['mlp_h:w']), leaf('c_proj;b', m['mlp_c_proj:ch'])))
const modelTree = () => stack('distilgpt2', [0, 1].flatMap(l => [
  { ...headStage(), row: l }, { ...attnAdd(), row: l },
  { ...mlpStage(), row: l }, { ...mlpAdd(), row: l },
]))

describe('node builders', () => {
  it('marks each kind so viz.js can tell them apart', () => {
    expect(unary('s', 'gelu', leaf('x', sp(2, 3))).op).toBe('unary')
    expect(add('a', leaf('x', sp(2, 3)), leaf('y', sp(2, 3))).op).toBe('add')
    expect(stack('m', [leaf('x', sp(2, 3))]).op).toBe('stack')
  })

  it('keys stages rather than listing them, because copyTree cannot hold arrays', () => {
    // util.copyTree round trips through flatten/unflatten, whose own comment
    // says "no arrays". A stage array would silently lose the whole scene.
    const st = stack('m', [leaf('a', sp(1, 1)), leaf('b', sp(1, 1))])
    expect(Array.isArray(st.stages)).toBe(false)
    expect(Object.keys(st.stages)).toEqual(['s0', 's1'])
    // insertion order is forward-pass order, and it survives a round trip
    const back = unflatten(flatten(st))
    expect(Object.keys(back.stages)).toEqual(['s0', 's1'])
    expect(back.stages.s0.name).toBe('a')
  })
})

describe('checkShapes over the new node kinds', () => {
  it('accepts the whole-model tree the page actually builds', () => {
    expect(checkShapes(modelTree())).toEqual({ h: 64, w: 768 / S })
  })

  it('still refuses a mis-contracted matmul inside a stage', () => {
    // The original hazard, now one level deeper: mm tiles rather than fails, so
    // a stage list would hide a bad contraction behind 24 good ones.
    const bad = { ...M, 'wq:h': sp(768, 64) }     // 768, not the augmented 769
    expect(() => checkShapes(stack('m', [headStage(bad)])))
      .toThrow(/contracts .*769.*768|768 ≠ 769|769 ≠ 768/)
  })

  it('refuses an add whose operands are sampled differently', () => {
    // The live risk in a strided model view: the residual stream has to be
    // strided with the projection it is added to. Off by one stride step and
    // both operands are real matrices of the wrong columns.
    const bad = { ...M, 'resid:c': sp(64, 768 / 32) }
    expect(() => checkShapes(stack('m', [attnAdd(bad)])))
      .toThrow(/elementwise sum needs one shape/)
  })

  it('refuses an add whose operands disagree in height', () => {
    expect(() => checkShapes(add('a', leaf('x', sp(4, 8)), leaf('y', sp(5, 8)))))
      .toThrow(/4×8.*5×8/)
  })

  it('names the stage as well as the node when a stage is wrong', () => {
    const bad = { ...M, 'resid:c': sp(64, 1) }
    expect(() => checkShapes(stack('m', [headStage(), attnAdd(bad)])))
      .toThrow(/root\.s1 \('x \+ attn'\)/)
  })
})

describe('countPoints and bbox over the new node kinds', () => {
  it('counts a unary stage as its input plus the materialized result', () => {
    // 4x6 @ 6x5 = 24 + 30 + 20 = 74 drawn elements, then gelu adds its own 4x5.
    const u = unary('g', 'gelu', node('m', 'l', sp(4, 6), 'r', sp(6, 5)))
    expect(countPoints(u)).toEqual({ h: 4, w: 5, n: 74 + 20 })
  })

  it('counts an add as both operands plus the sum', () => {
    const a = add('a', leaf('x', sp(4, 5)), leaf('y', sp(4, 5)))
    expect(countPoints(a)).toEqual({ h: 4, w: 5, n: 20 + 20 + 20 })
  })

  it('sums a stack over its stages and reports the last one\'s shape', () => {
    const st = stack('m', [
      leaf('a', sp(2, 3)),                                   // 6
      add('b', leaf('x', sp(4, 5)), leaf('y', sp(4, 5))),    // 60
    ])
    expect(countPoints(st)).toEqual({ h: 4, w: 5, n: 66 })
  })

  it('measures the whole-model scene as rows of stages, not as one matmul', () => {
    // A wrong camera here draws fine and frames nothing, so the bbox has to
    // follow Stack.layoutStages: stages left to right within a row, rows down.
    const bb = bbox(modelTree())
    const one = bbox({ ...headStage(), row: 0 })
    expect(bb.h).toBeGreaterThan(one.h)            // two rows, so taller
    expect(bb.w).toBeGreaterThan(one.w)            // four stages wide
    expect(bb.d).toBeGreaterThan(0)
  })

  it('transposes the row/column composition when the layers switch to horizontal', () => {
    // The page's Layers switch writes layout['row flow'], and this bbox is what
    // sizes the camera for it — so it has to follow Stack.layoutStages through
    // the transpose. Two rows of known leaves, gap 0 so the margins vanish:
    //
    //   vertical    row 0 = 2×3 beside 4×5 → h 4, w 8;  row 1 = 6×7
    //               rows sum in h, widest row wins w   → h 10, w 8
    //   horizontal  row 0 = 2×3 above 4×5  → h 6, w 5;  row 1 = 6×7
    //               rows sum in w, tallest row wins h  → h 6,  w 12
    const two = () => stack('m', [
      { ...leaf('a', sp(2, 3)), row: 0 }, { ...leaf('b', sp(4, 5)), row: 0 },
      { ...leaf('c', sp(6, 7)), row: 1 },
    ])
    const flowed = flow => bbox({ ...two(), layout: { gap: 0, ...(flow ? { 'row flow': flow } : {}) } })
    expect(flowed('vertical')).toEqual({ h: 10, w: 8, d: 0 })
    expect(flowed('horizontal')).toEqual({ h: 6, w: 12, d: 0 })
    // an absent flow is the vertical arrangement, so a scene built before this
    // existed — the whole model tree included — is sized exactly as before
    expect(flowed(undefined)).toEqual(flowed('vertical'))
    expect(bbox({ ...modelTree(), layout: { gap: 24 } }))
      .toEqual(bbox({ ...modelTree(), layout: { gap: 24, 'row flow': 'vertical' } }))
  })

  it('sizes the six-layer model differently in each arrangement, and finitely in both', () => {
    // The camera distance mount() derives is (h + w + d) / 2, and it has to be
    // sane in both: a switch that framed nothing would look like a broken layout.
    const six = () => stack('distilgpt2', [0, 1, 2, 3, 4, 5].flatMap(l => [
      { ...headStage(), row: l }, { ...attnAdd(), row: l },
      { ...mlpStage(), row: l }, { ...mlpAdd(), row: l },
    ]))
    const d = flow => {
      const b = bbox({ ...six(), layout: { gap: 24, 'row flow': flow } })
      expect(Number.isFinite(b.h + b.w + b.d)).toBe(true)
      return b
    }
    const v = d('vertical'), h = d('horizontal')
    expect(h.d).toBe(v.d)                       // depth is untouched by the flow
    expect(h.w).toBeGreaterThan(v.w)            // six layers now advance across
    expect(h.h).toBeLessThan(v.h)               // and no longer stack up
  })

  it('gives the model scene a camera distance that is not the old matmul rule', () => {
    // The old bbox would have thrown on `p.left` of a stack; this pins that the
    // camera rule in mount() gets a finite, sane distance out of the new shape.
    const bb = bbox(modelTree())
    const d = Math.round((bb.h + bb.w + bb.d) / 2)
    expect(Number.isFinite(d)).toBe(true)
    expect(d).toBeGreaterThan(100)
  })
})

describe('renderClaim', () => {
  // The third claim. `data:` says what the numbers are, `product:` says what
  // the matmul computes, and this says what the renderer did to them on the way
  // to pixels — because a heatmap at LOD 2 is showing one maxAbs per 4x4 block
  // and "exact" over the leaf data would be true about the wrong thing.
  const sum = (o = {}) => ({
    absmin: 0, absmax: 1, mats: 10, encoding: 'magnitude', reducer: 'maxAbs',
    lod: 1, texels: 1000, elements: 0, heatmaps: 10, ...o,
  })

  it('says nothing at all before the viewer has reported back', () => {
    expect(renderClaim(null)).toBe('')
  })

  it('calls the sphere path exact, because one element is one element', () => {
    const c = renderClaim(sum({ heatmaps: 0, texels: 0, encoding: null, elements: 4096 }))
    expect(c).toContain('exact')
    expect(c).toContain('sphere')
    expect(c).not.toContain('LOD')
  })

  it('prints what forcing spheres costs, rather than preventing it', () => {
    // The Render control takes an override at its word, so the bar has to say
    // what the override bought: one instanced quad per element, four vertices
    // each. Below the threshold it is stated plainly, above it, marked.
    const small = renderClaim(sum({ heatmaps: 0, texels: 0, encoding: null, elements: 4096 }))
    expect(small).toContain('4,096 elements as spheres')
    expect(small).not.toContain('slow frame')

    const big = renderClaim(sum({ heatmaps: 0, texels: 0, encoding: null, elements: 6_122_373 }))
    expect(big).toContain('6,122,373 elements as spheres')
    expect(big).toContain('class="sampled"')
    expect(big).toContain('slow frame')
  })

  it('reports both paths when a scene is mixed, which auto makes it', () => {
    const c = renderClaim(sum({ heatmaps: 147, mats: 159, elements: 250_000 }))
    expect(c).toContain('147/159 as heatmap')
    expect(c).toContain('250,000 elements as spheres')
  })

  it('never lets a reduced heatmap print as exact', () => {
    const c = renderClaim(sum({ lod: 4 }))
    expect(c).toContain('class="sampled"')
    expect(c).toContain('LOD 2')                  // log2(4)
    expect(c).toContain('4×4')
    expect(c).toContain('maxAbs')
    expect(c).not.toContain('class="exact"')
  })

  it('says LOD 0 is exact-per-element, and says the quantization out loud', () => {
    const c = renderClaim(sum())
    expect(c).toContain('class="exact"')
    expect(c).toContain('one texel per element')
    expect(c).toContain('8 bits')                 // display only, never the readout
  })

  it('names the encoding, because |x| and signed are different pictures', () => {
    expect(renderClaim(sum())).toContain('magnitude |x|')
    expect(renderClaim(sum({ encoding: 'signed' }))).toContain('signed (hue by sign)')
    expect(renderClaim(sum({ encoding: 'mixed' }))).toContain('mixed encodings')
  })

  it('promises the hover readout is still the checkpoint\'s own number', () => {
    expect(renderClaim(sum({ lod: 2 }))).toContain("checkpoint's own value")
  })
})
