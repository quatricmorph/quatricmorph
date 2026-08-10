"use strict"

import * as THREE from 'three'

import { OrbitControls } from 'three/addons/controls/OrbitControls.js'

import * as viz from './viz.js'
import * as util from './util.js'
import * as gui from './gui.js'
import { defaultParams, type Params } from './params.js'
import { colormapHex, elementHSL, indexValue } from './colormap.js'
import { createEditor } from './interaction.js'

// The element material's magnifier uniform. points.ts hangs `.uniforms` on the
// NodeMaterial as a deliberate alias for this one call site -- NodeMaterial
// neither declares nor reads it, so the cast is the honest description.
const matUniforms = (viz.MATERIAL as any).uniforms

//
// params start with single-mm default, get updated from url params
//

const params: Params = defaultParams()

// we use this when resetting via message passing
const default_params = (() => {
  const p = util.copyTree(params)
  delete p.cam // interferes with camera placement
  return p
})()

function resetParams() {
  Object.entries(util.copyTree(default_params)).forEach(([k, v]) => params[k] = v)
  params.cam = viz.defaultCam()
}

function updateTitle() {
  document.getElementById('info').innerHTML = viz.genExpr(params)
  updateColorbar()
}

//
// The heatmap colorbar.
//
// The ramp, with the *live* range of what is on screen at each end, the active
// encoding named, and the mip level called out when it is not 0. All four are
// claims about what the picture means and none of them is safe to leave
// implicit: |x| and signed are different pictures of the same matrix, and a
// heatmap showing one maxAbs per 16x16 block is not exact and must not read as
// though it were.
//
const fmt = x => !isFinite(x) ? String(x)
  : x === 0 ? '0'
    : Math.abs(x) >= 1e4 || Math.abs(x) < 1e-3 ? x.toExponential(2) : x.toPrecision(4)

function updateColorbar() {
  const el = document.getElementById('colorbar')
  if (!el || !obj) return
  const s = obj.getVizSummary()
  if (!s.heatmaps) {
    el.style.display = 'none'
    return
  }
  el.style.display = 'block'

  // The gradient is sampled through the same functions that fill the shader's
  // lookup -- *including the encoding*, which is the part that is easy to get
  // wrong. 'signed' does not use the seven-stop ramp at all: it fills the LUT
  // from elementHSL, mm's hue-by-sign encoding. Drawing the sequential ramp
  // there would put a legend on screen describing colours nothing is using,
  // and no test could catch it, because the test and the legend would be
  // calling the same (wrong) function.
  const signed = s.encoding === 'signed'
  const swatch = (t: number) => {
    if (!signed) return colormapHex(t)
    const x = indexValue(Math.round(t * 255), 'signed', s.absmin, s.absmax)
    const hsl = elementHSL(x, { absmin: s.absmin, absmax: s.absmax, absdiff: s.absmax - s.absmin },
      params.viz)
    if (!hsl) return '#000000'
    const c = new THREE.Color().setHSL(hsl.h, hsl.s, hsl.l)
    return '#' + [c.r, c.g, c.b]
      .map(v => Math.round(v * 255).toString(16).padStart(2, '0')).join('')
  }
  // more samples for 'signed': its lightness ramp is a sqrt and its hue jumps
  // at zero, neither of which a 12-step gradient reproduces
  const n = signed ? 48 : 12
  const stops = Array.from({ length: n + 1 }, (_, k) =>
    `${swatch(k / n)} ${(k / n * 100).toFixed(1)}%`).join(', ')
  ;(el.querySelector('.ramp') as HTMLElement).style.background =
    `linear-gradient(to right, ${stops})`
  ;(el.querySelector('.lo') as HTMLElement).textContent =
    signed ? fmt(-s.absmax) : fmt(s.absmin)
  ;(el.querySelector('.hi') as HTMLElement).textContent = fmt(s.absmax)

  const enc = s.encoding === 'signed' ? 'signed'
    : s.encoding === 'mixed' ? 'mixed encodings'
      : 'magnitude |x|'
  const lod = s.lod > 1
    ? ` · <span class="reduced">LOD ${Math.log2(s.lod)} — 1 texel per ` +
      `${s.lod}×${s.lod} cells by ${s.reducer}, not exact</span>`
    : ' · LOD 0 — one texel per element'
  ;(el.querySelector('.what') as HTMLElement).innerHTML =
    `${enc} · ${params.viz.sensitivity} range · ` +
    `${s.heatmaps}/${s.mats} matrices as heatmap · ` +
    `${s.texels.toLocaleString()} texels${lod}`
}

