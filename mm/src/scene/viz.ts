"use strict"

import * as THREE from 'three'
import * as util from '../common/util.js'

//
// element rendering
//
// Elements were GL points shaded as sphere impostors. WebGPU has no sized point
// primitive, so both the geometry and that shading now live in ../render/points.js as
// an instanced quad; MATERIAL is re-exported because main.js drives the
// magnifier through its `mag` uniform.
//

import { PointCloud } from '../render/points.js'
export { MATERIAL } from '../render/points.js'

//
// The second element-render path: texture-backed heatmap.
//
// A Mat picks one of the two per matrix (see `pickRenderMode`). `elements`
// is unchanged and stays the default for small matrices -- it is what the
// magnifier lens and the spotlight labels are tuned against. `heatmap` is one
// quad per block with the matrix as an RG8 texture, which is the only way a
// 768x3072 weight is drawable at all: as instanced quads it is 2.4M elements
// and ~9.4M vertices.
//
import { HeatmapMesh } from '../render/heatmapmesh.js'
import {
  pickRenderMode, chooseLodFactor, lodSize,
  HEATMAP_TEXEL_BUDGET, HEATMAP_SCENE_TEXEL_BUDGET,
  TEXEL_HIDDEN, TEXEL_SHOWN, TEXEL_BUMPED, RENDER_MODES,
} from '../render/heatmap.js'
import { colormapLUT, elementHSL, HEATMAP_ENCODINGS, HEATMAP_REDUCERS } from '../render/colormap.js'
export { RENDER_MODES, HEATMAP_ENCODINGS, HEATMAP_REDUCERS }

//
// initialization
//

// https://stackoverflow.com/questions/25582882/javascript-math-random-normal-distribution-gaussian-bell-curve
// Standard Normal variate using Box-Muller transform.
function gaussianRandom(mean = 0, stdev = 1) {
  let u = 1 - Math.random() //Converting [0,1) to (0,1)
  let v = Math.random()
  let z = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v)
  // Transform to the desired mean and standard deviation:
  return z * stdev + mean
}

// Removed: sampleSphere(), which called `sm.randn` / `sm.sum` against a shumai
// import that was never present in this file. It would have thrown
// ReferenceError on the first call. Its only caller is the `sphere` entry in
// INIT_FUNCS below, which has been commented out for as long as the file has
// existed. The typecheck is what surfaced it.
// https://github.com/facebookresearch/shumai/blob/main/test/gradient.test.ts#L5

export const INIT_FUNCS = {
  rows: (i, j, h) => h > 1 ? i / (h - 1) : 0,
  cols: (i, j, h, w) => w > 1 ? j / (w - 1) : 0,
  'row major': (i, j, h, w) => h * w > 1 ? (i * w + j) / (h * w - 1) : 0,
  'col major': (i, j, h, w) => h * w > 1 ? (j * h + i) / (h * w - 1) : 0,
  'pt linear': (i, j, h, w) => (2 * Math.random() - 1) / Math.sqrt(w),
  uniform: () => Math.random(),
  gaussian: () => gaussianRandom(0.5, 0.5),
  // sphere: (i, j, h, w) => sampleSphere([h, w]),
  'tril mask': (i, j) => j <= i ? 1 : 0,
  'triu mask': (i, j) => j >= i ? 1 : 0,
  eye: (i, j) => +(i == j),
  diff: (i, j) => i == j ? 1 : i == j + 1 ? -1 : 0,
}

export const INITS = Object.keys(INIT_FUNCS).concat(['url', 'expr'])

const USE_RANGE = ['rows', 'cols', 'row major', 'col major', 'uniform', 'gaussian']
const USE_DROPOUT = USE_RANGE.concat(['pt linear'])

export const useRange = name => USE_RANGE.indexOf(name) >= 0
export const useDropout = name => USE_DROPOUT.indexOf(name) >= 0

const DATA_CACHE = {}

function tryLoadData(data_url) {
  if (DATA_CACHE[data_url]) {
    return DATA_CACHE[data_url]
  }
  try {
    console.log(`loading data from ${data_url}...`)
    const url = new URL(data_url)
    const req = new XMLHttpRequest()
    req.open("GET", url, false)
    req.send(null)
    // Keyed by `data_url`, the same string the lookup at the top of this
    // function uses. It used to be keyed by the URL *object*, which stringifies
    // to the normalised href -- so any caller whose URL was not already in
    // normal form wrote a key the reader could never hit and re-fetched the CSV
    // synchronously on every re-init. Found by the typecheck (`Type 'URL'
    // cannot be used as an index type`).
    DATA_CACHE[data_url] = req.responseText.split(/\r?\n|\r/).map(l => l.split(',').map(s => +s))
    console.log(`done loading data from ${data_url}`)
    return DATA_CACHE[data_url]
  } catch (e) {
    console.log(`error loading data from URL '${data_url}' message '${e.message}`)
  }
}

function tryURLInit(url) {
  const data = tryLoadData(url)
  if (data) {
    return (i, j, h, w) => {
      const row = data[i % data.length]
      return row[j % row.length]
    }
  }
}

function tryEvalInitExpr(expr) {
  try {
    return eval?.(`(i, j, h, w) => { try { return (${expr}) } catch (e) { return 0 } }`)
  } catch ({ name, message }) {
    console.log(`error ${name} evaluating init expr '${expr}' message '${message}'`)
    return () => 0
  }
}

function getInitFunc(init_params) {
  const { init, min, max, dropout, url, expr } = init_params
  const f = INIT_FUNCS[init] ||
    (init == 'url' && tryURLInit(url)) ||
    (init == 'expr' && tryEvalInitExpr(expr))
  if (!f) {
    console.log(init == 'url' ?
      `'can't load from URL '${url}'` :
      `unrecognized initializer '${init}'`)
    return () => 0
  }
  const scaled = useRange(init) && (min != 0 || max != 1) ?
    (i, j, h, w) => min + Math.max(0, max - min) * f(i, j, h, w) :
    f
  const sparse = useDropout(init) && dropout > 0 ?
    (i, j, h, w) => Math.random() > dropout ? scaled(i, j, h, w) : 0 :
    scaled
  return sparse
}

// pointwise funcs

const ERF_A1 = 0.254829592
const ERF_A2 = -0.284496736
const ERF_A3 = 1.421413741
const ERF_A4 = -1.453152027
const ERF_A5 = 1.061405429
const ERF_P = 0.3275911

function erf(x) {
  const absx = Math.abs(x)
  const t = 1.0 / (1.0 + ERF_P * absx)
  const y = (((((ERF_A5 * t + ERF_A4) * t) + ERF_A3) * t + ERF_A2) * t + ERF_A1) * t
  return Math.sign(x) * (1 - y * Math.exp(-absx * absx))
}

const SQRT2 = Math.sqrt(2)

const gelu = x => x * (1 + erf(x / SQRT2)) / 2

const sigmoid = x => 1 / (1 + Math.exp(-x))

const silu = x => x * sigmoid(x)

const relu = x => Math.max(0, x)

const pow2 = x => x ** 2

const POINTWISE = {
  'relu': relu,
  'gelu': gelu,
  'sigmoid': sigmoid,
  'silu': silu,
  'tanh': Math.tanh,
  'x**2': pow2,
}

// epilogs
// TODO the way epis are done is kind of messy rn

export const EPILOGS = [
  'none',
  'relu',
  'gelu',
  'sigmoid',
  'silu',
  'tanh',
  'layernorm',
  'softmax',
  'softmax(x/sqrt(k))',
  'softmax(tril(x/sqrt(k)))',
  'softmax(tril(x/8))',
  'x/k',
  'x/sqrt(k)',
  'x**2',
]

function softmax_(h, w, data, tril = false) {

  const row_max = (ptr, w) => {
    let x = 0
    for (let j = 0; j < w; j++, ptr++) {
      x = Math.max(x, data[ptr])
    }
    return x
  }

  const calc_denom = (ptr, w, rmax) => {
    let d = 0
    for (let j = 0; j < w; j++, ptr++) {
      d += Math.exp(data[ptr] - rmax)
      if (!isFinite(d)) {
        // console.log(`HEY denom at data[${ptr}) = ${data[ptr]} becomes infinite`)
        break
      }
    }
    return d
  }

  for (let i = 0, ptr = 0; i < h; i++) {
    const rmax = row_max(ptr, tril ? i + 1 : w)
    const denom = calc_denom(ptr, tril ? i + 1 : w, rmax)
    for (let j = 0; j < w; j++, ptr++) {
      const x = tril && j > i ? 0 : Math.exp(data[ptr] - rmax) / denom
      if (isNaN(x)) {
        // console.log(`HEY Math.exp(data[${ptr}) = ${data[ptr]}]) / ${denom} is NaN`)
        data[ptr] = 0
      } else {
        data[ptr] = x
      }
    }
  }
}

const softmax_tril_ = (h, w, data) => softmax_(h, w, data, true)

function layernorm_(h, w, data) {
  const mean = data.reduce((acc, x) => acc + x) / data.length
  const mean2 = data.map(x => x ** 2).reduce((acc, x) => acc + x) / data.length
  const variance = mean2 - mean ** 2
  const denom = Math.sqrt(variance + 1e-5)
  const n = h * w
  for (let ptr = 0; ptr < n; ptr++) {
    const x = data[ptr]
    data[ptr] = (x - mean) / denom
  }
}

const IN_PLACE_EPILOGS = {
  'softmax': softmax_,
  'softmax(x/sqrt(k))': softmax_,
  'softmax(tril(x/sqrt(k)))': softmax_tril_,
  'softmax(tril(x/8))': softmax_tril_, // TODO remove with epi cleanup
  'layernorm': layernorm_,
}

const getInPlaceEpilog = name => IN_PLACE_EPILOGS[name]

function applyInPlaceEpilog_(data, h, w, epi) {
  const epi_ = epi && getInPlaceEpilog(epi)
  if (epi_) {
    epi_(h, w, data)
  }
}

//
// Array2D
//

function toRange(x, n) {
  return x === undefined ? [0, n] : x.constructor === Array ? x : [x, x + 1]
}

function initArrayData_(data, h, w, init, epi = undefined, r = undefined, c = undefined) {
  const [rstart, rend] = toRange(r, h)
  const [cstart, cend] = toRange(c, w)
  for (let i = rstart; i < rend; i++) {
    for (let j = cstart, ptr = i * w + cstart; j < cend; j++, ptr++) {
      data[ptr] = init(i, j, h, w)
    }
  }
  applyInPlaceEpilog_(data, h, w, epi)
}

export class Array2D {
  // Fields, declared for the typechecker. Most are assigned in init()
  // rather than the constructor, so TS cannot infer them; without these,
  // every read is an error and a genuine typo would hide among them.
  data: any
  h: any
  w: any


  static fromInit(h, w, init, epi = undefined) {
    const data = new Float32Array(h * w)
    initArrayData_(data, h, w, init, epi)
    return new Array2D(h, w, data)
  }

  constructor(h, w, data) {
    this.h = h | 0
    this.w = w | 0
    this.data = data
  }

  reinit(f, epi = undefined, r = undefined, c = undefined) {
    initArrayData_(this.data, this.h, this.w, f, epi, r, c)
  }

  numel() {
    return this.h * this.w
  }

  get(i, j) {
    return this.data[this.addr(i, j)]
  }

  slice(i = undefined, j = undefined) {
    const [istart, iend] = toRange(i, this.h)
    const [jstart, jend] = toRange(j, this.w)
    const init = (i, j, h, w) => this.get(istart + i, jstart + j)
    return Array2D.fromInit(iend - istart, jend - jstart, init)
  }

  addr(i, j) {
    return i * this.w + j
  }

  absmax() {
    const data = this.data
    let absmax = 0
    for (let i = 0; i < data.length; i++) {
      const absx = Math.abs(data[i])
      if (absmax < absx) {
        absmax = absx
      }
    }
    return absmax
  }

  absmin() {
    const data = this.data
    let absmin = Infinity
    for (let i = 0; i < data.length; i++) {
      const absx = Math.abs(data[i])
      if (!isFinite(absmin) || absx < absmin) {
        absmin = absx
      }
    }
    return absmin
  }

  transpose() {
    return Array2D.fromInit(this.w, this.h, (i, j) => this.get(j, i))
  }

  map(f) {
    // `n` was never declared here, so this threw ReferenceError on every call.
    // Nothing in the app reaches it -- which is why it survived -- but leaving
    // it would have been a hard typecheck error, and a dead method that crashes
    // is a trap for the next person who finds it. Declared as map2 does.
    const n = this.h * this.w
    const data = new Float32Array(n)
    for (let ptr = 0; ptr < n; ptr++) {
      data[ptr] = f(this.data[ptr])
    }
    return new Array2D(this.h, this.w, data)
  }

  map2(f, a) {
    if (a.h != this.h || a.w != this.w) {
      throw Error(`shape error: this ${this.h} ${this.w} a ${a.h} ${a.w}`)
    }
    const n = this.h * this.w
    const data = new Float32Array(n)
    for (let ptr = 0; ptr < n; ptr++) {
      data[ptr] = f(this.data[ptr], a.data[ptr])
    }
    return new Array2D(this.h, this.w, data)
  }

  add(a) {
    return this.map2((x, y) => x + y, a)
  }
}

//
//
//

function grid(info, dims, f) {
  const infos = Array.from(dims as Iterable<string>).map(d => info[d])
  const loop = (args, infos, f) => infos.length == 0 ?
    f(...args) :
    [...Array(infos[0].n).keys()].map(index => {
      const { size, max } = infos[0]
      const start = index * size
      if (start < max) {  // dead final block when size * n - max > size
        const end = Math.min(start + size, max)
        const extent = end - start
        loop([...args, { index, start, end, extent }], infos.slice(1), f)
      }
    })
  loop([], infos, f)
}

