//
// Shared driver for the checkpoint-backed example pages.
//
// The home page (`index.html`), `attngpt2/` and `attnqkov/` all do the same
// thing: ask `tools/gpt2_server.py` (under /api/) for the shape and URL of every matrix in a
// tree, hand that tree to the unmodified mm viewer in an iframe, and state
// exactly what the picture is and is not. Only the *tree* differs between them,
// so only the tree lives in the page; everything else is here.
//
// Those statements used to have a status bar of their own above the frame. They
// do not any more — the frame has that height — and where they go now is the
// note above `fail`/`claims`.
//
// The rule this module exists to state once instead of three times:
//
//   **Every leaf's h/w comes from /api/specs.json — never a literal.**
//
// mm's `tryURLInit` wraps out-of-range indices (`data[i % data.length]`), so a
// leaf whose declared shape disagrees with its CSV is silently *tiled* rather
// than rejected. A hand-written `h: 256` next to a server that emits one row
// per token produces a plausible, wrong picture with no error anywhere. The
// server refuses to emit a matrix that disagrees with the spec it published
// (`route_matrix`), and this module refuses to write a shape the server did not
// give it.
//
// The pages drive mm through its public message surface. That surface has
// grown, and this comment used to say the viewer was untouched by any of it:
//
//   `?params=` on the iframe URL          — unchanged, and still how every
//                                           per-layer view is loaded
//   postMessage({setParams})              — now also takes `replace: true`,
//                                           which swaps the whole params object
//                                           instead of merging onto it. A tree
//                                           of a different *shape* leaves stale
//                                           nodes behind under a merge.
//   postMessage({setStage})               — new. Moves the active stage of a
//                                           staged model scene without
//                                           rebuilding it. A scrubbing timeline
//                                           cannot be driven a reload at a time,
//                                           which is what the view selector does.
//   viewer -> page {stages: {...}}        — new. The stage list, the active
//                                           index, and what the renderer
//                                           actually built (texels, LOD level,
//                                           colour encoding), so the chrome and
//                                           the claims describe the picture on
//                                           screen rather than the one this file
//                                           asked for.
//
// So `viz.ts`, `util.ts`, `gui.ts` and `main.ts` are no longer untouched by the
// checkpoint pages: the node kinds a staged forward pass needs (a materialized
// unary stage, a residual add, an ordered stack of stages) live in viz.ts, and
// main.ts answers the two messages above. The page/viewer split is intact --
// the page still owns the chrome, the fetches and the claims, and the viewer
// still owns the scene -- but the protocol between them is wider than it was.
//

import './gpt2page.css'
import { RENDER_MODES } from './render/heatmap.js'

// Typed `any` deliberately. Everything this module looks up is an <input>, a
// <select> or the <iframe>, and it reads .value / .options / .disabled / .src
// off them -- but getElementById is only ever HTMLElement, so the alternative
// is a cast at all ~30 call sites. The chrome these ids belong to is injected
// by this same file (see CHROME), so the shapes are not in question.
const $ = (id: string): any => document.getElementById(id)

// Absolute, same-origin. The CSV URLs specs.json hands back are root-relative
// (`/api/matrix.csv?…`), which works both under `vite dev` — vite.config.ts
// proxies /api to the python server — and under gpt2_server.py serving mm/
// directly. A cross-origin :8000 fetch from a :5173 page would be blocked.
export const abs = u => new URL(u, location.href).href

// ---------------------------------------------------------------------------
// mm params helpers
// ---------------------------------------------------------------------------

export const L = (pol, left, right, res) => ({
  'polarity': pol, 'left placement': left,
  'right placement': right, 'result placement': res,
})
export const A = (alg = undefined) => ({ alg: alg || 'none' })
export const B = () => ({ 'j blocks': 1 })

// The only place a leaf's h/w is written. `spec` is a specs.json entry.
//
// The throw is for the other half of the same mistake `checkShapes` catches:
// a view that asks for `ln_1:w` in its `kinds` but looks the spec up as
// `m['ln_1']` gets `undefined`, and without this a leaf would go out with
// `h: undefined` — which mm renders as an empty matrix, not an error.
export const leaf = (name, spec) => {
  if (!spec) {
    throw new Error(
      `leaf '${name}' was given no spec: the key it was looked up by is not in ` +
      `this view's kinds list (flags are part of the key — 'ln_1' and 'ln_1:w' ` +
      `are different entries)`)
  }
  return {
    name, matmul: false, h: spec.h, w: spec.w,
    init: 'url', url: abs(spec.url), min: -1, max: 1, dropout: 0,
  }
}

// An interior matmul over two already-built subtrees. `matmul: true` is what
// tells viz.js this node has children; note that the *root* must not carry it —
// `ensureChildCounts` uses `matmul === undefined` to recognise the root and
// propagate `total`, so a root with `matmul: true` leaves `total` unset
// throughout the tree.
export const inner = (name, l, r, epilog = undefined, layout = undefined) => ({
  name, matmul: true, epilog: epilog || 'none',
  left: l, right: r,
  anim: A(), block: B(), layout,
})

// An interior matmul over two leaves.
export const node = (name, lname, lspec, rname, rspec, epilog = undefined, layout = undefined) =>
  inner(name, leaf(lname, lspec), leaf(rname, rspec), epilog, layout)

// The root node. No `matmul` key — see `inner` above.
export const root = (name, l, r, epilog = undefined) => ({
  name, epilog: epilog || 'none', left: l, right: r,
})

//
// Node kinds beyond the matmul.
//
// mm's expression grammar is matmul-only, and the comment these pages used to
// carry said that was why GPT-2's residual additions could not be drawn. That
// named the obstacle slightly wrong: `parseExpr` is matmul-only, but these
// pages never call it — they build the params tree directly. What was missing
// was node *kinds*, not notation, and viz.ts now has three. (The grammar is
// still matmul-only, and `genExpr`/`syncExpr` refuse a tree containing any of
// these rather than printing an '@' where an add is drawn.)
//