const url_info = { json: '', url: urlPrefix(), compressed: '', search_params: '' }

function urlPrefix() {
  return window.location.origin + window.location.pathname
}

function saveUrlInfo() {
  // A staged model scene is tens of thousands of nodes. Serializing it on every
  // camera move would push a multi-hundred-kilobyte history entry per frame, so
  // the URL says what the scene is instead of trying to be it. The page that
  // built the tree is the thing that can rebuild it, and it keeps its own deep
  // link; `open↗` on a model view therefore lands on the page, not the viewer.
  if (params.op) {
    url_info.json = `{"op":"${params.op}","name":${JSON.stringify(params.name)}}`
    url_info.url = url_info.compressed = urlPrefix()
    url_info.search_params = ''
    return
  }
  url_info.json = JSON.stringify(params)
  const prefix = urlPrefix()
  let search_params = util.makeSearchParams(params)
  // not the cleanest place to do it, but - turn compression on when params get big
  if (!params.compress && search_params.toString().length > 2048) {
    params.compress = true
    search_params = util.makeSearchParams(params)
  }
  // url is whatever we're putting in history, compressed is always compressed
  url_info.url = prefix + '?' + search_params
  url_info.compressed = prefix + '?' + util.makeSearchParams({ ...params, compress: true })
  url_info.search_params = '' + search_params
}

function saveUrl() {
  saveUrlInfo()
  // Framed, this window does not own the URL and must not write history.
  //
  // It used to pushState here on every panel change and every camera move (see
  // requestCameraPositionSave), so orbiting a scene left a trail of Back steps
  // whose top-level URL was the embedding page's. One swipe-back -- which is
  // also this viewer's own documented zoom gesture, two fingers on a trackpad
  // -- then took the frame off the scene that page had built and onto a
  // previous one, or, for a staged scene whose URL carries no params at all,
  // onto mm's default demo scene. The chrome outside went on describing the
  // scene it asked for. That was the reported "auto redirect to another view".
  //
  // `saveUrlInfo` above still runs either way, so `url_info` -- the diag
  // panel's json/url/compressed fields and the `getUrlInfo` responder that
  // answers the page's `open↗` -- is as current as it ever was. The embedding
  // page keeps its own deep link (`saveDeepLink` in gpt2page.ts), which is
  // history-neutral for the same reason: it uses replaceState.
  if (window.parent != window) {
    window.parent.postMessage({ search_params: url_info.search_params }, parent.origin)
    return
  }
  window.history.pushState({}, '', url_info.url)
}

// obj

let obj

const getObj = () => obj

// Set while initFromParams is driving, to suppress the zoom-preserving rescale
// below. A module flag rather than an argument to initObj because gui.ts passes
// initObj itself as an onChange handler, so it is routinely called with the
// changed *value* as its first argument -- a `rescale = true` parameter would
// silently take the slider's value as its answer.
let applying_params = false

// Whether the next initObj should fit the camera to what it built. Decided in
// initFromParams and *before* orbit.update(), because that dispatches a change
// event whose handler writes a `target` into params.cam -- so by the time
// initObj runs, "the caller supplied no target" is no longer answerable.
let frame_staged = false