//
// Mat
//

let elem_scale = 1.25
let elem_size = elem_scale

function setElemScale(s) {
  s ||= elem_scale
  const old_elem_scale = elem_scale
  elem_scale = s
  elem_size *= elem_scale / old_elem_scale
}

export function setElemSize(scale, pixel_ratio) {
  elem_size = elem_scale * Math.min(scale.x, scale.y) * pixel_ratio
}

const ZERO_COLOR = new THREE.Color(0, 0, 0)
const COLOR_TEMP = new THREE.Color()

// Per-axis lattice spacing. Independent knobs because one number does not read
// well across a matmul: a 64 x 3072 weight wants a line every 8 rows and every
// 128 columns, and a single spacing makes one of the two a solid sheet.
const gridSpacing = deco => ({
  i: deco['grid spacing i'] || 8,
  j: deco['grid spacing j'] || 8,
  k: deco['grid spacing k'] || 8,
})

// Exported so the heatmap path's texel placement can be asserted against the
// element path's own attribute rather than against a restatement of this
// arithmetic. A heatmap that silently transposes or flips is the failure
// nobody notices; see test/points.test.ts.
export function emptyPoints(h, w, info) {
  const { i: { size: si }, j: { size: sj }, gap } = info
  const n = h * w
  const points = new Float32Array(n * 3)
  for (let i = 0, ptr = 0; i < h; i++) {
    const ioff = Math.floor(i / si)
    for (let j = 0; j < w; j++) {
      const joff = Math.floor(j / sj)
      points[ptr++] = j + joff * gap
      points[ptr++] = i + ioff * gap
      points[ptr++] = 0
    }
  }
  // `points` are the element centres. PointCloud makes them the per-instance
  // attribute and keeps `pointSize` / `pointColor` addressable exactly as
  // before, so setSize/setColor/updateLabels below are untouched by the move
  // off GL points.
  return new PointCloud(points, n)
}

export class Mat {
  // Fields, declared for the typechecker. Most are assigned in init()
  // rather than the constructor, so TS cannot infer them; without these,
  // every read is an error and a genuine typo would hide among them.
  H: any
  W: any
  _extents: any
  absmax: any
  absmin: any
  _global_absmax: any
  _range: any
  align_grid_group: any
  context: any
  data: any
  group: any
  hdim_text: any
  inner_group: any
  label_cache: any
  label_group: any
  legend_state: any
  n_size_from_data_errors: any
  name_text: any
  params: any
  points: any
  row_guide_groups: any
  wdim_text: any

  // heatmap mode. `heat` is null in elements mode and is the only thing any
  // mode-aware branch below tests, so there is exactly one place the two paths
  // can disagree about which one is live.
  heat: any
  mode: string


  constructor(data, params, context, init_viz) {
    this.params = params
    this.context = context

    this.data = data
    this.H = data.h
    this.W = data.w
    this.absmax = this.data.absmax()
    this.absmin = this.data.absmin()

    if (init_viz) {
      this.initViz()
    }
  }

  getBlockInfo() {
    const ni = Math.min(this.params.block['i blocks'], this.H)
    const nj = Math.min(this.params.block['j blocks'], this.W)
    return {
      i: { n: ni, size: Math.ceil(this.H / ni), max: this.H },
      j: { n: nj, size: Math.ceil(this.W / nj), max: this.W },
    }
  }

  grid(dims, f) {
    grid(this.getBlockInfo(), dims, f)
  }

  getDispH() {
    const { i: { n, size } } = this.getBlockInfo()
    return this.H + this.params.layout.gap * (Math.min(n, Math.ceil(this.H / size)) - 1)
  }

  getDispW() {
    const { j: { n, size } } = this.getBlockInfo()
    return this.W + this.params.layout.gap * (Math.min(n, Math.ceil(this.W / size)) - 1)
  }

  //
  // heatmap mode
  //

  /** The per-matrix texel cap in force. C3's scene budget lowers it. */
  getTexelBudget() {
    const b = this.params.viz['texel budget']
    return b > 0 ? b : HEATMAP_TEXEL_BUDGET
  }

  getEncoding() {
    return this.params.viz['heatmap encoding'] || 'magnitude'
  }

  /** What the picture claims, for the status bar. Never inferred by a caller. */
  getHeatmapInfo() {
    if (!this.heat) return null
    const { h, w } = lodSize(this.H, this.W, this.heat.f)
    return {
      lod: this.heat.f, encoding: this.heat.enc, reducer: this.heat.op,
      texels: this.heat.texels, h, w,
    }
  }

  buildHeatmap(info) {
    const viz = this.params.viz
    // The viewport bound is a real footprint bound and a stable one; see
    // chooseLodFactor for why it is not the live projected size.
    const screen_px = this.context.screenPx || Infinity
    const lod = chooseLodFactor(this.H, this.W, this.getTexelBudget(), screen_px)
    this.heat = new HeatmapMesh(this.H, this.W, info, {
      lod,
      reducer: viz['lod reduce'] || 'maxAbs',
      encoding: this.getEncoding(),
      linear: viz['heatmap filter'] === 'linear',
    })
    this.refreshLUT()
    this.paint([0, this.H], [0, this.W], TEXEL_SHOWN)
    return this.heat
  }

  /**
   * Rebuild the ramp lookup. Kept explicit rather than left to the full
   * rebuild `gui.ts` happens to trigger for `sensitivity`: the LUT depends on
   * getRangeInfo() and on four viz knobs, and a texture encoded once against a
   * range that has since moved is a wrong picture with nothing to say so.
   */
  refreshLUT() {
    if (!this.heat) return
    const { absmin, absmax } = this.getRangeInfo()
    this.heat.setLUT(colormapLUT(
      this.getEncoding(), absmin, absmax, this.params.viz,
      (h, s, l) => { COLOR_TEMP.setHSL(h, s, l); return [COLOR_TEMP.r * 255, COLOR_TEMP.g * 255, COLOR_TEMP.b * 255] }))
  }

  /** Write the value channel over a range, and set the cells' state. */
  paint(r, c, state) {
    const rr = toRange(r, this.H), cc = toRange(c, this.W)
    const { absmin, absmax } = this.getRangeInfo()
    if (state !== TEXEL_HIDDEN) {
      this.heat.writeValues(this.data.data, rr, cc, this.getEncoding(), absmin, absmax)
    }
    this.heat.writeState(state, rr, cc)
    // Labels cache by element and carry the value they were built from. In
    // elements mode checkLabel() invalidates them one at a time as the loop
    // passes; here there is no such loop, and iterating H*W to invalidate
    // would defeat the point of the mode. Dropping the cache is O(1) and the
    // labels are rebuilt on the next hover anyway.
    if (this.label_cache) this.label_cache = []
  }

  initViz() {
    const gap = this.params.layout.gap
    const info = { ...this.getBlockInfo(), gap }

    this._range = null
    this.heat = null
    this.mode = pickRenderMode(this.H, this.W, this.params.viz['render mode'] || 'auto')
    this.points = this.mode === 'heatmap' ?
      this.buildHeatmap(info) :
      emptyPoints(this.H, this.W, info)
    this.points.name = `${this.params.name}.points`

    if (!this.heat) {
      this.setColorsAndSizes()
    }

    this.inner_group = new THREE.Group()
    this.inner_group.name = `${this.params.name}.inner_group`
    this.inner_group.add(this.points)

    util.updateProps(this.inner_group.position, { x: gap, y: gap })

    this.group = new THREE.Group()
    this.group.name = `${this.params.name}.group`
    this.group.add(this.inner_group)

    this.setLegends()
  }

  setColorsAndSizes(r = undefined, c = undefined, get_size = undefined, get_color = undefined) {
    if (this.heat) {
      // The two overrides that exist -- hide's `_ => ZERO_COLOR` and
      // bumpColor's `+ 0x808080` -- are both colour-as-state, and both are
      // handled as state below rather than as colour. So every remaining
      // caller of this method means "restore these cells to their data", which
      // is what this does. Nothing silently no-ops.
      this.paint(r, c, TEXEL_SHOWN)
      return
    }
    const [rstart, rend] = toRange(r, this.H)
    const [cstart, cend] = toRange(c, this.W)
    get_size = get_size || this.sizeFromData.bind(this)
    get_color = get_color || this.colorFromData.bind(this)
    for (let i = rstart; i < rend; i++) {
      for (let j = cstart; j < cend; j++) {
        const x = this.getData(i, j)
        this.setSize(i, j, get_size(x))
        this.setColor(i, j, get_color(x))
        this.checkLabel(i, j, x)
      }
    }
  }

  getExtent() {
    const gap = this.params.layout.gap
    return this._extents || (this._extents = {
      x: this.getDispW() + 2 * gap - 1,
      y: this.getDispH() + 2 * gap - 1,
      z: 0,
    })
  }

  // Cached: setColorsAndSizes calls this twice per element (sizeFromData and
  // colorFromData each ask), and it allocated a fresh object every time. It is
  // a function of params.viz and of this matrix's own absmin/absmax, neither of
  // which moves without a rebuild -- so the cache is dropped in initViz() and
  // in setVizProp(), which are the two places those can change.
  getRangeInfo() {
    return this._range ??= this.computeRangeInfo()
  }

  computeRangeInfo() {
    const viz = this.params.viz
    const use_absmin = viz.sensitivity == 'superlocal'

    const local_absmax = this.absmax
    const global_absmax = this.getGlobalAbsmax()
    const absmax = (use_absmin || viz.sensitivity == 'local') ? local_absmax :
      viz.sensitivity == 'global' ? global_absmax :
        Math.sqrt(local_absmax * global_absmax) // semilocal
    const absmin = use_absmin ? this.absmin : 0
    const absdiff = absmax - absmin
    if (absmin > absmax) {
      console.log(`HEY absmin ${absmin} > absmax ${absmax}`)
    }
    return { viz, absmin, absmax, absdiff }
  }

  sizeFromData(x) {
    if (x === undefined || isNaN(x)) {
      console.log(`HEY sizeFromData(${x})`)
      return 0
    }

    if (x === 0) {
      return 0
    }

    const absx = Math.abs(x)
    if (absx === Infinity) {
      return elem_size
    }

    const { viz, absmin, absmax, absdiff } = this.getRangeInfo()
    const vol = absmax <= absmin ? 0 : (absx - absmin) / absdiff
    const zsize = viz['min size'] * elem_size
    const size = zsize + (elem_size - zsize) * Math.sqrt(vol)

    if (isNaN(size)) {
      this.n_size_from_data_errors = (this.n_size_from_data_errors || 0) + 1
      if (this.n_size_from_data_errors <= 100) {
        console.log(`HEY x ${x} size ${size} absx ${absx} absmax ${absmax} absmin ${absmin} zsize ${zsize}`)
        if (this.n_size_from_data_errors == 100) {
          console.log(`HEY stopping logging after 100 errors`)
        }
      }
    }

    // boundary violations can happen in intermediates
    return Math.min(size, elem_size)
  }

  colorFromData(x) {
    if (x === undefined || isNaN(x)) {
      console.log(`HEY colorFromData(${x})`)
      return COLOR_TEMP.setHSL(0.0, 1.0, 1.0)
    }

    if (x === 0) {
      return COLOR_TEMP.setHSL(0.0, 1.0, 0.0)
    }

    if (Math.abs(x) === Infinity) {
      return COLOR_TEMP.setHSL(1.0, 1.0, 1.0)
    }

    // The arithmetic itself lives in colormap.ts, as a pure function, because
    // the heatmap's 'signed' ramp lookup has to produce exactly these colours.
    // Two copies of it would drift, and the drift would show up as two modes
    // disagreeing about what a weight looks like.
    const range = this.getRangeInfo()
    const hsl = elementHSL(x, range, range.viz)
    return hsl ? COLOR_TEMP.setHSL(hsl.h, hsl.s, hsl.l) : COLOR_TEMP.setHSL(0.0, 1.0, 0.0)
  }

  getAbsmax() {
    return this.absmax
  }

  /**
   * What the colours on screen currently mean, for the colorbar and the status
   * bar. Aggregated up the tree by MatMul below.
   *
   * `lod` is the coarsest level any matrix in the subtree is showing. It is
   * reported rather than left implicit because a reduced heatmap is not exact,
   * and a picture that is one maxAbs per 16 cells must not print as though
   * every cell were its own.
   */
  getVizSummary() {
    const { absmin, absmax } = this.getRangeInfo()
    const hm = this.getHeatmapInfo()
    return {
      absmin, absmax, mats: 1,
      encoding: hm ? hm.encoding : null,
      reducer: hm ? hm.reducer : null,
      lod: hm ? hm.lod : 1,
      texels: hm ? hm.texels : 0,
      elements: hm ? 0 : this.H * this.W,
      heatmaps: hm ? 1 : 0,
    }
  }

  // Memoized, and it has to be.
  //
  // getRangeInfo() calls this once *per element* -- from both sizeFromData and
  // colorFromData -- and `params.getGlobalAbsmax` climbs to the root of the
  // tree, whose getAbsmax() walks every matrix under it. In a four-node matmul
  // that is a few calls per element and merely wasteful. In a 25-stage model
  // scene the root walks 159 matrices, so setColorsAndSizes over a 250,000
  // element stage did 40 million subtree traversals and took six seconds.
  //
  // Nothing invalidates it, because nothing moves it: `absmax` is measured in
  // the constructor and no path recomputes it (reinit deliberately leaves it
  // alone, which is why intermediates can exceed their own range -- see the
  // "boundary violations" clamps above).
  getGlobalAbsmax() {
    return this._global_absmax ??=
      (this.params.getGlobalAbsmax ? this.params.getGlobalAbsmax() : this.absmax)
  }

