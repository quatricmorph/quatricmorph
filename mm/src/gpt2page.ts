//
// Shared driver for the checkpoint-backed example pages.
//
// `gpt2/`, `attngpt2/` and `attnqkov/` all do the same
// thing: ask `tools/gpt2_server.py` (under /api/) for the shape and URL of every matrix in a
// tree, hand that tree to the unmodified mm viewer in an iframe, and say in the
// status bar exactly what the picture is and is not. Only the *tree* differs
// between them, so only the tree lives in the page; everything else is here.
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
// The pages drive mm through its existing public surface only — `?params=` on
// the iframe URL and `postMessage({setParams})`. `index.html`, `viz.js`,
// `util.js` and `gui.js` are not touched by any of this.
//

import './gpt2page.css'

// Typed `any` deliberately. Everything this module looks up is an <input>, a
// <select> or the <iframe>, and it reads .value / .options / .disabled / .src
// off them -- but getElementById is only ever HTMLElement, so the alternative
// is a cast at all ~30 call sites. The chrome these ids belong to is injected
// by this same file (see CHROME), so the shapes are not in question.
const $ = (id: string): any => document.getElementById(id)

// Absolute, same-origin. The CSV URLs specs.json hands back are root-relative
// (`/api/matrix.csv?…`), which works both under `vite dev` — vite.config.ts
// proxies /gpt2 to the python server — and under gpt2_server.py serving mm/
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