function initObj() {

  let oldmag
  if (obj) {
    const oldsz = util.bbhwd(obj.getBoundingBox())
    oldmag = oldsz.h + oldsz.w + oldsz.d
    scene.remove(obj.group)
    obj.disposeAll()
  }

  // A root carrying `op` is one of the non-matmul node kinds -- today a
  // `stack`, the whole-model staged scene. It presents the same surface the
  // rest of this file drives (group, center, setLegends, initAnimation, bump).
  obj = params.op ?
    viz.buildOpNode(params, getContext(), true) :
    new viz.MatMul(params, getContext())
  obj.group.rotation.x = Math.PI
  obj.center()

  // Only when the scene changed *underneath* a camera the user placed -- a
  // shape edit in the panel, say. initFromParams has just set the camera from
  // params for this exact scene, and rescaling that by how much bigger the new
  // scene is than the old one multiplies a correct camera by a large number:
  // swapping the viewer's small default scene for the 25-stage model pushed it
  // 21.4x further out, past the 10,000 far plane, and drew a black frame.
  if (oldmag && !applying_params) {
    const newsz = util.bbhwd(obj.getBoundingBox())
    const newmag = newsz.h + newsz.w + newsz.d
    const ratio = newmag / oldmag
    if (ratio != 1) {
      console.log(`HEY ratio ${ratio}`)
      camera.position.set(camera.position.x * ratio, camera.position.y * ratio, camera.position.z * ratio)
      orbit.update()
      requestCameraPositionSave()
    }
  }

  obj.setLegends()
  if (obj.stages) {
    // The page owns the timeline chrome, so it needs the stage list and every
    // change of active stage. See the protocol note above RESPONDERS.
    // setStage rebuilds the two stages that changed hands, which replaces
    // their pickable objects and (for leaf stages) their groups — the editor
    // re-indexes, but keeps its edit baselines: the data arrays survive.
    obj.onStageChange = (i, playing) => {
      postStages(i, playing)
      editor.refreshTree(obj)
    }
    // Parked, not playing: a 25-stage scene that starts walking on load has
    // moved on before anyone has read the first stage.
    obj.playing = params.anim['play stages'] === true
    obj.setStage(params.anim.stage | 0)
  } else {
    obj.initAnimation()
  }
  scene.add(obj.group)
  frameStagedScene()
  editor.attach(obj)

  updateTitle()
  postRender()
}

/**
 * Aim the camera at a staged scene's actual pixels.
 *
 * Every other view is framed by the page, which derives a distance from its own
 * approximate bbox and leaves `orbit.target` at the origin. That works because
 * a matmul is one box near the origin. A stack is seven rows of stages spread
 * over thousands of units, and `center()`'s translation happens *after* the
 * group's x-rotation, so the drawn content does not straddle the origin -- the
 * scene ends up off screen even at the right distance.
 *
 * So this measures the world box of what was actually built and fits the camera
 * to it. Deliberately only for `op` roots and only when the caller did not
 * supply its own target: every existing view keeps the framing it has.
 */
function frameStagedScene() {
  if (!frame_staged) return
  frame_staged = false
  obj.group.updateMatrixWorld(true)
  const box = new THREE.Box3().setFromObject(obj.group)
  if (box.isEmpty()) return
  const centre = box.getCenter(new THREE.Vector3())
  const radius = box.getBoundingSphere(new THREE.Sphere()).radius
  // Fit the sphere in the narrower of the two half-angles, with a little air.
  const vfov = THREE.MathUtils.degToRad(camera.fov)
  const hfov = 2 * Math.atan(Math.tan(vfov / 2) * camera.aspect)
  const dist = 1.15 * radius / Math.sin(Math.min(vfov, hfov) / 2)
  const dir = new THREE.Vector3(-1, 1, 1).normalize()
  camera.position.copy(centre).addScaledVector(dir, dist)
  camera.far = Math.max(camera.far, dist + 2 * radius)
  camera.updateProjectionMatrix()
  util.updateProps(orbit.target, centre)
  orbit.update()
}

// -- stage timeline --------------------------------------------------------

function postStages(active = undefined, playing = undefined) {
  if (!obj || !obj.stages || window.parent === window) return
  params.anim.stage = active === undefined ? obj.active : active
  params.anim['play stages'] = playing === undefined ? obj.playing : playing
  window.parent.postMessage({
    stages: {
      list: obj.stageList(),
      active: params.anim.stage,
      playing: params.anim['play stages'],
      summary: obj.getVizSummary(),
    }
  }, '*')
  updateColorbar()
}