  // Params were copied down at construction, so a viz knob set on the root
  // after the fact reaches nothing. This is how C3's budget is enforced in
  // practice: Stack.setStage sets 'texel budget' and 'render mode' on one
  // stage and then rebuilds it, so full-resolution texels and the sphere path
  // are spent on the active stage and nowhere else.
  setVizProp(k, v) {
    this.params.viz[k] = v
    this._range = null
  }

  reinit(init, epi = undefined, r = undefined, c = undefined) {
    this.data.reinit(init, epi, r, c)
    this.setColorsAndSizes(r, c)
  }

  getDataArray() {
    return this.data.data
  }

  getData(i, j) {
    if (i >= this.H || j >= this.W) {
      console.log(`HEY i ${i} >= this.H ${this.H} || j ${j} >= this.W ${this.W}`)
      return 0
    }
    return this.data.get(i, j)
  }

  getColor(i, j) {
    if (this.heat) {
      // The ramp entry this cell is showing. Not the element's value -- that is
      // getData()'s job and comes from the FP32 Array2D.
      const lut = this.heat.lutTex.image.data
      const b = this.heat.byteAt(i, j) * 4
      return COLOR_TEMP.setRGB(lut[b] / 255, lut[b + 1] / 255, lut[b + 2] / 255)
    }
    const colors = this.points.geometry.attributes.pointColor.array
    return COLOR_TEMP.fromArray(colors, this.data.addr(i, j) * 3)
  }

  setColor(i, j, c) {
    if (this.heat) {
      // Deliberately loud rather than a no-op: a heatmap cell's colour is the
      // ramp entry for its value, so there is nothing an arbitrary colour could
      // honestly mean here. Nothing in viz.ts reaches this in heatmap mode --
      // setColorsAndSizes above short-circuits first.
      throw new Error('setColor is an elements-mode operation; a heatmap cell ' +
        'takes its colour from the ramp. Use show/hide/bumpColor.')
    }
    const colors = this.points.geometry.attributes.pointColor.array
    c.toArray(colors, this.data.addr(i, j) * 3)
    this.points.geometry.attributes.pointColor.needsUpdate = true
  }

  getSize(i, j) {
    if (this.heat) {
      throw new Error('getSize is an elements-mode operation; heatmap cells are ' +
        'contiguous texels and have no per-element size')
    }
    return this.points.geometry.attributes.pointSize.array[this.data.addr(i, j)]
  }

  setSize(i, j, x) {
    if (this.heat) {
      throw new Error('setSize is an elements-mode operation; heatmap cells are ' +
        'contiguous texels and have no per-element size')
    }
    this.points.geometry.attributes.pointSize.array[this.data.addr(i, j)] = x
    this.points.geometry.attributes.pointSize.needsUpdate = true
  }

  show(r = undefined, c = undefined) {
    this.heat ? this.paint(r, c, TEXEL_SHOWN) : this.setColorsAndSizes(r, c)
  }

  hide(r = undefined, c = undefined) {
    this.heat ?
      this.paint(r, c, TEXEL_HIDDEN) :
      this.setColorsAndSizes(r, c, _ => 0, _ => ZERO_COLOR)
  }

  isHidden(i, j) {
    // Elements mode reads visibility out of the colour. Heatmap mode cannot:
    // the ramp's low stop is #03051A, so a hidden cell and the smallest value
    // in the matrix would be one byte apart and the animation would look like
    // it was skipping cells. Hence the separate state channel.
    return this.heat ?
      this.heat.stateAt(i, j) === TEXEL_HIDDEN :
      this.getColor(i, j).equals(ZERO_COLOR)
  }

  bumpColor(r = undefined, c = undefined) {
    if (this.heat) {
      this.paint(r, c, TEXEL_BUMPED)
      return
    }
    COLOR_TEMP.set(0x808080)
    this.setColorsAndSizes(r, c, undefined, x => this.colorFromData(x).add(COLOR_TEMP))
  }

  isFacing() {
    const c = this.group.localToWorld(new THREE.Vector3()).sub(this.context.camera.position).normalize()
    const m = this.group.getWorldDirection(new THREE.Vector3())
    return m.angleTo(c) < Math.PI / 2
  }

  isRightSideUp() {
    const q = new THREE.Quaternion()
    const p = new THREE.Vector3(0, -1, 0).applyQuaternion(this.group.getWorldQuaternion(q))
    const c = new THREE.Vector3(0, 1, 0).applyQuaternion(this.context.camera.quaternion)
    return p.angleTo(c) < Math.PI / 2
  }

  setRowGuides(light = undefined) {
    const prev = this.params.deco['row guides']
    light = util.syncProp(this.params.deco, 'row guides', light)
    if (this.row_guide_groups && prev == light) {
      return
    }
    if (this.row_guide_groups) {
      this.row_guide_groups.forEach(g => {
        this.inner_group.remove(g)
        util.disposeAndClear(g)
      })
    }
    this.row_guide_groups = []
    if (light > 0.0) {
      const gap = this.params.layout.gap
      this.grid('ij', (
        { start: i, extent: ix, index: ii },
        { start: j, extent: jx, index: ji }
      ) => {
        const g = util.rowGuide(ix, jx, light)
        util.updateProps(g.position, { x: j + ji * gap, y: i + ii * gap })
        this.inner_group.add(g)
        this.row_guide_groups.push(g)
      })
    }
  }

  setFlowGuide(light) { }

  // The alignment lattice, in this matrix's own plane so it lines up with this
  // matrix's own elements wherever the layout has put it. `d` is 0: a Mat is
  // one face. The enclosing MatMul draws the box that gives the third axis.
  setAlignGrid(light = undefined) {
    const prev = this.params.deco.grid
    light = util.syncProp(this.params.deco, 'grid', light)
    if (this.align_grid_group && prev == light) {
      return
    }
    if (this.align_grid_group) {
      this.inner_group.remove(this.align_grid_group)
      util.disposeAndClear(this.align_grid_group)
      this.align_grid_group = undefined
    }
    if (light > 0.0) {
      this.align_grid_group = util.alignGrid(
        this.H, this.W, 0, gridSpacing(this.params.deco), light,
        this.getBlockInfo(), this.params.layout.gap)
      this.inner_group.add(this.align_grid_group)
    }
  }

  setName(name) {
    util.syncProp(this.params, 'name', name)
    this.setLegends()
  }

  setLegends(size = undefined, shape = undefined) {
    shape = util.syncProp(this.params.deco, 'shape', shape)
    const facing = this.isFacing()
    const rsu = this.isRightSideUp()
    const [H, W] = [this.H, this.W]
    const name = this.params.name // && this.params.name + (shape ? ` [${H}, ${W}]` : '')

    if ((size === undefined || size == this.params.deco.legends) &&
      this.legend_state &&
      this.legend_state.facing == facing &&
      this.legend_state.rsu == rsu &&
      this.legend_state.name == name &&
      this.legend_state.shape == shape &&
      this.legend_state.H == H && this.legend_state.W == W) {
      return
    }

    size = util.syncProp(this.params.deco, 'legends', size)
    this.legend_state = { facing, rsu, name, shape, H, W }
    const rmv = x => {
      if (x) {
        this.inner_group.remove(x)
        util.disposeAndClear(x)
      }
    }
    rmv(this.name_text)
    rmv(this.hdim_text)
    rmv(this.wdim_text)

    if (size > 0) {
      const color = 0xCCCCFF
      const adjsiz = size * Math.cbrt(H * W) / 10
      const xdir = facing ? 1 : -1
      const ydir = rsu ? 1 : 0
      const zdir = facing ? 1 : -1
      if (name) {
        const adjsiz2 = adjsiz * Math.min(1, 8 / name.length)
        this.name_text = util.getText(name, color, adjsiz2)
        this.name_text.name = `${name}.name`
        this.name_text.geometry.rotateZ(Math.PI)
        this.name_text.geometry.rotateY(facing ? Math.PI : 0)
        const { h, w } = util.gbbhwd(this.name_text.geometry)
        this.name_text.geometry.translate(
          util.center(this.getDispW() - 1, xdir * w),
          h + util.center(this.getDispH() - 1, h),
          -zdir
        )
        this.inner_group.add(this.name_text)
      }
      if (shape && this.params.deco.shape_info) {
        const htext = util.getText("X", color, adjsiz / 2.5)
        const { h } = util.gbbhwd(htext.geometry)
        util.disposeAndClear(htext)
        const { i: { n: ni }, j: { n: nj } } = this.getBlockInfo()
        {
          const { h: { name, place } } = this.params.deco.shape_info
          const hdim_str = `${name} = ${H}` + (ni == 1 ? '' : ` / ${ni}`)
          this.hdim_text = util.getText(hdim_str, color, adjsiz / 2.5)
          const { w } = util.gbbhwd(this.hdim_text.geometry)
          this.hdim_text.geometry.rotateZ((place == facing ? 1 : -1) * Math.PI / 2)
          this.hdim_text.geometry.rotateY(facing ? Math.PI : 0)
          const xgap = 2 * h
          this.hdim_text.geometry.translate(
            place ? this.getDispW() - 1 + xgap : -xgap,
            (place == facing ? 0 : w) + util.center(this.getDispH() - 1, w),
            0
          )
          this.inner_group.add(this.hdim_text)
        }
        {
          const { w: { name, place } } = this.params.deco.shape_info
          const wdim_str = `${name} = ${W}` + (nj == 1 ? '' : ` / ${nj}`)
          this.wdim_text = util.getText(wdim_str, color, adjsiz / 2.5)
          const { w } = util.gbbhwd(this.wdim_text.geometry)
          this.wdim_text.name = `${name}.wdim`
          this.wdim_text.geometry.rotateZ(Math.PI)
          this.wdim_text.geometry.rotateY(facing ? Math.PI : 0)
          this.wdim_text.geometry.translate(
            util.center(this.getDispW() - 1, (facing ? 1 : -1) * w),
            place ? this.getDispH() - 1 + 3 * h : -2 * h,
            0
          )
          this.inner_group.add(this.wdim_text)
        }
      }
    }
  }

  checkLabel(i, j, x) {
    if (this.label_cache) {
      const addr = this.data.addr(i, j)
      const label = this.label_cache[addr]
      if (label != undefined && label.value != x) {
        util.disposeAndClear(label)
        this.label_cache[addr] = undefined
      }
    }
  }

  updateLabels(spotlight = undefined) {
    spotlight = util.syncProp(this.params.deco, 'spotlight', spotlight)
    if (spotlight == 0) {
      if (this.label_group) {
        this.inner_group.remove(this.label_group)
        util.disposeAndClear(this.label_group)
        this.label_group = undefined
      }
    } else {
      if (!this.label_group) {
        this.label_group = new THREE.Group()
        this.label_group.name = `${this.params.name}.label_group`
        this.inner_group.add(this.label_group)
        this.label_cache = []
      } else {
        util.disposeAndClear(this.label_group)
      }
      const gap = this.params.layout.gap
      const { i: { size: si }, j: { size: sj } } = this.getBlockInfo()
      this.context.raycaster.params.Points.threshold = spotlight
      const intersects = this.context.raycaster.intersectObject(this.points)
      let count = 0
      intersects.forEach(p => {
        const index = p.index
        const i = Math.floor(index / this.W)
        const j = index % this.W
        if (!this.isHidden(i, j)) {
          const x = this.getData(i, j)
          let label = this.label_cache[index]
          const facing = this.isFacing()
          const rsu = this.isRightSideUp()
          if (!label || label.facing != facing || label.rsu != rsu) {
            const fsiz = isNaN(x) || !isFinite(x) ? 0.12 :
              0.16 - 0.008 * Math.log10(Math.floor(1 + Math.abs(x)))
            label = util.getText(x.toFixed(5), 0xffffff, fsiz)
            count += 1
            // label.name = `${this.params.name}.label[${i}, ${j}]`
            label.value = x
            label.facing = facing
            label.rsu = rsu
            const zdir = facing ? 1 : -1
            label.geometry.rotateX(zdir * Math.PI)
            label.geometry.rotateY(facing ? 0 : Math.PI)
            label.geometry.rotateZ(rsu ? 0 : Math.PI)
            const { h, w } = util.gbbhwd(label.geometry)
            const disp_i = i + Math.floor(i / si) * gap
            const disp_j = j + Math.floor(j / sj) * gap
            label.geometry.translate(
              util.center(disp_j * 2, (rsu ? zdir : -zdir) * w),
              h + util.center(disp_i * 2, h),
              -zdir * 0.5
            )
            this.label_cache[index] = label
          }
          this.label_group.add(label)
        }
      })
    }
  }
}

//
// MatMul
//

export const SCHEMES = ['blocks', 'zigzag', 'wheel', 'custom']
export const POLARITIES = ['negative', 'positive']
export const LEFT_PLACEMENTS = ['left', 'right']
export const RIGHT_PLACEMENTS = ['top', 'bottom']
export const RESULT_PLACEMENTS = ['front', 'back']
// Which way a Stack's rows advance. Read only by Stack.layoutStages — a matmul
// has no rows — but it lives in `layout` with the rest of the arrangement.
export const ROW_FLOWS = ['vertical', 'horizontal']

function layoutDesc(layout) {
  const pol = { 'positive': '+', 'negative': '-', }[layout.polarity]
  const lfp = { 'left': 'L', 'right': 'R', }[layout['left placement']]
  const rtp = { 'top': 'T', 'bottom': 'B', }[layout['right placement']]
  const rsp = { 'front': 'F', 'back': 'B', }[layout['result placement']]
  return `${pol}${lfp}${rtp}${rsp}`
}