// f(input), materialized as its own matrix beside the input that produced it.
// This is the difference between mm's in-place epilogs — which mutate a
// matmul's result buffer, and which every other view still uses — and a stage
// an inspector can look at: `softmax(tril(QK^T/√d))` next to `Q @ K^T`.
export const unary = (name, fn, input, extra = {}) => ({
  name, op: 'unary', fn, input, anim: A(), block: B(), ...extra,
})

// An elementwise sum. GPT-2 adds twice per block and those are real edges of
// the graph; drawing one as a matmul that happened to produce the right numbers
// is exactly the class of lie this repository forbids.
export const add = (name, l, r, extra = {}) => ({
  name, op: 'add', left: l, right: r, anim: A(), block: B(), ...extra,
})

// An ordered list of stages in one scene. Keyed rather than an array because
// mm's `copyTree` round trips through flatten/unflatten, which does not handle
// arrays; string keys keep insertion order, which is the forward-pass order.
export const stack = (name, stages) => ({
  name, op: 'stack',
  stages: Object.fromEntries(stages.map((st, i) => [`s${i}`, st])),
})

// Root defaults. `build()` supplies name/epilog/left/right on top of these.
export const BASE = () => ({
  folder: 'closed',
  // Every group the viewer's panel builds has to be here, because a staged
  // scene is pushed with `{replace: true}` and that deletes the viewer's own
  // defaults rather than merging onto them. `diag` in particular is easy to
  // forget: nothing on this side reads it, and its absence surfaced as
  // "cannot read properties of undefined (reading 'folder')" from lil-gui.
  diag: { url: '', folder: 'closed' },
  anim: {
    alg: 'none', speed: 16, fuse: 'sync', 'hide inputs': false, spin: 0,
    stage: 0, 'play stages': false,
  },
  block: { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 },
  layout: {
    scheme: 'blocks', gap: 24, scatter: 0, molecule: 1, blast: 0,
    polarity: 'negative', 'left placement': 'left',
    'right placement': 'top', 'result placement': 'front',
    // Which way a staged scene's rows — one per transformer block — advance.
    // `refresh` overwrites this from the page's own Layers switch.
    'row flow': 'vertical',
  },
  deco: {
    legends: 6, shape: true, spotlight: 4, 'row guides': 0.1, 'flow guides': 0,
    'lens size': 0.5, magnification: 12, 'interior spotlight': false, axes: false,
    grid: 0, 'grid spacing i': 8, 'grid spacing j': 8, 'grid spacing k': 8,
  },
  // Written out rather than left to gui.ts's `||=` fallbacks: those run when
  // the panel is built, which is after the first scene. A view that means to
  // draw as heatmap has to say so before anything is constructed.
  viz: {
    sensitivity: 'local', 'min size': 0.05, 'min light': 0.2, 'max light': 0.9,
    'elem scale': 2, 'zero hue': 0.75, 'hue gap': 0.75, 'hue spread': 0.03,
    'render mode': 'auto', 'heatmap encoding': 'magnitude',
    'heatmap filter': 'nearest', 'lod reduce': 'maxAbs', 'texel budget': 0,
  },
  cam: { x: -400, y: 400, z: 400 },
})

// ---------------------------------------------------------------------------
// small utilities
// ---------------------------------------------------------------------------

export const esc = t => String(t).replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]))

const ord = n => n % 10 == 1 && n % 100 != 11 ? 'st' : n % 10 == 2 && n % 100 != 12 ? 'nd'
  : n % 10 == 3 && n % 100 != 13 ? 'rd' : 'th'

const copy = o => JSON.parse(JSON.stringify(o))

// Deep-merge an anim preset into a built params tree. The preset is copied
// first: presets are module-level literals and merging assigns sub-objects by
// reference, so without the copy the second application would mutate the first.
//
// A preset addresses nodes by spine (`left.left.left.anim`), so a preset written
// against a tree shape the page no longer has would otherwise *create* the path
// it named and leave a phantom `{anim: {...}}` node in params — the animation
// would silently not happen and nothing would say so. (The example this guards
// against is real: the original attnqkov page carried attngpt2's presets, whose
// `left.left.left` is `Q` in that tree and `inputQK` in this one.) Creating a
// leaf value is fine; creating a subtree is a bug in the preset.
export function merge(dst: any, src: any, path = '') {
  for (const [k, v] of Object.entries(src)) {
    const nested = v && typeof v == 'object' && !Array.isArray(v)
    if (nested && (dst[k] === undefined || dst[k] === null)) {
      throw new Error(`anim preset addresses '${path}${k}', which this tree does not have`)
    }
    if (nested && typeof dst[k] == 'object' && !Array.isArray(dst[k])) {
      merge(dst[k], v, path + k + '.')
    } else {
      dst[k] = v
    }
  }
  return dst
}

// Every rendered matrix, intermediates included — a leaf-only sum badly
// understates the attention-head views, where `attn` alone is n×n, and the
// QK/OV view, where the two premultiplied factors are 768×768 each.
export function countPoints(p) {
  if (p.op === 'stack') {
    const parts = Object.values(p.stages).map(countPoints)
    const last = parts[parts.length - 1]
    return { h: last.h, w: last.w, n: parts.reduce((n, x) => n + x.n, 0) }
  }
  if (p.op === 'unary') {
    // the input's whole subtree, plus the materialized result
    const i = countPoints(p.input)
    return { h: i.h, w: i.w, n: i.n + i.h * i.w }
  }
  if (p.op === 'add') {
    const l = countPoints(p.left), r = countPoints(p.right)
    return { h: l.h, w: l.w, n: l.n + r.n + l.h * l.w }
  }
  if (!p.left) return { h: p.h, w: p.w, n: p.h * p.w }
  const l = countPoints(p.left), r = countPoints(p.right)
  return { h: l.h, w: r.w, n: l.n + r.n + l.h * r.w }
}