// What the renderer actually built, for every view and not only staged ones.
// The page cannot work this out for itself: the LOD ladder is bounded by the
// *viewer's* viewport, which the page does not know, so a claim computed on
// that side would be optimistic about how much resolution is on screen. This
// is measured, after the fact, from the objects that exist.
//
// `mode` is the one field here that is *not* measured: it is the override
// itself — 'auto' | 'spheres' | 'heatmap' — and it is reported because the page
// owns a selector over the same state and re-pushes its value on every rebuild.
// Without this, flipping the panel's toggle and then changing the layer would
// silently restore the selector's stale choice.
//
// It cannot be inferred from the measured fields. A scene where `auto` chose
// heatmap for every matrix reports exactly what 'heatmap' reports, and adopting
// 'heatmap' there would quietly delete the page's 'auto' — a different setting,
// which decides per matrix rather than for all of them.
//
function postRender() {
  if (!obj || window.parent === window) return
  window.parent.postMessage({
    render: { ...obj.getVizSummary(), mode: params.viz['render mode'] || 'auto' },
  }, '*')
}

//
// three.js scene hookup
//

const aspect = () => window.innerWidth / window.innerHeight
const fov = () => 45 / Math.min(1, aspect())

const camera = new THREE.PerspectiveCamera(fov(), aspect(), 5, 10000)
const pointer = new THREE.Vector2(-1, -1)
const raycaster = new THREE.Raycaster()
// raycaster.far = 100
raycaster.params.Points.threshold = 0
raycaster.setFromCamera(pointer, camera)

const scene = new THREE.Scene()

// A black screen-filling quad, used to clear the magnifier's scissor rect.
//
// WebGPU's colour clear is a render-pass loadOp over the whole attachment and
// ignores the scissor, so the lens pass has to keep the colour buffer to
// preserve the frame around it -- which also leaves the lens rect holding the
// previous pass's pixels, visible through the gaps between magnified elements.
// Drawing black is an ordinary draw, so the scissor clips it to exactly the
// rect WebGL's scissored clear used to cover.
//
// Setting scene.backgroundNode would clear the rect too, and with less code,
// but it moves the whole frame onto three.js's linear intermediate-target
// path. That changes where the semi-transparent flow guides blend -- linear
// instead of encoded -- and shifts their colour away from what WebGL produced.
// Measured: an arrow over black lands 242,142,142 that way against WebGL's
// 226,133,133, where this keeps it at 226,133,133 exactly.
//
// The 2x2 plane and the unit orthographic camera are three.js's own
// full-screen-quad pairing (see FullScreenQuad): together they cover exactly
// the viewport, whatever its size. A bare Camera cannot stand in -- the
// renderer calls updateProjectionMatrix() on whatever it is handed.
const lens_clear_scene = new THREE.Scene()
const lens_clear_camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
lens_clear_scene.add(new THREE.Mesh(
  new THREE.PlaneGeometry(2, 2),
  new THREE.MeshBasicMaterial({ color: 0x000000, depthTest: false, depthWrite: false })))

// WebGPU, falling back to this renderer's own WebGL2 backend where WebGPU is
// unavailable -- one code path, no second set of materials. The device request
// is async; init() is awaited before the first frame so nothing renders against
// a half-built backend.
const renderer = new THREE.WebGPURenderer({ antialias: true })
renderer.setPixelRatio(window.devicePixelRatio)
renderer.setSize(window.innerWidth, window.innerHeight)
document.getElementById('container').appendChild(renderer.domElement)
await renderer.init()

// diag info updated on render
const render_info = renderer.info.memory

// object params plus env stuff for text, spotlight, isFacing etc
function getContext() {
  return {
    raycaster: raycaster,
    camera: camera,
    pointer: pointer,
    // The heatmap LOD ladder's screen bound. The viewport's larger dimension
    // in physical pixels is a real upper bound on how many texels any single
    // matrix could show, and unlike a live projected footprint it does not
    // change as the camera moves -- so the ladder does not churn 512 KB
    // uploads on every orbit. See chooseLodFactor in heatmap.ts.
    screenPx: Math.max(window.innerWidth, window.innerHeight) * window.devicePixelRatio,
  }
}