export const SENSITIVITIES = ['global', 'semilocal', 'local', 'superlocal']
export const TOP_LEVEL_ANIM_ALGS = [
  'none', 'dotprod (row major)', 'dotprod (col major)', 'axpy', 'vmprod', 'mvprod', 'vvprod',
]
export const ANIM_ALGS = TOP_LEVEL_ANIM_ALGS.concat('inherit')
export const FUSE_MODE = ['none', 'sync', 'async']

/**
 * Fold subtree summaries into one. `lod` takes the *coarsest* level in the
 * subtree, never the average and never the finest: what a viewer needs to know
 * is whether anything on screen is reduced, and by how much at worst.
 * `encoding` collapses to null when the subtree disagrees, so the colorbar
 * says "mixed" rather than picking one and implying it holds everywhere.
 */
export function mergeVizSummaries(parts) {
  // null means "this matrix is drawn as elements and has no ramp", which is not
  // a disagreement -- a scene of 12 sphere matrices and 147 magnitude heatmaps
  // is a magnitude scene, and calling it 'mixed' would describe a ramp nothing
  // is using. Only two *stated* values that differ make it mixed.
  const fold = (x, y) => x === y ? x : x == null ? y : y == null ? x : 'mixed'
  return parts.reduce((a, b) => ({
    absmin: Math.min(a.absmin, b.absmin),
    absmax: Math.max(a.absmax, b.absmax),
    mats: a.mats + b.mats,
    encoding: fold(a.encoding, b.encoding),
    reducer: fold(a.reducer, b.reducer),
    lod: Math.max(a.lod, b.lod),
    texels: a.texels + b.texels,
    elements: a.elements + b.elements,
    heatmaps: a.heatmaps + b.heatmaps,
  }))
}

// Children of a node, whatever kind it is. One place that knows the shapes, so
// a new node kind cannot half-exist -- the recursions below all go through it.
export const childParamsOf = p =>
  p.op === 'stack' ? Object.values(p.stages || {}) :
    p.op === 'unary' ? [p.input] :
      p.matmul === false ? [] :
        [p.left, p.right].filter(Boolean)

const ensureChildCounts = p => {
  if (p.count === undefined) {
    p.count = p.matmul === false ? 0 :
      childParamsOf(p).reduce((n, c) => n + ensureChildCounts(c).count, 1)
    // sloppy - this means root. `!p.op` keeps an `add` node -- which has no
    // `matmul` key either -- from being mistaken for one and blowing away the
    // real root's `total`.
    if (p.matmul === undefined && !p.op) {
      const total = p.count
      const setTotal = p => {
        p.total = total
        childParamsOf(p).forEach(setTotal)
      }
      setTotal(p)
    }
  }
  return p
}

export class MatMul {
  // Fields, declared for the typechecker. Most are assigned in init()
  // rather than the constructor, so TS cannot infer them; without these,
  // every read is an error and a genuine typo would hide among them.
  D: any
  H: any
  W: any
  _extents: any
  alg_join: any
  _absmax: any
  align_grid_group: any
  anim_mats: any
  bump: any
  context: any
  flow_guide_group: any
  getIndex: any
  group: any
  left: any
  onAnimDone: any
  params: any
  result: any
  right: any


  constructor(params, context, init_viz = true) {
    this.context = context

    this.params = util.copyTree(params)
    ensureChildCounts(this.params)

    this.group = new THREE.Group()
    this.group.name = `${this.params.name}.group`

    // nodeHeight/nodeWidth, not a local matmul-only pair: an operand may now
    // be a materialized unary stage or a residual add, and a shape helper that
    // did not know that would silently read `undefined` off it.
    this.H = nodeHeight(params.left)
    this.D = nodeWidth(params.left)
    this.W = nodeWidth(params.right)

    if (this.D != nodeHeight(params.right)) {
      console.log(`HEY left width ${this.D} != right height ${nodeHeight(params.right)}`)
    }

    this.initLeft()
    this.initRight()
    this.initResult()

    if (init_viz) {
      this.initViz()
    }
  }

  getDispH() {
    const { i: { n, size } } = this.getBlockInfo()
    return this.H + this.params.layout.gap * (Math.min(n, Math.ceil(this.H / size)) - 1)
  }

  getDispD() {
    const { k: { n, size } } = this.getBlockInfo()
    return this.D + this.params.layout.gap * (Math.min(n, Math.ceil(this.D / size)) - 1)
  }

  getDispW() {
    const { j: { n, size } } = this.getBlockInfo()
    return this.W + this.params.layout.gap * (Math.min(n, Math.ceil(this.W / size)) - 1)
  }

  disposeAll() {
    util.disposeAndClear(this.group)
  }

  prepChildParams(base = undefined) {
    base ||= util.copyTree(this.params)
    return {
      ...base,
      ...(base != this.params ? {
        anim: { ...this.params.anim, ...base.anim || {} },
        block: { ...this.params.block, ...base.block || {} },
        deco: { ...this.params.deco, ...base.deco || {} },
        layout: { ...this.params.layout, ...base.layout || {} },
        viz: { ...this.params.viz, ...base.viz || {} },
      } : {}),
      getGlobalAbsmax: this.getGlobalAbsmax.bind(this),
    }
  }

  initLeft() {
    const left_params = this.prepChildParams(this.params.left)
    left_params.is_child = 'left'
    left_params.block['i blocks'] = this.params.block['i blocks']
    left_params.block['j blocks'] = this.params.block['k blocks']
    if (left_params.op) {
      this.left = buildOpNode(left_params, this.context)
    } else if (left_params.matmul) {
      this.left = new MatMul(left_params, this.context, false)
    } else {
      const { right, result, polarity } = this.getPlacementInfo()
      left_params.deco.shape_info = {
        h: { name: 'I', place: result == polarity },
        w: { name: 'K', place: right },
      }
      const data = Array2D.fromInit(this.H, this.D, getInitFunc(left_params))
      this.left = new Mat(data, left_params, this.context, false)
    }
  }

  initRight() {
    const right_params = this.prepChildParams(this.params.right)
    right_params.is_child = 'right'
    right_params.block['i blocks'] = this.params.block['k blocks']
    right_params.block['j blocks'] = this.params.block['j blocks']
    if (right_params.op) {
      this.right = buildOpNode(right_params, this.context)
    } else if (right_params.matmul) {
      this.right = new MatMul(right_params, this.context, false)
    } else {
      const { left, result, polarity } = this.getPlacementInfo()
      right_params.deco.shape_info = {
        h: { name: 'K', place: left },
        w: { name: 'J', place: result == polarity },
      }
      const data = Array2D.fromInit(this.D, this.W, getInitFunc(right_params))
      this.right = new Mat(data, right_params, this.context, false)
    }
  }

  initResult() {
    const result_init = (i, j) => this.dotprod(i, j, 0, this.D)
    const data = Array2D.fromInit(this.H, this.W, result_init, this.params.epilog)
    const result_params = this.prepChildParams()
    // if (this.params.total == this.params.count) {
    if (!this.params.is_child) {
      const placement = this.getPlacementInfo()
      result_params.deco.shape_info = {
        h: { name: 'I', place: placement.left },
        w: { name: 'J', place: placement.right },
      }
    }
    result_params.block['i blocks'] = result_params.block['i blocks']
    result_params.block['j blocks'] = result_params.block['j blocks']
    this.result = new Mat(data, result_params, this.context, false)
  }

  // TODO clean up the way epilogs are done.
  // currently we run a pointwise epi if we find one,
  // or we do preprocessing needed by the in-place epi
  // (which is run later) based on snooping the expression
  applyPointwiseEpilog(x) {
    const epi = this.params.epilog
    const pw = POINTWISE[epi]
    if (pw) {
      return pw(x)
    } else if (epi == 'x/k') {
      return x / this.D
    } else if (epi.includes('x/sqrt(k)')) {
      return x / Math.sqrt(this.D)
    } else if (epi.includes('x/8')) {
      return x / 8
    } else {
      return x
    }
  }

  dotprod(i, k, minj, maxj) {
    const lw = this.left.W
    const ld = this.left.getDataArray()
    const rw = this.right.W
    const rd = this.right.getDataArray()
    const maxlx = i * lw + maxj

    let x = 0.0
    for (let lx = i * lw + minj, rx = minj * rw + k; lx < maxlx; lx++, rx += rw) {
      x += ld[lx] * rd[rx]
    }

    if (isNaN(x)) {
      console.log(`HEY dotprod(${i}, ${k}, ${minj}, ${maxj}) is NaN`)
      return 0
    }

    return this.applyPointwiseEpilog(x)   // reads params.epilog itself
  }

  getDataArray() {
    return this.result.getDataArray()
  }

  getData(i, j) {
    return this.result.getData(i, j)
  }

  show(r = undefined, c = undefined) {
    this.left.show(r, c)
    this.right.show(r, c)
    this.result.show(r, c)
  }

  hide(r = undefined, c = undefined) {
    this.left.hide(r, c)
    this.right.hide(r, c)
    this.result.hide(r, c)
  }

  setColorsAndSizes(r = undefined, c = undefined, size = undefined, color = undefined) {
    this.result.setColorsAndSizes(r, c, size, color)
  }

  bumpColor(r = undefined, c = undefined) {
    this.result.bumpColor(r, c)
  }

  ikjmul(i, k, j) {
    return this.left.getData(i, k) * this.right.getData(k, j)
  }

  getExtent() {
    const gap = this.params.layout.gap
    return this._extents || (this._extents = {
      x: this.getDispW() + 2 * gap - 1,
      y: this.getDispH() + 2 * gap - 1,
      z: this.getDispD() + 2 * gap - 1,
    })
  }

  initViz(params = undefined) {
    if (params) {
      this.params = params
    }

    util.disposeAndClear(this.group)
    this.flow_guide_group = undefined
    this.anim_mats = []

    if (this.left.params.anim.alg == 'inherit') {
      this.left.params.anim.alg = this.params.anim.alg
    }
    if (this.right.params.anim.alg == 'inherit') {
      this.right.params.anim.alg = this.params.anim.alg
    }

    setElemScale(this.params.viz['elem scale'])
    this.initResultViz()
    this.initLeftViz()
    this.initRightViz()

    this.setFlowGuide()
    this.setRowGuides()
    this.setAlignGrid()
  }

  initLeftViz() {
    this.left.initViz()
    if (this.params.layout.polarity.startsWith('positive')) {
      this.left.group.rotation.y = -Math.PI / 2
      this.left.group.position.x = this.params.layout['left placement'].startsWith('left') ?
        -this.getLeftScatter() :
        this.getExtent().x + this.left.getExtent().z + this.getLeftScatter()
    } else { // negative
      this.left.group.rotation.y = Math.PI / 2
      this.left.group.position.z = this.getExtent().z
      this.left.group.position.x = this.params.layout['left placement'].startsWith('left') ?
        -(this.left.getExtent().z + this.getLeftScatter()) :
        this.getExtent().x + this.getLeftScatter()
    }
    this.group.add(this.left.group)
  }

  initRightViz() {
    this.right.initViz()
    if (this.params.layout.polarity.startsWith('positive')) {
      this.right.group.rotation.x = Math.PI / 2
      this.right.group.position.y = this.params.layout['right placement'].startsWith('top') ?
        -this.getRightScatter() :
        this.getExtent().y + this.right.getExtent().z + this.getRightScatter()
    } else { // negative
      this.right.group.rotation.x = -Math.PI / 2
      this.right.group.position.z = this.getExtent().z
      this.right.group.position.y =
        this.params.layout['right placement'].startsWith('top') ?
          -(this.right.getExtent().z + this.getRightScatter()) :
          this.getExtent().y + this.getRightScatter()
    }
    this.group.add(this.right.group)
  }

  initResultViz() {
    this.result.initViz()
    this.result.group.position.z =
      this.params.layout['result placement'].startsWith('back') ?
        this.getExtent().z :
        0
    this.group.add(this.result.group)
  }

  getPlacementInfo() {
    return {
      polarity: this.params.layout.polarity.startsWith('positive'),
      left: this.params.layout['left placement'].startsWith('left'),
      right: this.params.layout['right placement'].startsWith('top'),
      result: this.params.layout['result placement'].startsWith('front'),
    }
  }

  getLayoutInfo() {
    const info: any = this.getPlacementInfo()
    Object.entries(info).forEach(([k, v]) => info[k] = v ? 1 : -1)
    info.gap = this.params.layout.gap
    info.left_scatter = this.getLeftScatter()
    info.right_scatter = this.getRightScatter()
    return info
  }

  setFlowGuide(light = undefined) {
    if (light != this.params.deco['flow guides']) {
      light = util.syncProp(this.params.deco, 'flow guides', light)
      if (this.flow_guide_group) {
        this.group.remove(this.flow_guide_group)
        util.disposeAndClear(this.flow_guide_group)
        this.flow_guide_group = undefined
      }
      if (light > 0.0) {
        this.flow_guide_group = util.flowGuide(
          this.getDispH(), this.getDispD(), this.getDispW(), this.getLayoutInfo(), light
        )
        this.group.add(this.flow_guide_group)
      }
    }
    this.left.setFlowGuide(light)
    this.right.setFlowGuide(light)
  }

  scatterFromCount(count) {
    const { scatter, molecule, blast } = this.params.layout
    const mult = count < molecule ? 0 :
      blast >= 0 ? count ** blast :
        (this.params.total - count) ** -blast
    return scatter * mult
  }

  getLeftScatter() {
    return this.scatterFromCount(this.left.params.count)
  }