// Approximate the drawn scene's bounding box. A matmul occupies H×W×D — the
// same H, D, W viz.js derives in its constructor — and each nested matmul is
// drawn as its own box hanging off that one, so nesting accumulates depth.
//
// These three are module scope rather than locals of `mount` because they are
// the rule that decides the camera distance, and a wrong camera is the failure
// nobody notices: the scene still draws, just framed uselessly. At module scope
// they can be asserted against a hand-computed tree.
export const height = p =>
  p.op === 'stack' ? bbox(p).h :
    p.op === 'unary' ? height(p.input) :
      p.op === 'add' ? height(p.left) :
        p.left ? height(p.left) : p.h

export const width = p =>
  p.op === 'stack' ? bbox(p).w :
    p.op === 'unary' ? width(p.input) :
      p.op === 'add' ? width(p.right) :
        p.left ? width(p.right) : p.w

// The extents `Stack.layoutStages` and `OpNode.layoutRow` actually produce,
// approximated the way the matmul case already is (gaps ignored). A wrong
// camera on a 37-stage scene draws fine and frames nothing, so these have to
// follow the layout rather than the arithmetic.
const rowOf = (parts, margin = 0) => ({
  h: Math.max(...parts.map(b => b.h)),
  w: parts.reduce((n, b) => n + b.w + margin, -margin),
  d: Math.max(...parts.map(b => b.d)),
})

// The same, stacked instead of laid across — a stack's row under
// `row flow: horizontal`, where stages advance up the row rather than along it.
const colOf = (parts, margin = 0) => ({
  h: parts.reduce((n, b) => n + b.h + margin, -margin),
  w: Math.max(...parts.map(b => b.w)),
  d: Math.max(...parts.map(b => b.d)),
})

export function bbox(p, gap = (p.layout && p.layout.gap) || 0) {
  // The matmul cases ignore `gap`, as they always have — a matmul's own extent
  // dwarfs it. The staged cases cannot: `Stack.layoutStages` puts a `gap * 4`
  // margin between rows and every stage extent carries `2 * gap` of its own, so
  // for a 7-row model scene of 30-token matrices the gaps are most of the
  // height. Ignoring them underestimated the scene by 6x on that axis, and the
  // camera distance derived from it framed a fifth of the model.
  const pad = 2 * gap, margin = 4 * gap
  if (p.op === 'stack') {
    // Follows `Stack.layoutStages` and must keep following it, including its
    // transpose: vertically, stages run along a row and rows stack up the
    // scene; horizontally, both swap. Read off `p.layout` — a stack is a root
    // node, and stages carry no layout of their own.
    const horizontal = (p.layout || {})['row flow'] === 'horizontal'
    const rows: any = {}
    Object.values(p.stages).forEach((st: any) => (rows[st.row || 0] ||= []).push(bbox(st, gap)))
    const laid = Object.keys(rows).map(k =>
      horizontal ? colOf(rows[k], margin) : rowOf(rows[k], margin))
    const d = Math.max(...laid.map(r => r.d))
    return horizontal ? {
      h: Math.max(...laid.map(r => r.h)),
      w: laid.reduce((n, r) => n + r.w + margin, -margin),
      d,
    } : {
      h: laid.reduce((n, r) => n + r.h + margin, -margin),
      w: Math.max(...laid.map(r => r.w)),
      d,
    }
  }
  if (p.op === 'unary') {
    const i = bbox(p.input, gap)
    return rowOf([i, { h: height(p.input) + pad, w: width(p.input) + pad, d: 0 }], margin)
  }
  if (p.op === 'add') {
    const [l, r] = [bbox(p.left, gap), bbox(p.right, gap)]
    return rowOf([l, r, { h: l.h, w: l.w, d: 0 }], margin)
  }
  if (!p.left) return { h: p.h + pad, w: p.w + pad, d: 0 }
  const child = (p.left.left || p.left.op ? bbox(p.left, gap).d : 0) +
    (p.right.left || p.right.op ? bbox(p.right, gap).d : 0)
  return { h: height(p.left), w: width(p.right), d: width(p.left) + child }
}

// ---------------------------------------------------------------------------
// what the picture is — the claims `showStatus` builds
// ---------------------------------------------------------------------------
//
// These are separate on purpose, and both are separate from "it drew". A leaf
// can be exact checkpoint data while the product is not the model's, and since
// augmentation arrived the reverse is possible too: the product can be exactly
// GPT-2's while one column of one leaf is a synthetic constant. Saying only
// "exact" would be false in the second case and misleading in the first.
//
// Pure functions of the server's own spec entries, so they can be tested
// without a server, a browser or a frame.
//

const list = xs => xs.length < 2 ? xs.join('')
  : xs.slice(0, -1).join(', ') + ' and ' + xs[xs.length - 1]

const kindOf = k => k.split(':')[0]

// the flags only — `mlp_c_proj:ch` strides columns, and looking for 'r' in the
// whole item would find the one in the kind's own name
const flagsOf = k => k.split(':')[1] || ''

const uniq = xs => [...new Set(xs)]

// A view can augment a matrix both ways — the attention head takes ln_1 as a
// left operand (ones column) and its transpose as a right one (ones row) — so
// this groups by axis rather than assuming one.
function augPhrase(items, what) {
  const on = axis => uniq(items.filter(([, v]) => v.augment.axis == axis).map(([k]) => kindOf(k)))
  const [cols, rows] = [on('col'), on('row')]
  return list([
    ...(cols.length ? [`${what} column on ${list(cols)}`] : []),
    ...(rows.length ? [`${what} row on ${list(rows)}`] : []),
  ])
}