// orbit control

const orbit = new OrbitControls(camera, renderer.domElement)
orbit.keys = { LEFT: orbit.keys.LEFT, RIGHT: orbit.keys.RIGHT } as any   // up/down deliberately unbound
orbit.zoomSpeed = 0.2
orbit.listenToKeyEvents(window)

// The tensor editor: selection, picking, highlights, outliner/inspector
// panels, camera framing, edit stack. It owns no scene objects of the viz
// tree — attach() below hands it each rebuilt root, and it indexes by path.
const editor = createEditor({ scene, camera, orbit, renderer, raycaster, getObj })

const viewState = () => ({
  ...camera.position,
  target: { ...orbit.target }
})

let cam_changing = false

let spin_stash = 0
let view_stash
orbit.addEventListener('start', () => {
  spin_stash = params.anim.spin
  view_stash = viewState()
  params.anim.spin = 0
  cam_changing = true
})

orbit.addEventListener('change', () => {
  params.cam = viewState()
})

// update object labels from view changes
// throttle a little

let mag

const updateSpotlight = () => {
  if (mag) {
    const mag_pointer = pointer.clone()
    mag_pointer.y += params.deco['lens size'] / params.deco.magnification
    raycaster.setFromCamera(mag_pointer, camera)
    raycaster.far = Infinity
  } else {
    raycaster.far = 0
  }
  obj.updateLabels()
}

let label_update_pending = false
const requestLabelUpdate = (legends_only = false) => {
  if (!label_update_pending) {
    label_update_pending = true
    setTimeout(() => {
      label_update_pending = false
      raycaster.setFromCamera(pointer, camera)
      obj.setLegends()
      if (!legends_only) {
        updateSpotlight()
      }
    }, 10)
  }
}

// update params from camera changes
// zooms can blast these

let camera_save_pending = false
let last_camera_save_request = 0
const requestCameraPositionSave = () => {
  last_camera_save_request = performance.now()
  if (!camera_save_pending) {
    camera_save_pending = true
    setTimeout(() => {
      camera_save_pending = false
      const t = performance.now()
      if (t - last_camera_save_request > 250) {
        saveUrl()
      } else {
        requestCameraPositionSave()
      }
    }, 250)
  }
}

orbit.addEventListener('end', () => {
  params.anim.spin = spin_stash
  // these happen only at the end of drags, but multiple times per zoom
  cam_changing = false
  requestLabelUpdate()
  if (JSON.stringify(view_stash) != JSON.stringify(viewState())) {
    requestCameraPositionSave()
  }
})

//
// browser hookups
//

function initFromParams(save = true) {
  save && saveUrlInfo()
  frame_staged = !!params.op && !(params.cam && params.cam.target)
  camera.position.set(params.cam.x, params.cam.y, params.cam.z)
  params.cam.target && util.updateProps(orbit.target, params.cam.target)
  orbit.update()
  initAxes(params.deco.axes)
  applying_params = true
  try {
    initObj()
  } finally {
    applying_params = false
  }

  // gui setup happens here but probably shouldn't
  const callbacks = { initObj, getObj, saveUrl, updateTitle, animPause, animStep, initAxes }
  const info = { url_info, render_info }
  gui.initGui(params, callbacks, info)
}

function initFromSearchParams() {
  const searchParams = new URL(window.location.href).searchParams
  if (searchParams.size > 0) {
    util.updateObjectFromSearchParams(params, searchParams)
  } else {
    resetParams()
  }
  if (params.sync_expr !== undefined) {
    delete params.sync_expr
  }
  params.expr = viz.genExpr(params)
  initFromParams(false)
}