  getRightScatter() {
    return this.scatterFromCount(this.right.params.count)
  }

  updateLabels(params = undefined) {
    if (params) {
      this.params.deco.spotlight = params.deco.spotlight
      this.params.deco['interior spotlight'] = params.deco['interior spotlight']
    }

    const spotlight = this.params.deco.spotlight
    this.left.updateLabels(isInterior(this.params.left) ? params : spotlight)
    this.right.updateLabels(isInterior(this.params.right) ? params : spotlight)
    this.result.updateLabels(spotlight)

    const interior_spotlight = this.params.deco['interior spotlight'] ? spotlight : 0
    this.anim_mats.map(m => m.updateLabels(interior_spotlight))
  }

  // The children `nodeBoundingBox` may recurse into. It guards on
  // `getBoundingBox`, which only the interior kinds have -- a leaf Mat's box is
  // inside this one's extent by construction -- so this is the same set the
  // matmul-only version unioned, spelled once for every node kind.
  childNodes() {
    return [this.left, this.right, this.result]
  }

  getBoundingBox() {
    return nodeBoundingBox(this)
  }

  center() {
    const c = this.getBoundingBox().getCenter(new THREE.Vector3())
    util.updateProps(this.group.position, c.negate())
  }

  // Memoized for the same reason Mat.getGlobalAbsmax is, and with the same
  // justification: no path changes a Mat's `absmax` after construction.
  getAbsmax() {
    return this._absmax ??=
      Math.max(this.left.getAbsmax(), this.right.getAbsmax(), this.result.getAbsmax())
  }

  getVizSummary() {
    return mergeVizSummaries([this.left, this.right, this.result].map(m => m.getVizSummary()))
  }

  setVizProp(k, v) {
    this.params.viz[k] = v
    ;[this.left, this.right, this.result].forEach(m => m.setVizProp(k, v))
  }

  getGlobalAbsmax() {
    return this.params.getGlobalAbsmax ? this.params.getGlobalAbsmax() : this.getAbsmax()
  }

  hideInputs(hide) {
    util.syncProp(this.params.anim, 'hide inputs', hide)
    const one = m => {
      if (m.hideInputs) {
        m.hideInputs(hide)
      } else if (this.params.anim.alg != 'none') {
        hide ? m.hide() : m.show()
      }
    }
    one(this.left)
    one(this.right)
  }

  setRowGuides(light = undefined) {
    light = util.syncProp(this.params.deco, 'row guides', light)
    this.left.setRowGuides(light)
    this.right.setRowGuides(light)
    this.result.setRowGuides(light)
    this.anim_mats.forEach(m => m.setRowGuides(light))
  }

  // The 3D half of the lattice: one box over this matmul's own H x W x D
  // extent, which is the box its three faces are drawn on. At scatter 0 the
  // operand faces coincide with two of these faces, which is the point -- it
  // is what makes an operand read as belonging to the product.
  setAlignGrid(light = undefined) {
    const prev = this.params.deco.grid
    light = util.syncProp(this.params.deco, 'grid', light)
    if (!this.align_grid_group || prev != light) {
      if (this.align_grid_group) {
        this.group.remove(this.align_grid_group)
        util.disposeAndClear(this.align_grid_group)
        this.align_grid_group = undefined
      }
      if (light > 0.0) {
        this.align_grid_group = util.alignGrid(
          this.getDispH(), this.getDispW(), this.getDispD(),
          gridSpacing(this.params.deco), light,
          this.getBlockInfo(), this.params.layout.gap)
        util.updateProps(this.align_grid_group.position, {
          x: this.params.layout.gap, y: this.params.layout.gap,
        })
        this.group.add(this.align_grid_group)
      }
    }
    this.left.setAlignGrid(light)
    this.right.setAlignGrid(light)
    this.result.setAlignGrid(light)
  }

  setName(name) {
    name = util.syncProp(this.params, 'name', name)
    this.result.setName(name)
  }

  setLegends(name = undefined, shape = undefined) {
    name = util.syncProp(this.params.deco, 'legends', name)
    shape = util.syncProp(this.params.deco, 'shape', shape)
    this.left.setLegends(name, shape)
    this.right.setLegends(name, shape)
    this.result.setLegends(name, shape)
  }

  // animation

  initAnimation(cb = undefined) {
    if (this.params.anim.alg == 'none') {
      if (this.params.anim['hide inputs']) {
        !this.params.left.matmul && this.left.show()
        !this.params.right.matmul && this.right.show()
      }
      return
    }

    const bumps = {
      'dotprod (row major)': () => this.getVmprodBump(true),
      'dotprod (col major)': () => this.getMvprodBump(true),
      'axpy': () => this.getVvprodBump(true),
      'mvprod': () => this.getMvprodBump(false),
      'vmprod': () => this.getVmprodBump(false),
      'vvprod': () => this.getVvprodBump(false),
    }

    const nj = this.getBlockInfo().j.n
    const nlk = () => this.left.getBlockInfo().k.n
    const nri = () => this.right.getBlockInfo().i.n

    const { alg, fuse } = this.params.anim

    let left_done = true, right_done = true

    this.alg_join = () => {
      const lalg = !this.params.left.matmul || left_done ? 'none' :
        (fuse == 'async' || this.left.getIndex() == this.getIndex() ?
          this.left.alg_join() :
          'mixed')

      const ralg = !this.params.right.matmul || right_done ? 'none' :
        (fuse == 'async' || this.right.getIndex() == this.getIndex() ?
          this.right.alg_join() :
          'mixed')

      const or_none = (a, b) => a == b || a == 'none'

      return (alg == 'vmprod' && or_none(lalg, 'vmprod') && ralg == 'none') ? 'vmprod' :
        (alg == 'mvprod' && lalg == 'none' && or_none(ralg, 'mvprod')) ? 'mvprod' :
          (alg == 'vvprod' && or_none(lalg, 'mvprod') && or_none(ralg, 'vmprod')) ? 'vvprod' :
            (lalg == 'none' && ralg == 'none') ? alg :
              'mixed'
    }

    const can_fuse = () => fuse != 'none' && this.alg_join() != 'mixed'

    const start = () => {
      const result_bump = bumps[alg]()

      this.bump = () => {
        const go = left_done && right_done || can_fuse()
        left_done || this.left.bump()
        right_done || this.right.bump()
        go && result_bump()
      }

      if (this.params.left.matmul && this.params.left.anim.alg != 'none') {
        left_done = false
        this.left.initAnimation(() => left_done = true)
      }

      if (this.params.right.matmul && this.params.right.anim.alg != 'none') {
        right_done = false
        this.right.initAnimation(() => right_done = true)
      }

      if (this.params.anim['hide inputs']) {
        this.left.hide()
        this.right.hide()
      }
      this.result.hide()

      !cb && this.bump()
    }

    this.onAnimDone = () => {
      this.clearAnimMats()
      nj > 1 && this.result.show()
      cb ? cb() : start()
    }

    start()
  }

  getBlockInfo() {
    const ni = Math.min(this.params.block['i blocks'], this.H)
    const nk = Math.min(this.params.block['k blocks'], this.D)
    const nj = Math.min(this.params.block['j blocks'], this.W)
    return {
      i: { n: ni, size: Math.ceil(this.H / ni), max: this.H },
      k: { n: nk, size: Math.ceil(this.D / nk), max: this.D },
      j: { n: nj, size: Math.ceil(this.W / nj), max: this.W },
    }
  }

  grid(dims, f) {
    grid(this.getBlockInfo(), dims, f)
  }

  getAnimIntermediateParams(name) {
    const params = this.prepChildParams()
    // params.name = name // debug
    delete params.name
    params.viz.sensitivity == 'superlocal' && (params.viz.sensitivity = 'local')
    params.block['i blocks'] = 1
    params.block['k blocks'] = 1
    params.block['j blocks'] = 1
    return params
  }

  getAnimResultParams() {
    const params = this.prepChildParams()
    // params.name = name // debug
    delete params.name
    params.viz.sensitivity == 'superlocal' && (params.viz.sensitivity = 'local')
    params.block['i blocks'] = params.block['i blocks']
    params.block['k blocks'] = params.block['j blocks']
    return params
  }

  clearAnimMats() {
    this.anim_mats.forEach(m => {
      this.group.remove(m.group)
      util.disposeAndClear(m.group)
    })
    this.anim_mats = []
  }

  getAnimResultMats() {
    const { k: { n: nk, size: sk } } = this.getBlockInfo()
    if (nk == 1) {
      return [this.result]
    }
    const { gap, polarity, result } = this.getLayoutInfo()
    const { z: extz } = this.getExtent()
    const results = []
    this.grid('k', ({ start: k, end: ke, index: ki }) => {
      const result_init = (i, j) => this.dotprod(i, j, k, ke)
      const data = Array2D.fromInit(this.H, this.W, result_init)
      const mat = new Mat(data, this.getAnimResultParams(), this.context, true)
      mat.group.position.z = polarity > 0 ?
        result > 0 ?
          ki == 0 ?
            this.result.group.position.z :
            gap + k + Math.floor(gap * k / sk - gap / 4) :
          ki == nk - 1 ?
            this.result.group.position.z :
            gap + ke + Math.floor(gap * k / sk + (gap - 1) / 4) :
        result > 0 ?
          ki == nk - 1 ?
            this.result.group.position.z :
            extz - ke - Math.floor(gap * ke / sk + (gap - 1) / 4) :
          ki == 0 ?
            this.result.group.position.z :
            extz - k - Math.floor(gap * ke / sk - gap / 4)
      mat.setRowGuides()
      mat.hide()
      results.push(mat)
      this.group.add(mat.group)
      this.anim_mats.push(mat)
    })
    return results
  }

  getVmprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const results = this.getAnimResultMats()