// `data:` — what the numbers in the leaves are.
export function dataClaim(specs: Record<string, any>, stride: number) {
  const entries = Object.entries(specs)
  const sampled = entries.filter(([, v]) => v.fidelity == 'sampled')
  const aug = k => entries.filter(([, v]) => v.augment && v.augment.vector == k)
  const [ones, biases] = [aug('ones'), aug('bias')]

  const parts = [sampled.length
    ? `<span class="sampled">sampled</span> <span class="dim">— every ${stride}${ord(stride)} ` +
      `${list(uniq(sampled.map(([k]) => kindOf(k))))} ` +
      `${sampled.every(([k]) => flagsOf(k).includes('r')) ? 'row' : 'column'}, no interpolation; ` +
      `contracted axes are never decimated</span>`
    : `<span class="exact">exact</span> <span class="dim">— every element is the checkpoint's own</span>`]

  // Named, never glossed: the ones vector is the one part of a leaf the model
  // did not supply. It is also what makes the product right, so it earns its
  // place — but "exact" must not be left standing over it unqualified.
  if (ones.length) {
    parts.push(`<span class="dim">plus a synthetic ${augPhrase(ones, 'all-ones')}` +
      ` — the constant 1 the bias multiplies</span>`)
  }
  if (biases.length) {
    parts.push(`<span class="dim">the appended ${augPhrase(biases, 'bias')}: ` +
      `${list(uniq(biases.map(([, v]) => esc(v.augment.tensor))))}` +
      `${sampled.length ? ', strided with the output axis it indexes' : ''}</span>`)
  }
  return parts.join('<span class="dim"> · </span>')
}

// `product:` — what the drawn matmul computes, against what GPT-2 computes.
//
// "complete" is a claim in its own right, so it is only ever printed when
// `gap` is empty. Saying "includes c_attn.bias" and "complete" in the same
// breath as a known omission is exactly the elision this bar exists to avoid.
export function productClaim(view: any) {
  const drawn = view.bias
    ? `<span class="exact">includes ${esc(view.bias)}</span> ` +
      `<span class="dim">— augmented into the matmul as [X | 1] @ [W ; b]</span>`
    : `<span class="dim">this step has no bias term</span>`
  return drawn + `<span class="dim"> · </span>` + (view.gap
    ? `<span class="sampled">gap</span> <span class="dim">— ${esc(view.gap)}</span>`
    : `<span class="exact">complete</span> ` +
      `<span class="dim">— what is drawn is the value GPT-2 computes here</span>`)
}

// `render:` — what the *renderer* did to the numbers on their way to pixels.
//
// The third claim, and it earns its place for the same reason the other two do:
// a heatmap at LOD 2 is showing one maxAbs per 4x4 block of the matrix, and a
// bar that printed "exact" over the leaf data while that was on screen would be
// telling the truth about the wrong thing. `summary` is measured by the viewer
// after it builds -- never predicted here, because the LOD ladder is bounded by
// the viewer's viewport and this side does not know it.
//
// The exact/sampled/approximate rule applied to pixels: visual fidelity is not
// numerical fidelity, and the hover readout still prints the checkpoint's own
// FP32 value whatever the texel says.
// Spheres past this many elements are drawn but will not be smooth: it is one
// instanced quad -- four vertices -- per element, so this is ~8M vertices a
// frame. Said rather than prevented: an explicit override is the user's call.
const SPHERE_WARN = 2_000_000

export function renderClaim(summary) {
  if (!summary) return ''
  const spheres = summary.elements
    ? `<span class="${summary.elements > SPHERE_WARN ? 'sampled' : 'dim'}">` +
      `${summary.elements.toLocaleString()} elements as spheres` +
      `${summary.elements > SPHERE_WARN ? ' — 4 vertices each, expect a slow frame' : ''}</span>`
    : ''
  if (!summary.heatmaps) {
    return `<span class="exact">spheres</span> <span class="dim">— one shaded ` +
      `sphere per element across ${summary.mats} ` +
      `${summary.mats == 1 ? 'matrix' : 'matrices'}</span>` +
      (spheres ? `<span class="dim"> · </span>` + spheres : '')
  }
  const enc = summary.encoding === 'signed' ? 'signed (hue by sign)'
    : summary.encoding === 'mixed' ? 'mixed encodings'
      : 'magnitude |x|'
  const where = `<span class="dim">${summary.heatmaps}/${summary.mats} as heatmap, ` +
    `${summary.texels.toLocaleString()} texels, ${enc}</span>` +
    (spheres ? `<span class="dim"> · </span>` + spheres : '')
  return (summary.lod > 1
    ? `<span class="sampled">LOD ${Math.log2(summary.lod)}</span> ` +
      `<span class="dim">— one texel per ${summary.lod}×${summary.lod} cells by ` +
      `${summary.reducer}, so the picture is not exact even where the data is; ` +
      `hover still reads the checkpoint's own value</span>`
    : `<span class="exact">LOD 0</span> <span class="dim">— one texel per element, ` +
      `quantized to 8 bits for display only</span>`) +
    `<span class="dim"> · </span>` + where
}

// Every matmul in the tree must contract over a shared extent. mm does not
// check: `tryURLInit` wraps out-of-range indices, so a left operand 769 wide
// against a right operand 768 tall is *tiled* into a plausible, wrong picture
// with nothing reported anywhere. Augmentation makes that a live risk — the
// two sides of a matmul now grow together, and augmenting one and not the
// other is a one-character mistake — so the tree is checked before it is sent.
export function checkShapes(p, path = 'root') {
  if (p.op === 'stack') {
    const shapes = Object.entries(p.stages)
      .map(([k, st]) => checkShapes(st, `${path}.${k} ('${(st as any).name}')`))
    return shapes[shapes.length - 1]
  }
  if (p.op === 'unary') {
    // pointwise and row-wise stages preserve shape by construction
    return checkShapes(p.input, path + '.input')
  }
  if (p.op === 'add') {
    const l = checkShapes(p.left, path + '.left')
    const r = checkShapes(p.right, path + '.right')
    if (l.h !== r.h || l.w !== r.w) {
      throw new Error(
        `${path} ('${p.name}') adds '${p.left.name}' ${l.h}×${l.w} to ` +
        `'${p.right.name}' ${r.h}×${r.w}: an elementwise sum needs one shape. ` +
        `A strided residual stream has to be strided with the projection whose ` +
        `output it is added to.`)
    }
    return l
  }
  if (!p.left) return { h: p.h, w: p.w }
  const l = checkShapes(p.left, path + '.left')
  const r = checkShapes(p.right, path + '.right')
  if (l.w !== r.h) {
    throw new Error(
      `${path} ('${p.name}') contracts '${p.left.name}' ${l.h}×${l.w} with ` +
      `'${p.right.name}' ${r.h}×${r.w}: ${l.w} ≠ ${r.h}. mm would tile the ` +
      `shorter one rather than fail — check the w/h augmentation flags on both.`)
  }
  return { h: l.h, w: r.w }
}