// Top-level only, for the same reason `saveUrl` does not push when framed: this
// window's URL is not a place the user navigated to. A staged scene arrives over
// postMessage and leaves no params behind at all, so re-reading the frame's URL
// on a back navigation would fall into `resetParams()` and silently replace the
// checkpoint scene with mm's default one.
window.addEventListener('popstate', () => {
  if (window.parent != window) return
  initFromSearchParams()
}, false)

window.addEventListener('resize', () => {
  camera.fov = fov()
  camera.aspect = aspect()
  camera.updateProjectionMatrix()
  renderer.setSize(window.innerWidth, window.innerHeight)
  syncVizToRenderer(true)
})

let pointer_start
let pointer_moved
let pointer_move_timeout

let clientX = 0
let clientY = 0
window.addEventListener('pointermove', e => {
  clientX = e.clientX
  clientY = e.clientY
  pointer.x = e.clientX / window.innerWidth * 2 - 1
  pointer.y = -(e.clientY / window.innerHeight * 2 - 1)
  pointer_moved = true
  if (!cam_changing) {
    requestLabelUpdate()
  }
})

window.addEventListener('pointerdown', e => {
  clientX = e.clientX
  clientY = e.clientY
  pointer.x = e.clientX / window.innerWidth * 2 - 1
  pointer.y = -(e.clientY / window.innerHeight * 2 - 1)
  pointer_moved = false
  pointer_start = Date.now()
  pointer_move_timeout = setTimeout(() => {
    if (!pointer_moved && e.target === renderer.domElement) {
      mag = true
      orbit.enabled = false
      cam_changing = false
      updateSpotlight()
    }
  }, 500)
})

window.addEventListener('pointerup', e => {
  clearTimeout(pointer_move_timeout)
  mag = false
  orbit.enabled = true
  updateSpotlight()
})

const key_funcs = {
  'Space': () => {
    let init = false
    if (params.anim.alg == 'none') {
      params.anim.alg = last_anim_alg
      init = true
    } else if (anim_pause) {
      animPause(false)
    } else {
      last_anim_alg = params.anim.alg
      params.anim.alg = 'none'
      init = true
    }
    if (params.anim.spin == 0) {
      params.anim.spin = last_anim_spin
      init = true
    } else {
      last_anim_spin = params.anim.spin
      params.anim.spin = 0
      init = true
    }
    init && initObj()
  },
  'ArrowUp': () => {
    const e = new WheelEvent('wheel', { deltaY: 1 })
    orbit.domElement.dispatchEvent(e)
  },
  'ArrowDown': () => {
    const e = new WheelEvent('wheel', { deltaY: -1 })
    orbit.domElement.dispatchEvent(e)
  },
  'KeyP': () => { // p
    if (params.anim.alg != 'none') {
      animPause(!anim_pause)
    }
  },
  'KeyS': () => { // s
    if (params.anim.alg != 'none') {
      animPause(true)
      animStep()
    }
  },
}

window.addEventListener('keydown', e => {
  if (e.ctrlKey) {
    mag = true
    updateSpotlight()
    return
  }
  const kf = key_funcs[e.code]
  kf && kf(e)
})

window.addEventListener('keyup', e => {
  if (mag) {
    mag = false
    updateSpotlight()
  }
})

// comms w/outside world (we're in an iframe, e.g.)