    const vmps: Record<string, any> = {}   // keyed by [i,k,j], coerced to 'i,k,j'
    this.grid('ikj', (
      { start: i, index: ii },
      { start: k, extent: kx, index: ki },
      { start: j, extent: jx, index: ji }
    ) => {
      const vmpinit = (kii, jii) => this.ikjmul(i, k + kii, j + jii)
      const data = Array2D.fromInit(kx, sweep ? 1 : jx, vmpinit)
      const vmp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.vmp[${ii}, ${ki}, ${ji}]`), this.context, true)
      vmp.hide()
      const z = polarity < 0 ? this.getExtent().z - k - (gap * ki) : k + (gap * ki)
      util.updateProps(vmp.group.position, { x: j + ji, y: gap + i + ii, z })
      vmp.group.rotation.x = polarity * Math.PI / 2
      vmps[String([i, k, j])] = vmp
      this.anim_mats.push(vmp)
      this.group.add(vmp.group)
    })

    const { i: { size: isize }, j: { size: jsize } } = this.getBlockInfo()
    let curi = -1
    let curj = sweep ? -1 : 0

    this.getIndex = () => curi

    return () => {
      // update indexes
      const [oldi, oldj] = [curi, curj]
      sweep && (curj = (curj + 1) % jsize)
      curj == 0 && curi++

      // clear old input hilights
      if (oldi >= 0 && !this.params.anim['hide inputs']) {
        sweep && this.grid('j', ({ start: j, extent: jx }) => {
          oldj < jx && this.right.setColorsAndSizes(undefined, j + oldj)
        })
        oldi != curi && this.grid('i', ({ start: i, extent: ix }) => {
          oldi < ix && this.left.setColorsAndSizes(i + oldi, undefined)
        })
      }

      // end of cycle
      if (curi == isize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curi == 0 && curj == 0) {
        Object.values(vmps).forEach(vmp => vmp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input hilights
      if (!this.params.anim['hide inputs']) {
        sweep && this.grid('j', ({ start: j, extent: jx }) => {
          curj < jx && this.right.bumpColor(undefined, j + curj)
        })
        oldi != curi && this.grid('i', ({ start: i, extent: ix }) => {
          curi < ix && this.left.bumpColor(i + curi, undefined)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i, extent: ix, index: ii },
        { start: k },
        { start: j, extent: jx, index: ji }
      ) => {
        const vmp = vmps[String([i, k, j])]
        if (curi < ix && curj < jx) {
          util.updateProps(vmp.group.position, { x: j + (ji * gap) + curj, y: gap + i + (ii * gap) + curi })
          vmp.reinit((ki, ji) => this.ikjmul(i + curi, k + ki, j + curj + ji))
        }
      })

      // reveal new results
      this.grid('ij', ({ start: i, extent: ix }, { start: j, end: je, extent: jx }) => {
        curi < ix && curj < jx && results.forEach(r => r.show(i + curi, sweep ? j + curj : [j, je]))
      })

      // update labels
      this.updateLabels()
    }
  }

  getMvprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const results = this.getAnimResultMats()

    const mvps: Record<string, any> = {}   // keyed by [i,k,j], coerced to 'i,k,j'
    this.grid('ikj', (
      { start: i, extent: ix, index: ii },
      { start: k, extent: kx, index: ki },
      { start: j, index: ji },
    ) => {
      const mvpinit = (iii, kii) => this.ikjmul(i + iii, k + kii, j)
      const data = Array2D.fromInit(sweep ? 1 : ix, kx, mvpinit)
      const mvp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.mvp[${ii}, ${ki}, ${ji}]`), this.context, true)
      mvp.hide()
      const z = polarity < 0 ? this.getExtent().z - k - (gap * ki) : k + (gap * ki)
      util.updateProps(mvp.group.position, { x: gap + j + ji, y: i + ii, z })
      mvp.group.rotation.y = polarity * -Math.PI / 2
      mvps[String([i, k, j])] = mvp
      this.anim_mats.push(mvp)
      this.group.add(mvp.group)
    })

    const { i: { size: isize }, j: { size: jsize } } = this.getBlockInfo()
    let curj = -1
    let curi = sweep ? -1 : 0

    this.getIndex = () => curj

    return () => {
      // update indexes
      const [oldi, oldj] = [curi, curj]
      sweep && (curi = (curi + 1) % isize)
      curi == 0 && curj++

      // clear old input hilights
      if (oldj >= 0 && !this.params.anim['hide inputs']) {
        sweep && this.grid('i', ({ start: i, extent: ix }) => {
          oldi < ix && this.left.setColorsAndSizes(i + oldi, undefined)
        })
        oldj != curj && this.grid('j', ({ start: j, extent: jx }) => {
          oldj < jx && this.right.setColorsAndSizes(undefined, j + oldj)
        })
      }

      // end of cycle
      if (curj == jsize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curj == 0 && curi == 0) {
        Object.values(mvps).forEach(vmp => vmp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input hilights
      if (!this.params.anim['hide inputs']) {
        sweep && this.grid('i', ({ start: i, extent: ix }) => {
          curi < ix && this.left.bumpColor(i + curi, undefined)
        })
        oldj != curj && this.grid('j', ({ start: j, extent: jx }) => {
          curj < jx && this.right.bumpColor(undefined, j + curj)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i, extent: ix, index: ii },
        { start: k },
        { start: j, extent: jx, index: ji }
      ) => {
        const mvp = mvps[String([i, k, j])]
        if (curi < ix && curj < jx) {
          util.updateProps(mvp.group.position, { x: gap + j + (ji * gap) + curj, y: i + (ii * gap) + curi })
          mvp.reinit((ii, ki) => this.ikjmul(i + curi + ii, k + ki, j + curj))
        }
      })

      // reveal new results
      this.grid('ij', ({ start: i, end: ie, extent: ix }, { start: j, extent: jx }) => {
        curi < ix && curj < jx && results.forEach(r => r.show(sweep ? i + curi : [i, ie], j + curj))
      })

      // update labels
      this.updateLabels()
    }
  }

  getVvprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const { z: extz } = this.getExtent()
    // no intermediate result planes for vvprod, too cluttered. just sum it into final result
    const results = [this.result]

    // pre-epilog shadow for result accum
    const pre_epilog = Array2D.fromInit(this.H, this.W, () => 0)

    const vvps: Record<string, any> = {}   // keyed by [i,k,j], coerced to 'i,k,j'
    this.grid('ikj', (
      { start: i, extent: ix, index: ii },
      { start: k, index: ki },
      { start: j, extent: jx, index: ji }
    ) => {
      const vvpinit = (iii, jii) => this.ikjmul(i + iii, k, j + jii)
      const data = Array2D.fromInit(ix, sweep ? 1 : jx, vvpinit)
      const vvp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.vvp[${ii}, ${ki}, ${ji}]`), this.context, true)
      vvp.hide()
      const z = polarity > 0 ? gap + k + ki : extz - gap - k - ki
      util.updateProps(vvp.group.position, { x: j + ji * gap, y: i + ii * gap, z })
      vvps[String([i, k, j])] = vvp
      this.anim_mats.push(vvp)
      this.group.add(vvp.group)
    })

    const { k: { size: ksize }, j: { size: jsize } } = this.getBlockInfo()
    let curk = -1
    let curj = sweep ? -1 : 0

    this.getIndex = () => curk

    return () => {
      // update indexes
      const [oldk, oldj] = [curk, curj]
      curk++
      if (sweep && curk % ksize == 0) {
        curk = 0
        curj++
      }

      // clear old input highlights
      if (oldk >= 0 && !this.params.anim['hide inputs']) {
        sweep ?
          this.grid('kj', ({ start: k, extent: kx }, { start: j, extent: jx }) => {
            oldk < kx && oldj < jx && this.right.setColorsAndSizes(k + oldk, j + oldj)
          }) :
          this.grid('k', ({ start: k, extent: kx }) => {
            oldk < kx && this.right.setColorsAndSizes(k + oldk, undefined)
          })
        this.grid('k', ({ start: k, extent: kx }) => {
          oldk < kx && this.left.setColorsAndSizes(undefined, k + oldk)
        })
      }

      // end of cycle
      if (sweep ? curj == jsize : curk == ksize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curj == 0 && curk == 0) {
        Object.values(vvps).forEach(vvp => vvp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input highlights
      if (!this.params.anim['hide inputs']) {
        sweep ?
          this.grid('kj', ({ start: k, extent: kx }, { start: j, extent: jx }) => {
            curk < kx && curj < jx && this.right.bumpColor(k + curk, j + curj)
          }) :
          this.grid('k', ({ start: k, extent: kx }) => {
            curk < kx && this.right.bumpColor(k + curk, undefined)
          })
        this.grid('k', ({ start: k, extent: kx }) => {
          curk < kx && this.left.bumpColor(undefined, k + curk)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i },
        { start: k, extent: kx, index: ki },
        { start: j, extent: jx, index: ji }
      ) => {
        const vvp = vvps[String([i, k, j])]
        if (curk < kx && curj < jx) {
          const z = polarity > 0 ? gap + k + (ki * gap) + curk : extz - gap - k - (ki * gap) - curk
          util.updateProps(vvp.group.position, { x: j + ji * gap + curj, z })
          vvp.reinit((iii, jii) => this.ikjmul(i + iii, k + curk, j + curj + jii))
        }
      })

      // no intermediate result planes for vvprod, too cluttered. just sum it into final result
      // also we go thru some gymnastics to do epilog during sweep
      this.grid('kj', ({ start: k, extent: kx, index: ki }, { start: j, end: je, extent: jx }) => {
        if (curk < kx && curj < jx) {
          const running_dp = (ii, ji) => {
            const x = this.left.getData(ii, k + curk) * this.right.getData(k + curk, ji)
            return (ki == 0 && curk == 0) ? x : (pre_epilog.get(ii, ji) + x)
          }
          pre_epilog.reinit(running_dp, undefined, undefined, sweep ? j + curj : [j, je])

          const pw_epilog_dp = (ii, ji) => this.applyPointwiseEpilog(pre_epilog.get(ii, ji))
          results[0].reinit(pw_epilog_dp, undefined, undefined, sweep ? j + curj : [j, je])
        }
      })
      applyInPlaceEpilog_(results[0].data.data, results[0].H, results[0].W, this.params.epilog)
      if (sweep) {
        this.grid('kj', ({ extent: kx }, { start: j, end: je, extent: jx }) => {
          if (curk < kx && curj < jx) {
            results[0].reinit(() => 0, undefined, undefined, [j + curj + 1, je])
          }
        })
      }
      results[0].setColorsAndSizes()

      // update labels
      this.updateLabels()
    }
  }
}

//
// Node kinds beyond the matmul
//
// mm's graph has always been matmuls all the way down, and everything else was
// squeezed into one of two places: a pointwise or in-place *epilog* mutated
// into a matmul's own result buffer, and a bias was augmented into the operands
// (`X @ W + b` drawn as `[X | 1] @ [W ; b]`). Both are still here and every
// existing view still uses them.
//
// What they cannot express is a stage of a forward pass that is a matrix in its
// own right. GPT-2's two residual additions per block are real edges of the
// graph, and `softmax(tril(QK^T/sqrt(d)))` is a matrix an inspector wants to
// look at *next to* `Q @ K^T`, not a mutation of it. So there are three more
// node kinds, marked by an explicit `op` on the params node:
//
//   op: 'unary'  materializes f(input) as its own Mat, drawn beside its input.
//                The in-place epilogs are untouched; this is the other form.
//   op: 'add'    an elementwise sum of two same-shaped operands. Never drawn
//                as a matmul: a fake matmul that happened to produce the right
//                numbers is exactly the class of lie this repository forbids.
//   op: 'stack'  an ordered list of stages laid out in one scene, with no
//                arithmetic of its own. This is what makes a whole-model view
//                possible; see the Stack class below.
//
// `genExpr`/`syncExpr` refuse trees containing any of them -- see the note
// there. They are a matmul-only notation and there is no honest '@' for an add.
//

// Shape of any node, by kind. MatMul used to carry a matmul-only pair of these
// as locals; an operand can now be a unary or an add, and a helper that did not
// know that would read `undefined` and draw an empty matrix.
export const nodeHeight = p =>
  p.op === 'unary' ? nodeHeight(p.input) :
    p.op === 'add' ? nodeHeight(p.left) :
      p.matmul ? nodeHeight(p.left) : p.h

export const nodeWidth = p =>
  p.op === 'unary' ? nodeWidth(p.input) :
    p.op === 'add' ? nodeWidth(p.right) :
      p.matmul ? nodeWidth(p.right) : p.w

// Whether a node draws a subtree of its own (so it takes the whole params
// object for label recursion) or is a single matrix (so it takes a spotlight).
export const isInterior = p => !!(p.matmul || p.op)

// The unary functions a materialized stage may apply. Both families:
// elementwise (POINTWISE) and row/matrix-wise (the in-place epilogs). Named
// separately from EPILOGS because the scale factors there -- 'x/sqrt(k)' and
// friends -- belong to a matmul's contraction depth and mean nothing to a
// stage that has no contraction.
const UNARY_FUNCS_ = {
  ...POINTWISE,
  'softmax': (h, w, d) => softmax_(h, w, d),
  'softmax(tril(x))': (h, w, d) => softmax_tril_(h, w, d),
  'layernorm': (h, w, d) => layernorm_(h, w, d),
}

export const UNARY_FUNCS = Object.keys(UNARY_FUNCS_)

// Applied to a copy, never in place on the input: the whole point of the kind
// is that the input stays on screen next to the result.
// Exported for editops.ts, which re-runs a UnaryOp's function over fresh input
// data when an edit upstream invalidates the materialized result.
export function applyUnary(fn, h, w, data) {
  const pw = POINTWISE[fn]
  if (pw) {
    for (let i = 0; i < data.length; i++) data[i] = pw(data[i])
    return
  }
  const mw = UNARY_FUNCS_[fn]
  if (!mw) {
    // Loud, and naming what is missing. A stage that cannot be computed must
    // not draw a zero-filled placeholder that looks like an answer.
    throw new Error(`unknown unary stage function '${fn}'; known: ${UNARY_FUNCS.join(', ')}`)
  }
  mw(h, w, data)
}

export function buildOpNode(params, context, init_viz = false) {
  switch (params.op) {
    case 'unary': return new UnaryOp(params, context, init_viz)
    case 'add': return new AddOp(params, context, init_viz)
    case 'stack': return new Stack(params, context, init_viz)
    default: throw new Error(`unknown node op '${params.op}'`)
  }
}

/** Build whatever kind of node `params` describes, sized `h` x `w` if a leaf. */
function buildChildNode(params, context, h, w) {
  if (params.op) return buildOpNode(params, context)
  if (params.matmul) return new MatMul(params, context, false)
  return new Mat(Array2D.fromInit(h, w, getInitFunc(params)), params, context, false)
}

/** Bounding box of any node and every interior node hanging off it. */
export function nodeBoundingBox(node) {
  const min = node.group.localToWorld(new THREE.Vector3())
  const max = node.group.localToWorld(new THREE.Vector3().copy(node.getExtent()))
  const swap = d => { const temp = min[d]; min[d] = max[d]; max[d] = temp }
  ;['x', 'y', 'z'].forEach(d => { if (min[d] > max[d]) swap(d) })
  const bb = new THREE.Box3(min, max)
  node.childNodes().forEach(c => c.getBoundingBox && bb.union(c.getBoundingBox()))
  return bb
}

/**
 * What UnaryOp, AddOp and Stack share: they own child node objects, they lay
 * them out in a row, and every recursion the app performs over the tree has to
 * reach them. Subclasses supply `childNodes()` and `initViz()`; everything else
 * here is the same recursion MatMul does, spelled once.
 */
abstract class OpNode {
  params: any
  context: any
  group: any
  H: any
  W: any
  _extents: any
  onAnimDone: any
  bump: any

  constructor(params, context) {
    this.context = context
    this.params = util.copyTree(params)
    ensureChildCounts(this.params)
    this.group = new THREE.Group()
    this.group.name = `${this.params.name}.group`
  }

  abstract childNodes(): any[]
  abstract initViz(params?): void
  abstract getDataArray(): any
  abstract getData(i, j): any

  prepChildParams(base) {
    return {
      ...base,
      anim: { ...this.params.anim, ...base.anim || {} },
      block: { ...this.params.block, ...base.block || {} },
      deco: { ...this.params.deco, ...base.deco || {} },
      layout: { ...this.params.layout, ...base.layout || {} },
      viz: { ...this.params.viz, ...base.viz || {} },
      getGlobalAbsmax: this.getGlobalAbsmax.bind(this),
    }
  }

  // Row layout: children left to right along x, separated by a gap, each
  // sitting on the same y and z. It is deliberately not a matmul's three-faces
  // arrangement -- these nodes do not contract anything, and borrowing the
  // matmul geometry would imply they did.
  layoutRow(nodes) {
    const gap = this.params.layout.gap
    let x = 0
    let [y, z] = [0, 0]
    nodes.forEach(n => {
      n.group.position.x = x
      const e = n.getExtent()
      x += e.x + 2 * gap
      y = Math.max(y, e.y)
      z = Math.max(z, e.z)
      this.group.add(n.group)
    })
    this._extents = { x: Math.max(0, x - 2 * gap), y, z }
  }

  getExtent() { return this._extents || { x: 0, y: 0, z: 0 } }
  getBoundingBox() { return nodeBoundingBox(this) }
  disposeAll() { util.disposeAndClear(this.group) }
  center() {
    util.updateProps(this.group.position,
      this.getBoundingBox().getCenter(new THREE.Vector3()).negate())
  }
  _absmax: any
  getAbsmax() { return this._absmax ??= Math.max(...this.childNodes().map(n => n.getAbsmax())) }
  getGlobalAbsmax() {
    return this.params.getGlobalAbsmax ? this.params.getGlobalAbsmax() : this.getAbsmax()
  }
  getVizSummary() { return mergeVizSummaries(this.childNodes().map(n => n.getVizSummary())) }
  show(r?, c?) { this.childNodes().forEach(n => n.show(r, c)) }
  hide(r?, c?) { this.childNodes().forEach(n => n.hide(r, c)) }
  setColorsAndSizes(r?, c?, size?, color?) {
    this.childNodes().forEach(n => n.setColorsAndSizes(r, c, size, color))
  }
  bumpColor(r?, c?) { this.childNodes().forEach(n => n.bumpColor(r, c)) }
  setRowGuides(light?) {
    light = util.syncProp(this.params.deco, 'row guides', light)
    this.childNodes().forEach(n => n.setRowGuides(light))
  }
  setAlignGrid(light?) {
    light = util.syncProp(this.params.deco, 'grid', light)
    this.childNodes().forEach(n => n.setAlignGrid(light))
  }
  setFlowGuide(light?) { this.childNodes().forEach(n => n.setFlowGuide(light)) }
  setLegends(name?, shape?) {
    name = util.syncProp(this.params.deco, 'legends', name)
    shape = util.syncProp(this.params.deco, 'shape', shape)
    this.childNodes().forEach(n => n.setLegends(name, shape))
  }
  setName(name) { util.syncProp(this.params, 'name', name) }
  setVizProp(k, v) {
    this.params.viz[k] = v
    this.childNodes().forEach(n => n.setVizProp(k, v))
  }
  hideInputs(hide) {
    util.syncProp(this.params.anim, 'hide inputs', hide)
    this.childNodes().forEach(n => n.hideInputs && n.hideInputs(hide))
  }
  updateLabels(params?) {
    const spotlight = this.params.deco.spotlight
    this.childNodes().forEach((n, k) => n.updateLabels(
      isInterior(this.childParams()[k]) ? params : spotlight))
  }
  abstract childParams(): any[]
}

/**
 * f(input), drawn as its own matrix beside the input that produced it.
 *
 * The animation is the point of the kind: the input animates by whatever
 * algorithm it carries, and the result materializes when it finishes. That is
 * the picture the whole-model view is for -- `Q @ K^T` sweeps out, and then a
 * softmax matrix appears next to it.
 */
export class UnaryOp extends OpNode {
  input: any
  result: any
  fn: string

  constructor(params, context, init_viz = false) {
    super(params, context)
    this.fn = this.params.fn
    const ip = this.params.input
    this.H = nodeHeight(ip)
    this.W = nodeWidth(ip)
    this.input = buildChildNode(this.prepChildParams(ip), this.context, this.H, this.W)

    const data = new Float32Array(this.H * this.W)
    data.set(this.input.getDataArray().subarray(0, data.length))
    applyUnary(this.fn, this.H, this.W, data)
    const result_params = this.prepChildParams(util.copyTree(this.params))
    delete result_params.input
    result_params.name = this.params.name
    this.result = new Mat(new Array2D(this.H, this.W, data), result_params, this.context, false)

    if (init_viz) this.initViz()
  }

  childNodes() { return [this.input, this.result] }
  childParams() { return [this.params.input, { matmul: false }] }
  getDataArray() { return this.result.getDataArray() }
  getData(i, j) { return this.result.getData(i, j) }

  initViz(params = undefined) {
    if (params) this.params = params
    util.disposeAndClear(this.group)
    this.input.initViz()
    this.result.initViz()
    this.layoutRow([this.input, this.result])
    this.setRowGuides()
    this.setAlignGrid()
  }

  initAnimation(cb = undefined) {
    const reveal = () => {
      this.result.show()
      cb ? cb() : this.initAnimation()
    }
    if (this.input.initAnimation && this.params.anim.alg != 'none') {
      this.result.hide()
      this.input.initAnimation(reveal)
      this.bump = () => this.input.bump()
    } else {
      this.result.show()
      this.bump = () => reveal()
    }
  }
}

/**
 * left + right, elementwise. GPT-2 adds the attention output and the MLP output
 * back into the residual stream twice per block, and those are edges of the
 * graph, not matmuls.
 */
export class AddOp extends OpNode {
  left: any
  right: any
  result: any

  constructor(params, context, init_viz = false) {
    super(params, context)
    const [lp, rp] = [this.params.left, this.params.right]
    this.H = nodeHeight(lp)
    this.W = nodeWidth(lp)
    if (nodeHeight(rp) != this.H || nodeWidth(rp) != this.W) {
      // An add over mismatched shapes has no honest reading, and mm's leaf
      // loader would tile the shorter operand into a plausible wrong picture
      // rather than fail. Refuse here, naming both shapes.
      throw new Error(
        `add '${this.params.name}' has operands ${this.H}x${this.W} and ` +
        `${nodeHeight(rp)}x${nodeWidth(rp)}: an elementwise sum needs one shape`)
    }
    this.left = buildChildNode(this.prepChildParams(lp), this.context, this.H, this.W)
    this.right = buildChildNode(this.prepChildParams(rp), this.context, this.H, this.W)

    const ld = this.left.getDataArray(), rd = this.right.getDataArray()
    const data = new Float32Array(this.H * this.W)
    for (let i = 0; i < data.length; i++) data[i] = ld[i] + rd[i]
    const result_params = this.prepChildParams(util.copyTree(this.params))
    delete result_params.left
    delete result_params.right
    result_params.name = this.params.name
    this.result = new Mat(new Array2D(this.H, this.W, data), result_params, this.context, false)

    if (init_viz) this.initViz()
  }

  childNodes() { return [this.left, this.right, this.result] }
  childParams() { return [this.params.left, this.params.right, { matmul: false }] }
  getDataArray() { return this.result.getDataArray() }
  getData(i, j) { return this.result.getData(i, j) }

  initViz(params = undefined) {
    if (params) this.params = params
    util.disposeAndClear(this.group)
    this.left.initViz()
    this.right.initViz()
    this.result.initViz()
    this.layoutRow([this.left, this.right, this.result])
    this.setRowGuides()
    this.setAlignGrid()
  }

  initAnimation(cb = undefined) {
    // An add has nothing to sweep: both operands are already there and the sum
    // is elementwise. So it reveals, and hands straight back to the driver.
    // Pretending otherwise would be an animation of an algorithm that is not
    // what the model does.
    const done = () => { this.result.show(); cb ? cb() : undefined }
    this.result.show()
    this.bump = done
  }
}


/**
 * An ordered list of stages laid out in one scene.
 *
 * A Stack computes nothing. It owns no matrix of its own, contracts nothing,
 * and adds nothing -- it places already-honest nodes in space and walks them in
 * order. That is deliberate: a whole-model view is a *presentation* of a
 * forward pass, and every number in it has to come from a stage that could
 * stand on its own.
 *
 * `stages` is an object rather than an array because `util.copyTree` round
 * trips through flatten/unflatten, which does not handle arrays; string keys
 * keep their insertion order, which is the forward-pass order.
 *
 * Each stage carries an optional `row`, and rows stack down the scene. The
 * model view puts one transformer block per row, so the residual stream reads
 * as a spine down the left edge and each block expands rightwards through
 * attention and then the MLP.
 *
 * ## The rendering decision that makes this possible
 *
 * At model level every matrix draws as one LOD-reduced heatmap texture.
 * Full-resolution texels -- and the `elements` sphere path -- are spent only on
 * the matrices in the currently active stage. distilgpt2 at seq 64 is millions
 * of elements across ~37 stages; as instanced quads at full resolution it does
 * not run at all, and the budget below is what is enforced rather than hoped
 * for. `setStage` promotes one stage and demotes the last one, so the cost is
 * bounded by the largest single stage and not by the model.
 */
export class Stack extends OpNode {
  stages: any[] = []
  active = 0
  playing = false
  onStageChange: any = null

  constructor(params, context, init_viz = false) {
    super(params, context)
    // `copyTree` round trips through flatten/unflatten, which drops empty
    // sub-objects -- so an empty `stages` arrives here as `undefined` and
    // Object.entries would throw "Cannot convert undefined or null to object".
    // Say what is actually wrong instead.
    if (!this.params.stages || !Object.keys(this.params.stages).length) {
      throw new Error(`stack '${this.params.name}' has no stages`)
    }
    const entries = Object.entries(this.params.stages)
    entries.forEach(([key, sp]: [string, any]) => {
      const cp = this.prepChildParams(sp)
      // 'inherit' has no meaning at the top of a stage -- MatMul.initAnimation
      // looks the algorithm up in a table and would find nothing.
      if (!cp.anim.alg || cp.anim.alg === 'inherit') cp.anim.alg = this.params.anim.alg
      cp.anim.alg = 'none'           // every stage starts static; setStage arms one
      const obj = buildChildNode(cp, this.context, nodeHeight(sp), nodeWidth(sp))
      this.stages.push({
        key, name: sp.name, kind: sp.op || (sp.matmul ? 'matmul' : 'leaf'),
        note: sp.note || '', row: sp.row || 0, params: cp, obj,
      })
    })
    if (!this.stages.length) {
      throw new Error(`stack '${this.params.name}' has no stages`)
    }
    this.H = this.stages[this.stages.length - 1].obj.H
    this.W = this.stages[this.stages.length - 1].obj.W
    if (init_viz) this.initViz()
  }

  childNodes() { return this.stages.map(s => s.obj) }
  childParams() { return this.stages.map(s => s.params) }

  // A stack has no matrix of its own. The last stage's result is the model's
  // output, which is the only reading of "this stack's data" that is not made
  // up, so that is what these return.
  getDataArray() { return this.stages[this.stages.length - 1].obj.getDataArray() }
  getData(i, j) { return this.stages[this.stages.length - 1].obj.getData(i, j) }

  /** Per-matrix texel cap for the stages that are not active. */
  stageBudget() {
    const scene = this.params.viz['scene texel budget'] || HEATMAP_SCENE_TEXEL_BUDGET
    return Math.max(1 << 12, Math.floor(scene / Math.max(1, this.stages.length)))
  }

  /**
   * The render path a stage gets, `active` or not.
   *
   * C3's rule -- every matrix a LOD-reduced heatmap at model level, with
   * full-resolution texels and the sphere path spent only on the active stage
   * -- is what `auto` does, and `auto` is the default. It is a default and not
   * an invariant: an explicit 'spheres' or 'heatmap' is taken at its word
   * everywhere, because a control that declined to do what it says would be
   * worse than no control. Forcing spheres on the whole model is millions of
   * instanced quads and will be slow; the status bar prints the count so the
   * cost is visible rather than a surprise.
   */
  stageRenderMode(active: boolean) {
    const want = this.params.viz['render mode'] || 'auto'
    if (want !== 'auto') return want
    return active ? 'auto' : 'heatmap'
  }

  initViz(params = undefined) {
    if (params) this.params = params
    util.disposeAndClear(this.group)
    this.stages.forEach(st => {
      st.obj.setVizProp('texel budget', this.stageBudget())
      st.obj.setVizProp('render mode', this.stageRenderMode(false))
      st.obj.initViz()
    })
    this.layoutStages()
    this.setRowGuides()
    this.setAlignGrid()
  }

  /**
   * Place every stage. Rows are the model's layers, and `row flow` decides
   * which way successive rows advance:
   *
   *   vertical    (default) rows stack up the scene, stages run across a row.
   *               Six blocks read top to bottom, each expanding rightwards.
   *   horizontal  rows advance across the scene, stages stack up a row. The
   *               model reads left to right as one strip of columns, which is
   *               the arrangement that fits a many-layer model into a landscape
   *               viewport.
   *
   * The two are a transpose of one another, not a rotation: a stage is never
   * turned on its side, only placed on the other axis. An absent or unknown
   * value is 'vertical', so a scene built before this existed lays out
   * unchanged.
   */
  layoutStages() {
    const gap = this.params.layout.gap
    const margin = gap * 4
    const horizontal = (this.params.layout || {})['row flow'] === 'horizontal'
    const rows: any = {}
    this.stages.forEach(st => (rows[st.row] ||= []).push(st))

    // `along` runs within a row, `across` steps from row to row; which of the
    // two is x and which is y is the whole difference between the arrangements.
    let across = 0
    let [max_along, maxz] = [0, 0]
    Object.keys(rows).sort((a, b) => +a - +b).forEach(k => {
      let along = 0, depth = 0
      rows[k].forEach(st => {
        const e = st.obj.getExtent()
        util.updateProps(st.obj.group.position,
          horizontal ? { x: across, y: along, z: 0 } : { x: along, y: across, z: 0 })
        this.group.add(st.obj.group)
        along += (horizontal ? e.y : e.x) + margin
        depth = Math.max(depth, horizontal ? e.x : e.y)
        maxz = Math.max(maxz, e.z)
      })
      max_along = Math.max(max_along, Math.max(0, along - margin))
      across += depth + margin
    })
    const total_across = Math.max(0, across - margin)
    this._extents = horizontal
      ? { x: total_across, y: max_along, z: maxz }
      : { x: max_along, y: total_across, z: maxz }
  }

  stageList() {
    return this.stages.map((st, i) => ({ i, name: st.name, kind: st.kind, note: st.note }))
  }

  /**
   * Make stage `i` the active one: promoted to the full texel budget and the
   * `auto` render path, animated by the stack's algorithm. The stage that was
   * active goes back to the scene budget and to heatmap, fully shown.
   */
  setStage(i, playing = undefined) {
    if (playing !== undefined) this.playing = !!playing
    const n = this.stages.length
    const next = ((i % n) + n) % n
    if (next !== this.active) {
      const prev = this.stages[this.active]
      if (prev) {
        prev.params.anim.alg = 'none'
        prev.obj.setVizProp('texel budget', this.stageBudget())
        prev.obj.setVizProp('render mode', this.stageRenderMode(false))
        // Decoration is per stage, never over the whole stack. Re-running
        // setLegends() across 135 matrices costs seconds -- `util.getText`
        // tessellates glyphs -- and a timeline that pauses for that is not a
        // timeline. Only the two stages that changed are re-decorated, which is
        // C3's budget rule applied to text as well as to texels.
        prev.obj.initViz()
        prev.obj.show()
      }
      this.active = next
    }
    const cur = this.stages[this.active]
    cur.params.anim.alg = this.params.anim.alg
    cur.obj.setVizProp('texel budget', 0)          // 0 = the full per-matrix cap
    cur.obj.setVizProp('render mode', this.stageRenderMode(true))
    cur.obj.initViz()
    this.layoutStages()
    this.armActive()
    this.onStageChange && this.onStageChange(this.active, this.playing)
  }

  armActive() {
    const cur = this.stages[this.active]
    const advance = () => {
      if (this.playing) {
        this.setStage(this.active + 1)
      } else {
        this.bump = () => { }        // parked at the end of a finished stage
      }
    }

    // A finished stage is held before the timeline moves on. Some stages have
    // nothing to sweep at all -- an `add` never does, because the sum is
    // elementwise and pretending otherwise would animate an algorithm the model
    // does not run -- and those would otherwise flash past in a single frame.
    // `speed` is bumps per second, so that many bumps is about a second.
    const hold = () => {
      let ticks = 0
      return () => {
        if (++ticks >= Math.max(1, this.params.anim.speed)) { ticks = 0; advance() }
      }
    }

    if (cur.params.anim.alg === 'none' || !cur.obj.initAnimation) {
      cur.obj.show()
      this.bump = hold()
      return
    }
    let done = false
    cur.obj.initAnimation(() => { done = true; this.bump = hold() })
    const inner = cur.obj.bump
    this.bump = () => { if (!done) { inner ? inner() : advance() } }
  }

  initAnimation(cb = undefined) {
    this.setStage(this.active)
  }

  override setVizProp(k, v) {
    this.params.viz[k] = v
    this.stages.forEach(st => st.obj.setVizProp(k, v))
  }
}

//
// layout schemes
//

const layoutToBool = layout => ({
  pol: !!POLARITIES.indexOf(layout.polarity),
  left: !!LEFT_PLACEMENTS.indexOf(layout['left placement']),
  right: !!RIGHT_PLACEMENTS.indexOf(layout['right placement']),
  res: !!RESULT_PLACEMENTS.indexOf(layout['result placement'])
})

const boolToLayout = ({ pol, left, right, res }) => ({
  polarity: POLARITIES[+pol],
  'left placement': LEFT_PLACEMENTS[+left],
  'right placement': RIGHT_PLACEMENTS[+right],
  'result placement': RESULT_PLACEMENTS[+res]
})

export const LAYOUT_RULES = {
  'blocks': (left_child, { pol, left, right, res }) => ({
    pol: !pol,
    left: left_child ? pol != res : !left,
    right: left_child ? !right : pol != res,
    res: pol == (left_child ? left : right),
  }),
  'zigzag': (left_child, { pol, left, right, res }) => ({
    pol: !pol,
    left: left_child ? pol != res : left,
    right: left_child ? right : pol != res,
    res: pol == (left_child ? left : right),
  }),
  'wheel': (left_child, { pol, left, right, res }) => ({
    pol: pol,
    left: left,
    right: right,
    res: res
  }),
}

export const childLayout = (parent_layout, rule, left_child) =>
  boolToLayout(rule(left_child, layoutToBool(parent_layout)))

export function setLayoutScheme(params, scheme_name = undefined) {
  scheme_name = util.syncProp(params.layout, 'scheme', scheme_name)
  const rule = LAYOUT_RULES[scheme_name]
  // The scheme is a rule about how a matmul's three faces fold relative to its
  // parent's, so it applies to matmuls and to nothing else: it descends
  // *through* the other node kinds (which have no polarity or placement) into
  // whatever matmuls hang off them, rather than stopping at the first one or
  // writing a `layout` onto a node that has no use for it.
  function f(p) {
    childParamsOf(p).forEach(c => {
      if (c.matmul) {
        c.layout = childLayout(p.layout || params.layout, rule, c === p.left)
      }
      f(c)
    })
  }
  rule && f(params)
}

// 
// exprs
//

export const default_dims = { i: 32, j: 32, k: 32 }

export const defaultCam = () => ({
  x: -default_dims.i * 1.5,
  y: default_dims.j * 1.5,
  z: default_dims.k * 1.5,
})

export const default_epilog = 'none'

export const defaultLeft = () => ({
  name: 'L',
  matmul: false,
  h: default_dims.i,
  w: default_dims.j,
  init: 'row major',
  url: '',
  min: -1,
  max: 1,
  dropout: 0,
})

export const defaultRight = () => ({
  name: 'R',
  matmul: false,
  h: default_dims.j,
  w: default_dims.k,
  init: 'col major',
  url: '',
  min: -1,
  max: 1,
  dropout: 0,
})

export const defaultAnim = () => ({
  alg: 'inherit',
})

export const defaultBlock = () => ({
  'i blocks': 1,
  'k blocks': 1,
  'j blocks': 1,
})

export const defaultLayout = () => ({
  polarity: 'negative',
  'left placement': 'left',
  'right placement': 'top',
  'result placement': 'front',
})

// adjust tree to match a param node's i/k/j blocks
export function fixBlocks(p, anc, root) {
  const getInfo = (p, anc, root) => {
    const is_root = anc.length == 0
    const pp = !is_root && anc[0](root)
    const panc = !is_root && anc.slice(1)
    const is_left = pp && p == pp.left
    const is_right = pp && p == pp.right
    return { is_left, is_right, pp, panc }
  }

  // from a given p, set i all the way down
  const setib = (i, p) => {
    p.block['i blocks'] = i
    p.left.block && setib(i, p.left)
  }

  // from a given p, set j all the way down
  const setjb = (j, p) => {
    p.block['j blocks'] = j
    p.right.block && setjb(j, p.right)
  }

  // from a given p, set k all the way down
  const setkb = (k, p) => {
    p.block['k blocks'] = k
    p.left.block && setjb(k, p.left)
    p.right.block && setib(k, p.right)
  }

  // return p and setter for where your i starts
  const iroot = (p, anc, root) => {
    const { is_left, is_right, pp, panc } = getInfo(p, anc, root)
    return is_left ? iroot(pp, panc, root) : is_right ? { p: pp, f: setkb } : { p, f: setib }
  }

  // return p and setter for where your j starts
  const jroot = (p, anc, root) => {
    const { is_left, is_right, pp, panc } = getInfo(p, anc, root)
    return is_right ? jroot(pp, panc, root) : is_left ? { p: pp, f: setkb } : { p, f: setjb }
  }

  const ir = iroot(p, anc, root)
  ir.f(p.block['i blocks'], ir.p)

  const jr = jroot(p, anc, root)
  jr.f(p.block['j blocks'], jr.p)

  // k always starts here
  setkb(p.block['k blocks'], p)
}

// adjust surroundings to match a param node's h/w
export function fixShape(h, w, p, anc, root) {
  const height = p => p.left ? height(p.left) : p.h
  const width = p => p.right ? width(p.right) : p.w

  const seth = (p, h) => p.left ? seth(p.left, h) : (p.h = h)
  const setw = (p, w) => p.right ? setw(p.right, w) : (p.w = w)

  const pp = anc[0](root)
  p === pp.left ? seth(pp.right, w) : setw(pp.left, h)
  anc.length > 1 && fixShape(height(pp.left), width(pp.right), pp, anc.slice(1), root)
}

export const leftLeaf = p => p.left.matmul ? leftLeaf(p.left) : p.left
export const rightLeaf = p => p.right.matmul ? rightLeaf(p.right) : p.right

// parseExpr, syncExpr

function parseExpr(s) {
  try {
    const node = spec => typeof spec == 'string' ? { name: spec } : make(spec)
    const make = spec => {
      const i = spec[1] == '=' ? 2 : 0
      const rname = r => /\s+/.test(r.name) ? '(' + r.name + ')' : r.name
      const f = (left, x) => {
        const right = node(x)
        return { left, right, name: left.name + ' @ ' + rname(right) }
      }
      const p = spec.slice(i + 1).reduce(f, node(spec[i]))
      i > 0 && (p.name = spec[0])
      return p
    }
    s = '[' + s.replace(/\s+/g, '').
      replace(/(\w+[\w\.\-\!\#\$\%\^\&\/\[\]]*)/g, '"$1"').
      replaceAll('@', ',').
      replaceAll('(', '[').
      replaceAll(')', ']').
      replaceAll('=', ',"=",') + ']'
    let spec = eval?.(s)
    while (spec.length == 1) {
      spec = spec[0]
    }
    return make(spec)
  } catch (e) {
    console.log(`error evaluating '${s}': ${e.message}`)
  }
}

/** Does this tree contain a node kind the expression grammar cannot spell? */
export function treeHasOps(p) {
  return !!p.op || childParamsOf(p).some(treeHasOps)
}

/**
 * A read-only descriptor for a tree that is not an expression.
 *
 * mm's grammar is matmul-only: `parseExpr` maps '@' and nothing else. Handed a
 * tree containing an `add`, `genExpr` would happily print `x @ attn_y` -- a
 * matmul where an add is drawn, which is precisely the kind of plausible lie
 * this repository exists to refuse. So a tree with any op node gets a
 * description instead, and it deliberately contains no '@'.
 */
export function describeTree(p) {
  const counts = {}
  const walk = q => {
    const kind = q.op || (q.matmul ? 'matmul' : q.matmul === false ? 'matrix' : 'matmul')
    counts[kind] = (counts[kind] || 0) + 1
    childParamsOf(q).forEach(walk)
  }
  walk(p)
  const stages = p.op === 'stack' ? `${Object.keys(p.stages).length} stages · ` : ''
  const parts = Object.entries(counts).sort().map(([k, n]) => `${n} ${k}`).join(', ')
  return `${p.name} — ${stages}${parts} (not an expression)`
}

export function syncExpr(params) {
  if (treeHasOps(params)) {
    // Refuse rather than round-trip. `childParams` below rebuilds every node as
    // a matmul or a leaf, so parsing an expression back over this tree would
    // silently turn the adds and the materialized stages into matmuls and
    // change what is drawn without changing what is said about it.
    console.log(`cannot parse an expression over '${params.name}': ` +
      `it contains node kinds the grammar has no notation for (${describeTree(params)})`)
    return false
  }
  if (params.expr == genExpr(params)) {
    return true
  }

  const foundParams = {}

  const findParams = (p, n) => p.name == n ?
    (foundParams[p.name] = p) :
    (p.left && findParams(p.left, n)) ||
    (p.right && findParams(p.right, n)) ||
    undefined

  const childParams = (p, is_left) => {
    const found = findParams(params, p.name)
    if (p.left && p.right) {
      if (found && found.left && found.right) {
        return {
          ...util.copyTree(found),
          left: childParams(p.left, true),
          right: childParams(p.right, false),
          matmul: true,
        }
      } else {
        const cp = {
          epilog: default_epilog,
          anim: defaultAnim(),
          block: defaultBlock(),
          layout: defaultLayout(),
          left: childParams(p.left, true),
          right: childParams(p.right, false),
          name: p.name,
          matmul: true,
        }
        if (found) {
          leftLeaf(cp).h = found.h
          rightLeaf(cp).w = found.w
        }
        return cp
      }
    } else {
      if (found) {
        return !(found.left && found.right) ? util.copyTree(found) : {
          ...(is_left ? leftLeaf(found) : rightLeaf(found)),
          w: rightLeaf(found).w,
          name: p.name,
          matmul: false,
        }
      }
      return {
        ...(is_left ? defaultLeft() : defaultRight()),
        name: p.name,
        matmul: false,
      }
    }
  }

  const fixShapes = (p, anc = [p => p]) => {
    if (p.left && p.right) {
      const path = anc[0]
      if (!foundParams[p.right.name]) {
        fixShapes(p.left, [p => path(p).left].concat(anc))
        fixShapes(p.right, [p => path(p).right].concat(anc))
      } else {
        fixShapes(p.right, [p => path(p).right].concat(anc))
        fixShapes(p.left, [p => path(p).left].concat(anc))
      }
    } else {
      fixShape(p.h, p.w, p, anc.slice(1), new_params)
    }
  }

  const p = parseExpr(params.expr)
  if (!p) {
    return false
  }

  const new_params = {
    name: p.name,
    left: childParams(p.left, true),
    right: childParams(p.right, false)
  }

  fixShapes(new_params)
  util.updateProps(params, new_params)
  setLayoutScheme(params)

  return true
}

export function genExpr(p) {
  if (treeHasOps(p)) {
    return describeTree(p)
  }
  const passign = e => /^\w+\s+=/.test(e) ? `(${e})` : e
  const l = p.left.matmul ? passign(genExpr(p.left)) : p.left.name
  const r = p.right.matmul ? '(' + genExpr(p.right) + ')' : p.right.name
  const expanded = `${l} @ ${r}`
  const named = `${p.left.name} @ ${p.right.name}`
  return p.name == expanded || p.name == named ? expanded : `${p.name} = ${expanded}`
}