// Which value the page's `Render` selector should show, given the render mode
// the viewer reports it is using — or null when nothing should change.
//
// The viewer's own lil-gui panel writes the same state this selector does (a
// root toggle plus the `render mode` dropdown, see gui.ts), and `refresh` pushes
// this selector's value back into params on every rebuild. So without adopting
// the report, a flip in the panel survives exactly until the next layer or
// prompt change and is then silently undone.
//
// Pure, and separate from the message listener that calls it, because the
// listener needs a live iframe and this needs a test. `allowed` is the
// selector's own option list: a mode the page does not offer is left alone
// rather than assigned to a `<select>` that has no such option — which would
// blank it and make the next refresh push an empty string.
export function adoptRenderMode(current, reported, allowed = RENDER_MODES) {
  if (!reported || reported === current) return null
  return allowed.includes(reported) ? reported : null
}

async function getJSON(url): Promise<any> {
  const r = await fetch(abs(url))
  const body = await r.json().catch(() => ({ error: `HTTP ${r.status}` }))
  if (!r.ok) throw new Error(body.error || `HTTP ${r.status}`)
  return body
}

// ---------------------------------------------------------------------------
// page chrome
// ---------------------------------------------------------------------------

const CHROME = `
<div id="header">
  <label for="prompt">Prompt</label>
  <input type="text" id="prompt" />
  <label for="view" id="view_label">View</label>
  <select id="view"></select>
  <label for="layer">Layer</label>
  <select id="layer"></select>
  <label for="head">Head</label>
  <select id="head"></select>
  <label for="seq">Tokens</label>
  <select id="seq"></select>
  <label for="stride">Stride</label>
  <select id="stride"></select>
  <label for="flow">Layers</label>
  <select id="flow">
    <option value="vertical">vertical (rows)</option>
    <option value="horizontal">horizontal (columns)</option>
  </select>
  <label for="anim">Animate</label>
  <select id="anim"></select>
  <label for="render">Render</label>
  <select id="render"></select>
  <a href="#" id="popout_link">open&#x2197;</a>
</div>
<div id="timeline" class="hidden">
  <button id="tl_prev" title="previous stage">&#x25C0;</button>
  <button id="tl_play" title="play / pause">&#x25B6;</button>
  <button id="tl_next" title="next stage">&#x25B6;&#x25B6;</button>
  <input type="range" id="tl_scrub" min="0" max="0" value="0" step="1" />
  <span id="tl_label" class="dim"></span>
</div>
<div id="stage">
  <iframe id="mm" src="about:blank"></iframe>
  <div id="status">loading&#x2026;</div>
</div>
`

function option(elem, value, text = undefined) {
  const o = document.createElement('option')
  o.value = value
  o.text = text === undefined ? value : text
  elem.add(o)
  return o
}

function fillRange(elem, n) {
  elem.options.length = 0
  for (let i = 0; i < n; i++) option(elem, String(i))
}

//
// The status bar no longer has a row of its own. It was six wrapped lines deep
// at the top of the page — tokens, the view's note, every leaf's shape, the
// element count and the three claims — and all of that height came off the
// viewer, which is the thing the page is for. It is now an overlay pinned to the
// top of the stage, out of flow, so the frame below it is full height.
//
// Two ways in, and only one of them shows it:
//
//   `fail`    the model server is unreachable, a matrix is a 501 refusal, a
//             tree would be tiled rather than contracted, a page's own
//             precondition is unmet. Every one of these means there is nothing
//             to look at, so the overlay is the only thing on screen that can
//             say so — and the first of them is the failure a new checkout hits
//             before anything works at all. Same for the `loading…` the chrome
//             ships with, which the first successful refresh clears.
//
//   `claims`  what `showStatus` builds. Still built on every refresh, still
//             written here, and still hidden: `data:`/`product:`/`render:` are
//             not errors. Written rather than dropped so that the claim
//             functions keep a caller and unhiding is one line — the height
//             argument is about where the text goes, not about whether it is
//             true.
//
const fail = html => {
  $('status').innerHTML = html
  $('status').classList.remove('hidden')
}

const claims = html => {
  $('status').innerHTML = html
  $('status').classList.add('hidden')
}

// ---------------------------------------------------------------------------
// mount
// ---------------------------------------------------------------------------