// Root defaults. `build()` supplies name/epilog/left/right on top of these.
export const BASE = () => ({
  folder: 'closed',
  anim: { alg: 'none', speed: 16, fuse: 'sync', 'hide inputs': false, spin: 0 },
  block: { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 },
  layout: {
    scheme: 'blocks', gap: 24, scatter: 0, molecule: 1, blast: 0,
    polarity: 'negative', 'left placement': 'left',
    'right placement': 'top', 'result placement': 'front',
  },
  deco: {
    legends: 6, shape: true, spotlight: 4, 'row guides': 0.1, 'flow guides': 0,
    'lens size': 0.5, magnification: 12, 'interior spotlight': false, axes: false,
  },
  viz: {
    sensitivity: 'local', 'min size': 0.05, 'min light': 0.2, 'max light': 0.9,
    'elem scale': 2, 'zero hue': 0.75, 'hue gap': 0.75, 'hue spread': 0.03,
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
export const height = p => p.left ? height(p.left) : p.h
export const width = p => p.left ? width(p.right) : p.w

export function bbox(p) {
  if (!p.left) return { h: p.h, w: p.w, d: 0 }
  const child = (p.left.left ? bbox(p.left).d : 0) + (p.right.left ? bbox(p.right).d : 0)
  return { h: height(p.left), w: width(p.right), d: width(p.left) + child }
}

// ---------------------------------------------------------------------------
// what the picture is — the two claims the status bar makes
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

// Every matmul in the tree must contract over a shared extent. mm does not
// check: `tryURLInit` wraps out-of-range indices, so a left operand 769 wide
// against a right operand 768 tall is *tiled* into a plausible, wrong picture
// with nothing reported anywhere. Augmentation makes that a live risk — the
// two sides of a matmul now grow together, and augmenting one and not the
// other is a one-character mistake — so the tree is checked before it is sent.
export function checkShapes(p, path = 'root') {
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
  <label for="anim">Animate</label>
  <select id="anim"></select>
  <a href="#" id="tensors_link">tensors</a>
  <a href="#" id="popout_link">open&#x2197;</a>
</div>
<div id="status">loading…</div>
<iframe id="mm" src="about:blank"></iframe>
<div id="tensors"></div>
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

const status = html => { $('status').innerHTML = html }

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
//   viewerUrl  page-relative URL of the mm viewer ('../index.html')
//
// view:
//   kinds   ['ln_1:w', 'wq:h', …] — `kind[:flags]`; t=transpose, r/c=stride
//           axis, w/h=augment with a bias column/row (see gpt2/index.html)
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
  const viewer = config.viewerUrl || '../index.html'
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

  const anims = () => views[$('view').value].anims || default_anims

  const state = () => ({
    prompt: $('prompt').value,
    view: $('view').value,
    layer: +$('layer').value,
    head: +$('head').value,
    stride: +$('stride').value,
    seq: +$('seq').value,
    anim: $('anim').value,
    dims: META.dims,
  })

  // -- build + push --------------------------------------------------------

  async function refresh(reload) {
    const s = state()
    const view = views[s.view]

    // stride only reaches matrices whose flags asked for it
    const q = new URLSearchParams({
      layer: String(s.layer), head: String(s.head), stride: String(s.stride),
      text: s.prompt, seq: String(s.seq), kinds: view.kinds.join(','),
    })

    let specs: Record<string, any>, n_tokens: number, toks: string[]
    try {
      const [sp, tk] = await Promise.all([
        getJSON('/api/specs.json?' + q),
        getJSON('/api/tokens.json?seq=' + s.seq + '&text=' + encodeURIComponent(s.prompt)),
      ])
      specs = sp.specs; n_tokens = sp.n_tokens; toks = tk.tokens
    } catch (e) {
      status(`<span class="err">${esc(e.message)}</span>`)
      return
    }

    // refuse up front rather than let mm coerce a 501 body into NaN
    const missing = Object.entries(specs).filter(([, v]) => v.available === false)
    if (missing.length) {
      status(`<span class="err">${esc(missing[0][1].reason)}</span>`)
      return
    }

    const params = Object.assign(BASE(), view.build(specs, s))

    // refuse a tree mm would tile rather than reject
    try {
      checkShapes(params)
    } catch (e) {
      status(`<span class="err">${esc(e.message)}</span>`)
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

    history.replaceState({}, '', '?' + new URLSearchParams({
      view: s.view, layer: String(s.layer), head: String(s.head),
      stride: String(s.stride), seq: String(s.seq), anim: s.anim, prompt: s.prompt,
    }))

    // a different view is a different tree shape, so merging props onto the
    // old one would leave stale nodes behind — reload the viewer instead
    if (reload || s.view !== current_view || !mm_ready) {
      current_view = s.view
      mm_ready = false
      $('mm').src = viewer + '?params=' + encodeURIComponent(JSON.stringify(params))
    } else {
      const { cam, ...rest } = params            // keep the user's camera
      $('mm').contentWindow.postMessage({ setParams: { props: rest, reset: false } }, '*')
    }

    showStatus(s, view, specs, toks, n_tokens, params)
  }

  function showStatus(s, view, specs: Record<string, any>, toks, n_tokens, params) {
    const entries = Object.entries(specs)
    const points = countPoints(params).n

    const shapes = entries
      .map(([k, v]) => `<span class="dim">${esc(k.replace(':', '·'))}</span> ${v.h}×${v.w}`)
      .join('   ')

    // two separate claims: what the leaf data is, and what the product is
    const data = dataClaim(specs, s.stride)
    const product = productClaim(view)

    status(
      `<div style="flex-basis:100%">` +
        toks.map(t => `<span class="tok">${esc(t)}</span>`).join('') +
        `<span class="dim">${n_tokens} tokens</span></div>` +
      `<div style="flex-basis:100%"><span class="expr">${esc(view.note(s))}</span></div>` +
      `<div>${shapes}</div>` +
      `<div class="dim">${points.toLocaleString()} points</div>` +
      `<div style="flex-basis:100%"><span class="dim">data:</span> ${data}</div>` +
      `<div style="flex-basis:100%"><span class="dim">product:</span> ${product}</div>`
    )
  }

  // -- tensor map — the whole checkpoint, enumerated -----------------------

  function toggleTensors() {
    const el = $('tensors')
    const open = el.style.display == 'flex'
    el.style.display = open ? 'none' : 'flex'
    if (!open && !el.dataset.filled) {
      const rows = META.tensors.map(t =>
        `<tr><td class="n">${esc(t.name)}</td><td>${esc(t.dtype)}</td>` +
        `<td>${t.shape.join('×')}</td><td class="r">${t.bytes.toLocaleString()} B</td></tr>`).join('')
      el.innerHTML =
        `<table><tr><td colspan="4" class="r">${META.tensors.length} tensors, ` +
        `${META.checkpoint_bytes.toLocaleString()} bytes on disk — read by byte range, never loaded whole. ` +
        `A bias is a vector, so it has no matmul view of its own; it is drawn as ` +
        `the extra row or column of the weight it belongs to. LayerNorm gains and ` +
        `shifts are folded into the activations by the forward pass.</td></tr>${rows}</table>`
      el.dataset.filled = '1'
    }
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
    status(`<span class="err">cannot reach the model server (${esc(e.message)}).` +
      ` Start it with <code>../.venv/bin/python tools/gpt2_server.py</code>` +
      ` from the <code>mm/</code> directory, then open` +
      ` <code>${esc(location.pathname)}</code> on that origin` +
      ` (or run <code>npm run dev</code>, which proxies /gpt2 to it).</span>`)
    return
  }

  // a page may declare a precondition it cannot draw honestly without
  const refusal = config.require && config.require(META)
  if (refusal) {
    status(`<span class="err">${esc(refusal)}</span>`)
    return
  }

  Object.keys(views).forEach(k => option($('view'), k))
  fillRange($('layer'), META.dims.n_layer)
  fillRange($('head'), META.dims.n_head)
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

  $('mm').addEventListener('load', () => { mm_ready = $('mm').src != 'about:blank' })

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
  }

  $('view').addEventListener('change', () => { applyViewDefaults(); refresh(true) })
  $('prompt').addEventListener('change', () => refresh(false))
  ;['layer', 'head', 'stride', 'seq', 'anim'].forEach(id =>
    $(id).addEventListener('change', () => refresh(false)))
  $('tensors_link').addEventListener('click', e => { e.preventDefault(); toggleTensors() })
  $('popout_link').addEventListener('click', e => { e.preventDefault(); popout() })

  applyViewDefaults()

  // deep link: ?view=mlp+down&layer=3&head=7&stride=4&prompt=…
  const qp = new URLSearchParams(location.search)
  if (qp.has('prompt')) $('prompt').value = qp.get('prompt')
  if (views[qp.get('view')]) { $('view').value = qp.get('view'); applyViewDefaults() }
  for (const id of ['layer', 'head', 'stride', 'seq', 'anim']) {
    if (qp.has(id) && [...$(id).options].some(o => o.value == qp.get(id))) {
      $(id).value = qp.get(id)
    }
  }

  refresh(true)
}