// Each responder takes (payload, event). The event used to be read off the
// global `window.event`, which is deprecated and only set while a dispatch is
// in flight; the listener below already has the real one, so it is passed in.
const RESPONDERS = {
  getUrlInfo: (_, event) => {
    // console.log(`HEY getUrlInfo called`)
    event.source.postMessage({ url_info }, event.origin)
  },
  getParams: (_, event) => {
    // console.log(`HEY getParams called`)
    event.source.postMessage({ params }, event.origin)
  },
  // Replace the whole params object rather than merging onto it. A tree of a
  // different *shape* -- a stack where there was a matmul -- leaves stale
  // `left`/`right` nodes behind under a merge, and mm would go on drawing them.
  // The page uses this instead of `?params=` for the model view, whose tree is
  // far too big to be a URL.
  setParams: ({ props = {} as Params, reset = false, replace = false }) => {
    console.log(`HEY setParams called reset ${reset} replace ${replace}`)
    reset && resetParams()
    if (replace) {
      Object.keys(params).forEach(k => delete params[k])
    }

    if (props.sync_expr) {
      params.expr = props.expr
      viz.syncExpr(params)
      delete props.sync_expr
    }
    util.updatePropsRec(params, props)
    params.expr = viz.genExpr(params)
    if (props.layout?.scheme) {
      viz.setLayoutScheme(params)
    }
    initFromParams()
  },

  // Stage control for a `stack` root. Deliberately *not* routed through
  // setParams: that rebuilds the whole scene, and a scrubbing timeline cannot
  // be driven a rebuild at a time.
  setStage: ({ index = undefined, playing = undefined, step = 0 }) => {
    if (!obj || !obj.stages) return
    if (playing !== undefined) obj.playing = !!playing
    const target = index === undefined ? obj.active + step : index
    obj.setStage(target, obj.playing)
  },

  getStages: (_, event) => {
    if (obj && obj.stages) {
      event.source.postMessage({
        stages: {
          list: obj.stageList(), active: obj.active,
          playing: obj.playing, summary: obj.getVizSummary(),
        }
      }, event.origin)
    }
  },
}

window.addEventListener('message', event => {
  Object.entries(event.data).forEach(([k, v]) => {
    const r = RESPONDERS[k]
    r && r(v, event)
  })
})

// diag info updated on resize
const display_info = { x: 0, y: 0, z: 0, devicePixelRatio: 0 }

function syncVizToRenderer(reinit = false) {
  renderer.setPixelRatio(window.devicePixelRatio)
  const size = renderer.getSize(new THREE.Vector3() as any)
  viz.setElemSize(size, window.devicePixelRatio)
  util.updateProps(display_info, size)
  display_info.devicePixelRatio = window.devicePixelRatio
  reinit && initObj()
}

syncVizToRenderer() // first time thru