//
// config:
//   views      {name: view}                    required
//   prompt     default prompt text
//   seq        default token count (64)
//   seqs       token-count choices
//   strides    stride choices
//   anims      default anim menu, label -> params patch (flat algs if omitted)
//   require    (meta) => message | null — a precondition to refuse on
//   viewerUrl  page-relative URL of the mm viewer ('../viewer/index.html').
//              The default suits a page at /<name>/; the home page at `/`
//              passes 'viewer/index.html' instead.
//
// view:
//   kinds   ['ln_1:w', 'wq:h', …] — `kind[:flags]`; t=transpose, r/c=stride
//           axis, w/h=augment with a bias column/row (see the home index.html)
//   layers  true to fetch specs for *every* layer and hand `build` an array
//           indexed by layer, for a view that draws the whole model at once
//   stride  default stride for this view
//   seq     default token count for this view (optional)
//   head    whether the head selector applies
//   bias    the GPT-2 bias now drawn inside the matmul, or null if there is none
//   gap     what the drawn product still is not, or null if it is the model's
//   note    (state) => one-line description
//   build   (specs, state) => root node (name/epilog/left/right)
//   anims   optional label -> params patch, replacing the page default
//
export async function mount(config: any) {
  const views = config.views
  const viewer = config.viewerUrl || '../viewer/index.html'
  const seqs = config.seqs || [16, 32, 64, 128, 256]
  const strides = config.strides || [1, 2, 4, 8, 16, 32, 64]
  const default_anims = config.anims || {
    'none': { anim: { alg: 'none' } },
    'vmprod': { anim: { alg: 'vmprod' } },
    'mvprod': { anim: { alg: 'mvprod' } },
    'vvprod': { anim: { alg: 'vvprod' } },
    'dotprod (row major)': { anim: { alg: 'dotprod (row major)' } },
  }

  // prepended, not assigned: the caller is an inline module script inside this
  // same body, and replacing innerHTML would detach it mid-execution
  document.body.insertAdjacentHTML('afterbegin', CHROME)
  $('prompt').value = config.prompt || ''

  let META = null
  let current_view = null
  let mm_ready = false
  // Whether the frame has been pointed at the viewer at all. Read where
  // `$('mm').src != 'about:blank'` used to be: `navigate` below drives the frame
  // through its own location rather than through the src attribute, so that
  // attribute stays 'about:blank' for the life of the page and can no longer
  // answer the question.
  let mm_navigated = false

  // Point the frame at a viewer URL.
  //
  // `location.replace`, not `$('mm').src = url`. Assigning src to a frame that
  // already holds a document pushes an entry onto the joint session history, so
  // every view, prompt, render-mode or stride change added a Back step. Backing
  // over one of those -- deliberately, or with the two-finger swipe the viewer
  // documents as its zoom gesture -- reloaded the frame at the *previous* view's
  // URL while this page's chrome, selectors and status all went on describing
  // the current one. That is the "auto redirect to another view", and it is a
  // navigation bug rather than a page-structure one: nothing here needs to be
  // split into another HTML file to fix it.
  //
  // `replace` swaps the frame's document in place and adds no entry. It is also
  // correct for the first navigation: the frame's initial about:blank is
  // same-origin, so `contentWindow` is reachable, and replacing it is what
  // assigning src to a never-loaded frame does anyway.
  //
  // The URL is made absolute against this page rather than left relative: it is
  // resolved against the *frame's* base URL, which for about:blank is inherited
  // and not something to depend on.
  const navigate = url => {
    mm_navigated = true
    $('mm').contentWindow.location.replace(abs(url))
  }

  // A staged scene is far too big to travel as `?params=` on the iframe URL --
  // distilgpt2's is tens of thousands of nodes -- so it is pushed over the
  // message protocol once the blank viewer has loaded.
  let pending_params = null
  let stage_state = null

  const anims = () => views[$('view').value].anims || default_anims

  const state = () => ({
    prompt: $('prompt').value,
    view: $('view').value,
    layer: +$('layer').value,
    head: +$('head').value,
    stride: +$('stride').value,
    seq: +$('seq').value,
    anim: $('anim').value,
    render: $('render').value,
    flow: $('flow').value,
    dims: META.dims,
  })

  // The deep link is written from the chrome's own state, so it is correct from
  // anywhere that changes the chrome — not only from `refresh`. The render mode
  // is the case that needed it: it can now be changed inside the viewer, which
  // reports back rather than going through a refresh.
  const saveDeepLink = (s = state()) =>
    history.replaceState({}, '', '?' + new URLSearchParams({
      view: s.view, layer: String(s.layer), head: String(s.head),
      stride: String(s.stride), seq: String(s.seq), anim: s.anim,
      render: s.render, flow: s.flow, prompt: s.prompt,
    }))

  // -- build + push --------------------------------------------------------

  async function refresh(reload) {
    const s = state()
    const view = views[s.view]

    // stride only reaches matrices whose flags asked for it
    const qs = layer => new URLSearchParams({
      layer: String(layer), head: String(s.head), stride: String(s.stride),
      text: s.prompt, seq: String(s.seq), kinds: view.kinds.join(','),
    })

    // A whole-model view needs every layer's shapes, and specs.json answers for
    // one layer at a time. Same rule as everywhere else: the shapes come from
    // the server, one request per layer, never from a literal.
    const layers = view.layers ? META.dims.n_layer : 1
    let specs: Record<string, any>, by_layer: Record<string, any>[]
    let n_tokens: number, toks: string[]
    try {
      const [sps, tk] = await Promise.all([
        Promise.all(Array.from({ length: layers }, (_, l) =>
          getJSON('/api/specs.json?' + qs(view.layers ? l : s.layer)))),
        getJSON('/api/tokens.json?seq=' + s.seq + '&text=' + encodeURIComponent(s.prompt)),
      ])
      by_layer = sps.map(r => r.specs)
      // Every layer carries the same kinds at the same shapes, so the status
      // bar's claims are the same for all of them; merging is only so the
      // claim functions see one entry per kind rather than six identical ones.
      specs = Object.assign({}, ...by_layer)
      n_tokens = sps[0].n_tokens; toks = tk.tokens
    } catch (e) {
      fail(`<span class="err">${esc(e.message)}</span>`)
      return
    }

    // refuse up front rather than let mm coerce a 501 body into NaN. Scanned
    // per layer, not over the merge: a merge would let an available layer 0
    // hide an unavailable layer 3.
    for (const [l, sp] of by_layer.entries()) {
      const missing = Object.entries(sp).filter(([, v]: any) => v.available === false)
      if (missing.length) {
        fail(`<span class="err">${esc((missing[0][1] as any).reason)}` +
          `${view.layers ? ` (layer ${l})` : ''}</span>`)
        return
      }
    }

    const params = Object.assign(BASE(), view.build(view.layers ? by_layer : specs, s))

    // The render path, chosen here rather than only in the viewer's own panel:
    // it is a property of the picture, so it belongs beside the other choices
    // that decide what the picture is. 'auto' is the default and is the only
    // one that varies per matrix (see pickRenderMode); 'spheres' and 'heatmap'
    // apply to every matrix in the tree, including a staged model scene, which
    // would otherwise force heatmap on everything and make this control a lie.
    params.viz['render mode'] = s.render

    // How a staged scene arranges its rows — one row per transformer block, so
    // this is "layers down the screen" vs "layers across it". Written before
    // `bbox` below, which follows the same arrangement to size the camera.
    params.layout['row flow'] = s.flow

    // refuse a tree mm would tile rather than reject
    try {
      checkShapes(params)
    } catch (e) {
      fail(`<span class="err">${esc(e.message)}</span>`)
      return
    }

    merge(params, copy(anims()[s.anim] || { anim: { alg: 'none' } }))

    // pull the camera back to fit this view — a 6-token attention head and a
    // pair of 768×768 premultiplied circuits need very different distances.
    //
    // The measure has to be the whole scene, not the largest single matrix.
    // mm draws one matmul as three faces of an H×W×D box and hangs each nested
    // matmul off it, so a deep tree is far bigger than any matrix in it: the
    // QK/OV view's largest leaf is 768 wide but its scene is ~3200 across.
    // main.js rescales the camera by the bounding box's h+w+d when params
    // change, and viz.js `defaultCam` sits at half that magnitude per axis —
    // this is the same rule, applied at first load.
    const bb = bbox(params)
    const d = Math.round((bb.h + bb.w + bb.d) / 2)
    params.cam = { x: -d, y: d, z: d }
    // For a staged scene this is a starting point, not the final answer: the
    // viewer re-fits the camera to the world box of what it actually built
    // (`frameStagedScene` in main.ts), because a stack's content does not
    // straddle the origin the way a single matmul's does. This still has to be
    // right to within an order of magnitude — it is what the viewer's near/far
    // planes and its first frame are sized against.

    saveDeepLink(s)

    // a different view is a different tree shape, so merging props onto the
    // old one would leave stale nodes behind — reload the viewer instead
    if (reload || s.view !== current_view || !mm_ready) {
      current_view = s.view
      mm_ready = false
      stage_state = null
      if (params.op) {
        // Staged scene: load the viewer bare and hand it the tree over the
        // protocol. `replace` (not a merge) because the default tree it comes
        // up with is a matmul and this one is not the same shape.
        pending_params = params
        navigate(viewer)
      } else {
        pending_params = null
        navigate(viewer + '?params=' + encodeURIComponent(JSON.stringify(params)))
      }
    } else if (params.op) {
      $('mm').contentWindow.postMessage(
        { setParams: { props: params, reset: false, replace: true } }, '*')
    } else {
      const { cam, ...rest } = params            // keep the user's camera
      $('mm').contentWindow.postMessage({ setParams: { props: rest, reset: false } }, '*')
    }

    $('timeline').classList.toggle('hidden', !params.op)
    showStatus(s, view, specs, toks, n_tokens, params)
  }

  // The last status render's inputs, so the claims can be rebuilt when the
  // viewer reports back what it built without refetching anything.
  let last_status = null
  let render_summary = null

  function showStatus(s, view, specs: Record<string, any>, toks, n_tokens, params) {
    last_status = [s, view, specs, toks, n_tokens, params]
    const entries = Object.entries(specs)
    const points = countPoints(params).n

    const shapes = entries
      .map(([k, v]) => `<span class="dim">${esc(k.replace(':', '·'))}</span> ${v.h}×${v.w}`)
      .join('   ')

    // two separate claims: what the leaf data is, and what the product is
    const data = dataClaim(specs, s.stride)
    const product = productClaim(view)

    claims(
      `<div style="flex-basis:100%">` +
        toks.map(t => `<span class="tok">${esc(t)}</span>`).join('') +
        `<span class="dim">${n_tokens} tokens</span></div>` +
      `<div style="flex-basis:100%"><span class="expr">${esc(view.note(s))}</span></div>` +
      `<div>${shapes}</div>` +
      `<div class="dim">${points.toLocaleString()} drawn matrix elements</div>` +
      `<div style="flex-basis:100%"><span class="dim">data:</span> ${data}</div>` +
      `<div style="flex-basis:100%"><span class="dim">product:</span> ${product}</div>` +
      (render_summary
        ? `<div style="flex-basis:100%"><span class="dim">render:</span> ` +
          `${renderClaim(render_summary)}</div>`
        : '')
    )
  }

  function popout() {
    const w = window.open('', '_blank')
    const r = e => {
      if (e.data && e.data.url_info) {
        w.location = e.data.url_info.url
        window.removeEventListener('message', r)
      }
    }
    window.addEventListener('message', r)
    $('mm').contentWindow.postMessage({ getUrlInfo: undefined }, '*')
  }

  // -- init ----------------------------------------------------------------

  try {
    META = await getJSON('/api/meta.json')
  } catch (e) {
    fail(`<span class="err">cannot reach the model server (${esc(e.message)}).` +
      ` Start it with <code>../.venv/bin/python tools/gpt2_server.py</code>` +
      ` from the <code>mm/</code> directory, then open` +
      ` <code>${esc(location.pathname)}</code> on that origin` +
      ` (or run <code>npm run dev</code>, which proxies /api to it).</span>`)
    return
  }

  // a page may declare a precondition it cannot draw honestly without
  const refusal = config.require && config.require(META)
  if (refusal) {
    fail(`<span class="err">${esc(refusal)}</span>`)
    return
  }

  Object.keys(views).forEach(k => option($('view'), k))
  fillRange($('layer'), META.dims.n_layer)
  fillRange($('head'), META.dims.n_head)
  RENDER_MODES.forEach(m => option($('render'), m,
    m === 'auto' ? 'auto (by size)' : m))
  strides.forEach(n => option($('stride'), String(n), n == 1 ? '1 (exact)' : String(n)))
  seqs.forEach(n => option($('seq'), String(n)))
  $('seq').value = String(config.seq || 64)

  if (!META.deep_layers) {
    $('layer').options.length = 1   // only layer 0 is honest without numpy
  }

  // a single-view page has nothing to choose between
  if (Object.keys(views).length < 2) {
    $('view').classList.add('hidden')
    $('view_label').classList.add('hidden')
  }

  // `load` is too early to *talk* to the viewer -- main.ts has a top-level
  // await, so its message listener is installed after this fires -- but it is
  // the right moment to know the frame exists. The push waits for the viewer's
  // own `ready`.
  $('mm').addEventListener('load', () => { mm_ready = mm_navigated })

  const flushPending = () => {
    if (!pending_params) return
    $('mm').contentWindow.postMessage(
      { setParams: { props: pending_params, reset: true, replace: true } }, '*')
    pending_params = null
  }

  // -- stage timeline ------------------------------------------------------
  //
  // The chrome lives here and the scene lives in the viewer, which is the split
  // the header comment argues for. What crosses is two small messages: this
  // side says which stage, that side says what it built.

  const sendStage = (patch) =>
    mm_ready && $('mm').contentWindow.postMessage({ setStage: patch }, '*')

  window.addEventListener('message', e => {
    if (e.data && e.data.ready) { mm_ready = true; flushPending() }
    if (e.data && e.data.render) {
      render_summary = e.data.render
      // Value only, never a dispatched `change`: the handler on that select
      // calls refresh(true), which reloads the very frame this message came
      // from. The deep link is rewritten here for the same reason — nothing
      // else will, until the next refresh.
      const adopted = adoptRenderMode($('render').value, e.data.render.mode,
        [...$('render').options].map(o => o.value))
      if (adopted !== null) {
        $('render').value = adopted
        saveDeepLink()
      }
      if (last_status) showStatus.apply(null, last_status)
    }
    if (!e.data || !e.data.stages) return
    stage_state = e.data.stages
    const { list, active, playing, summary } = stage_state
    $('tl_scrub').max = String(Math.max(0, list.length - 1))
    $('tl_scrub').value = String(active)
    $('tl_play').textContent = playing ? '\u23F8' : '\u25B6'
    const st = list[active] || { name: '?', kind: '?', note: '' }
    // Names what is on screen *and* what it is: a stage drawn one maxAbs per
    // 16x16 block is not exact, and the bar must not let it read as though it
    // were.
    const lod = summary && summary.lod > 1
      ? ` · <span class="sampled">LOD ${Math.log2(summary.lod)}</span>` +
        ` <span class="dim">— 1 texel per ${summary.lod}×${summary.lod} cells` +
        ` by ${summary.reducer}, not exact</span>`
      : ' · <span class="dim">LOD 0 — one texel per element</span>'
    const enc = summary && summary.heatmaps
      ? ` · <span class="dim">${summary.encoding === 'signed' ? 'signed' :
          summary.encoding === 'mixed' ? 'mixed encodings' : 'magnitude |x|'} ramp,` +
        ` ${summary.texels.toLocaleString()} texels over ${summary.heatmaps}/${summary.mats}` +
        ` matrices</span>`
      : ''
    $('tl_label').innerHTML =
      `stage ${active + 1}/${list.length}: <b>${esc(st.name)}</b>` +
      `<span class="dim"> (${esc(st.kind)})</span>` +
      (st.note ? ` <span class="dim">— ${esc(st.note)}</span>` : '') + enc + lod
  })

  $('tl_scrub').addEventListener('input', () =>
    sendStage({ index: +$('tl_scrub').value, playing: false }))
  $('tl_prev').addEventListener('click', () => sendStage({ step: -1, playing: false }))
  $('tl_next').addEventListener('click', () => sendStage({ step: 1, playing: false }))
  $('tl_play').addEventListener('click', () =>
    sendStage({ playing: !(stage_state && stage_state.playing) }))

  const applyViewDefaults = () => {
    const v = views[$('view').value]
    const strideable = v.kinds.some(k => /:[rc]/.test(k))

    const keep = $('anim').value
    $('anim').options.length = 0
    Object.keys(anims()).forEach(k => option($('anim'), k))
    if ([...$('anim').options].some(o => o.value == keep)) $('anim').value = keep

    $('stride').value = v.stride
    if (v.seq !== undefined) $('seq').value = v.seq
    $('head').disabled = !v.head
    $('head').style.opacity = v.head ? 1 : 0.4
    $('stride').disabled = !strideable
    $('stride').style.opacity = strideable ? 1 : 0.4
    // Rows exist only in a staged scene; a single matmul has nothing to arrange
    $('flow').disabled = !v.layers
    $('flow').style.opacity = v.layers ? 1 : 0.4
  }

  $('view').addEventListener('change', () => { applyViewDefaults(); refresh(true) })
  $('prompt').addEventListener('change', () => refresh(false))
  // A render-mode change rebuilds the scene from scratch: which path a matrix
  // takes is decided when it is built, so this reloads rather than patching.
  $('render').addEventListener('change', () => refresh(true))
  ;['layer', 'head', 'stride', 'seq', 'anim', 'flow'].forEach(id =>
    $(id).addEventListener('change', () => refresh(false)))
  $('popout_link').addEventListener('click', e => { e.preventDefault(); popout() })

  applyViewDefaults()

  // deep link: ?view=mlp+down&layer=3&head=7&stride=4&prompt=…
  const qp = new URLSearchParams(location.search)
  if (qp.has('prompt')) $('prompt').value = qp.get('prompt')
  if (views[qp.get('view')]) { $('view').value = qp.get('view'); applyViewDefaults() }
  for (const id of ['layer', 'head', 'stride', 'seq', 'anim', 'render', 'flow']) {
    if (qp.has(id) && [...$(id).options].some(o => o.value == qp.get(id))) {
      $(id).value = qp.get(id)
    }
  }

  refresh(true)
}