const pixel_ratio_watcher = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`)
pixel_ratio_watcher.addEventListener('change', _ => syncVizToRenderer(true))

//
// animation
//

// for spacebar control
let last_anim_alg = params.anim.alg
let last_anim_spin = params.anim.spin

// animation loop circuit breaker
let anim_pause = false
function animPause(p) {
  anim_pause = p
}

let anim_step = false
function animStep() {
  if (anim_pause) {
    anim_step = true
  }
}

// axes
const axes = util.axes()
function initAxes(enabled) {
  if (enabled) {
    scene.add(axes)
  } else {
    scene.remove(axes)
  }
}

let last_render = 0, last_anim = 0

function animate() {
  const t = performance.now()

  // Camera transitions (frame selected/all, presets, pivot moves) tween
  // through the editor's rig; a no-op when nothing is in flight.
  editor.update(t)
  // A staged scene keeps ticking while it is playing even with the algorithm
  // set to 'none': the thing being animated then is the *timeline*, not any one
  // matmul, and the stage driver dwells on each stage rather than sweeping it.
  const anim_live = params.anim.alg != 'none' || (obj.stages && obj.playing)
  if (anim_live &&
    (anim_step || !anim_pause) &&
    (t - last_render) > (1000 / params.anim.speed)) {
    obj.bump && obj.bump()
    last_render = t
    anim_step = false
  }

  if (params.anim.spin != 0) {
    const rad = (last_anim - t) * params.anim.spin / 20000
    const [cos, sin] = [Math.cos(rad), Math.sin(rad)]
    const { x, z } = camera.position
    util.updateProps(camera.position, { x: cos * x + sin * z, z: cos * z - sin * x })
    camera.lookAt(orbit.target.x, orbit.target.y, orbit.target.z)
    requestLabelUpdate(true)
  }

  last_anim = t

  util.updateProps(render_info, renderer.info.memory)
  renderer.setViewport(0, 0, window.innerWidth, window.innerHeight)
  renderer.render(scene, camera)

  if (mag) {
    const m = params.deco.magnification
    const size = window.innerHeight * params.deco['lens size']
    const x = clientX - size / 2
    // Top-left origin. WebGPURenderer takes setViewport/setScissor that way on
    // both of its backends -- the WebGL fallback flips y itself in
    // WebGLBackend.updateViewport -- whereas the legacy WebGLRenderer took
    // GL's bottom-left origin, which is what `innerHeight - clientY` gave.
    // Passing the old value put the lens a lens-height too low.
    const y = clientY - size
    const offsetX = clientX - size / 2 / m
    const offsetY = clientY - size / m

    const mag_camera = camera.clone()
    mag_camera.setViewOffset(
      window.innerWidth,
      window.innerHeight,
      offsetX,
      offsetY,
      size / m,
      size / m
    )

    renderer.setViewport(x, y, size, size)
    renderer.setScissorTest(true)
    renderer.setScissor(x, y, size, size)

    // The lens is a second pass over the same scene, drawn into a scissored
    // corner of the frame the pass above just produced. Under WebGL the colour
    // clear honoured the scissor, so the rest of the frame survived by itself.
    // WebGPU clears through the render pass's loadOp, which applies to the
    // whole attachment and ignores the scissor rect -- leaving autoClearColor
    // on here would wipe the frame and leave nothing but the lens. Loading the
    // colour instead preserves it, and the scissor confines what the lens
    // draws. Depth still clears, which the lens needs: it renders from a
    // different camera, and testing against the previous pass's depth would
    // punch holes in it.
    // keep the colour written above, clear depth, and paint the lens rect black
    renderer.autoClearColor = false
    renderer.render(lens_clear_scene, lens_clear_camera)

    // now keep both: the black backing just drawn, and the depth it left clear
    renderer.autoClear = false

    matUniforms.mag.value = m
    renderer.render(scene, mag_camera)
    matUniforms.mag.value = 1.0

    renderer.autoClear = true
    renderer.autoClearColor = true
    renderer.setScissorTest(false)
  }

  requestAnimationFrame(animate)
}

// instructions hover

// Registered rather than assigned to window.onload, and run directly if the
// document has already loaded. Awaiting the WebGPU device above makes the rest
// of this module a microtask, which can land after the load event has already
// fired -- an assignment then installs a handler nothing will ever call, and
// the panel stays at its stylesheet `display: none` forever.
const initInstructions = () => {
  const instr = document.getElementById('instructions')
  const instr_content = document.getElementById('instructions-content')
  const min_content = document.getElementById('minimized')
  const min_button = document.getElementById('minimize')
  const max_button = document.getElementById('maximize')

  const show = () => {
    instr_content.style.display = "block"
    min_content.style.display = "none"
    min_button.style.display = "block"
    max_button.style.display = "none"
  }

  const hide = () => {
    instr_content.style.display = "none"
    min_content.style.display = "block"
    min_button.style.display = "none"
    max_button.style.display = "block"
  }

  min_button.onclick = hide
  instr_content.onclick = hide
  max_button.onclick = show

  instr.style.display = "block"
  // Minimized on arrival, not expanded. The panel is fixed at the top left and
  // is ~480px wide; the outliner sits at left:12 top:48 underneath it, and
  // since the checkpoint pages' own sidebar went away that outliner is the only
  // scene tree there is. Covering it on every load would hide the one panel
  // that selects and hides. The `>` chip is 36x40 at the very corner, clear of
  // the outliner, and one click still brings the whole table back.
  hide()
}

if (document.readyState === 'complete') {
  initInstructions()
} else {
  window.addEventListener('load', initInstructions)
}

//
// run
//

initFromSearchParams()
animate()

// Announce readiness to an embedding page.
//
// The iframe's `load` event is not a safe moment to post to this window: this
// module has a top-level `await renderer.init()`, and a module with top-level
// await finishes *after* `load` fires. A page that pushed its scene on `load`
// would post into a window whose message listener did not exist yet, and the
// message would be dropped with nothing to say so -- which is exactly what the
// staged model view did before this line existed, coming up with the viewer's
// own default scene instead.
if (window.parent !== window) {
  window.parent.postMessage({ ready: true }, '*')
}
