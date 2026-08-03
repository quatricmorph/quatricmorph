// @ts-nocheck
import * as THREE from 'three'
import * as viz from '../viz.js'
import * as util from '../util.js'
import * as gui from '../gui.js'
import { createDefaultParams } from './default-params.js'
import { createUrlInfo, saveUrl as persistUrl, saveUrlInfo } from './url.js'
import { createScene } from './scene.js'
import { setupInstructions } from './instructions.js'
import { validateMatmulDims } from '../math/validate.js'
import { cameraPresetPose, gridRulerFromParams, mulVolumeExtent } from '../layout/grid-ruler.js'
import { selectOutput, clearSelection as emptySelection, pathFromSelection } from '../interaction/selection.js'

/**
 * Wire scene, params, input, animation, and MVP GUI into Quatricmorph.
 */
export function createApp() {
  const params = createDefaultParams()
  const default_params = (() => {
    const p = util.copyTree(params)
    delete p.cam
    return p
  })()

  function resetParams() {
    Object.entries(util.copyTree(default_params)).forEach(([k, v]) => params[k] = v)
    params.cam = viz.defaultCam()
  }

  function updateTitle() {
    const el = document.getElementById('info')
    if (el) el.innerHTML = viz.genExpr(params)
  }

  function setValidationMessage(msg) {
    const el = document.getElementById('validation')
    if (!el) return
    el.textContent = msg || ''
    el.style.display = msg ? 'block' : 'none'
  }

  function setHoverInfo(text) {
    const el = document.getElementById('hover-info')
    if (!el) return
    el.textContent = text || ''
    el.style.display = text ? 'block' : 'none'
  }

  const url_info = createUrlInfo()
  const saveUrl = () => persistUrl(params, url_info)

  const {
    aspect, fov, camera, pointer, raycaster, scene, renderer,
    render_info, getContext, orbit, viewState,
  } = createScene()

  let obj
  const getObj = () => obj
  let selection = emptySelection()
  let lastValidParams = util.copyTree(params)

  let cam_changing = false
  let spin_stash = 0
  let view_stash
  let mag
  let clientX = 0
  let clientY = 0
  let last_anim_alg = 'dotprod (row major)'
  let last_anim_spin = params.anim.spin
  let anim_pause = false
  let anim_step = false
  let anim_prev = false

  function animPause(p) {
    anim_pause = p
  }

  function animStep() {
    if (anim_pause) {
      anim_step = true
    }
  }

  function animPrevStep() {
    // Deterministic rewind: rebuild with same alg and pause at start of cycle.
    // Full micro-step reverse is approximated by resetting calculation.
    resetCalculation()
    animPause(true)
  }

  function resetCalculation() {
    selection = emptySelection()
    const was = params.anim.alg
    params.anim.alg = was === 'none' ? 'dotprod (row major)' : was
    anim_pause = true
    anim_step = false
    initObj()
    applySelectionHighlight()
  }

  function fitView() {
    if (!obj) return
    const bb = obj.getBoundingBox()
    const size = new THREE.Vector3()
    const center = new THREE.Vector3()
    bb.getSize(size)
    bb.getCenter(center)
    const maxDim = Math.max(size.x, size.y, size.z, 1)
    const dist = maxDim * 2.2
    camera.position.set(center.x - dist * 0.7, center.y + dist * 0.55, center.z + dist * 0.7)
    orbit.target.copy(center)
    orbit.update()
    params.cam = viewState()
    saveUrl()
  }

  function resetView() {
    const preset = params.mvp?.cameraPreset || 'volume'
    setCameraPreset(preset)
  }

  function setCameraPreset(presetName) {
    const gridCfg = gridRulerFromParams(params.layout)
    const extent = mulVolumeExtent(
      params.left.h,
      params.left.w,
      params.right.w,
      gridCfg,
    )
    const pose = cameraPresetPose(presetName, extent)
    camera.position.set(pose.position.x, pose.position.y, pose.position.z)
    orbit.target.set(pose.target.x, pose.target.y, pose.target.z)
    orbit.update()
    params.cam = viewState()
    params.mvp = params.mvp || {}
    params.mvp.cameraPreset = presetName
  }

  async function copyShareLink() {
    saveUrlInfo(params, url_info)
    try {
      await navigator.clipboard.writeText(url_info.url)
      setValidationMessage('Share link copied.')
      setTimeout(() => setValidationMessage(''), 1500)
    } catch {
      setValidationMessage(url_info.url)
    }
  }

  function clearSelection() {
    selection = emptySelection()
    applySelectionHighlight()
  }

  function applySelectionHighlight() {
    if (!obj || obj.left?.params?.matmul) return
    // Reset colors then bump path
    try {
      obj.left.setColorsAndSizes?.()
      obj.right.setColorsAndSizes?.()
      obj.result.setColorsAndSizes?.()
      const path = pathFromSelection(selection)
      if (path) {
        obj.left.bumpColor?.(path.aRow, undefined)
        obj.right.bumpColor?.(undefined, path.bCol)
        obj.result.bumpColor?.(path.cCell.i, path.cCell.j)
      }
    } catch (_) { /* ignore during teardown */ }
  }

  const axes = util.axes()
  function initAxes(enabled) {
    if (enabled) {
      scene.add(axes)
    } else {
      scene.remove(axes)
    }
  }

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

  const updateSpotlight = () => {
    if (!obj) {
      return
    }
    if (mag) {
      const mag_pointer = pointer.clone()
      mag_pointer.y += params.deco['lens size'] / params.deco.magnification
      raycaster.setFromCamera(mag_pointer, camera)
      raycaster.far = Infinity
    } else {
      raycaster.far = 0
    }
    obj.updateLabels()
    updateHoverPanel()
  }

  function updateHoverPanel() {
    if (!obj || !obj.result?.points) {
      setHoverInfo('')
      return
    }
    // Magnifier mode sets raycaster.far = 0 when inactive; restore range for hover picks (VIZ-07).
    raycaster.far = Infinity
    raycaster.setFromCamera(pointer, camera)
    raycaster.params.Points.threshold = Math.max(params.deco.spotlight || 0, 0.5)
    const mats = [
      { mat: obj.left, name: 'A', shape: [obj.H, obj.D] },
      { mat: obj.right, name: 'B', shape: [obj.D, obj.W] },
      { mat: obj.result, name: 'C', shape: [obj.H, obj.W] },
    ]
    for (const { mat, name, shape } of mats) {
      if (!mat?.points) continue
      const hits = raycaster.intersectObject(mat.points)
      if (hits.length) {
        const index = hits[0].index
        const i = Math.floor(index / mat.W)
        const j = index % mat.W
        const val = mat.getData(i, j)
        setHoverInfo(
          `Tensor: ${name}\nIndex: [${i}, ${j}]\nValue: ${val}\nShape: [${shape[0]}, ${shape[1]}]`,
        )
        return
      }
    }
    setHoverInfo('')
  }

  let label_update_pending = false
  const requestLabelUpdate = (legends_only = false) => {
    if (!label_update_pending) {
      label_update_pending = true
      setTimeout(() => {
        label_update_pending = false
        if (!obj) {
          return
        }
        raycaster.setFromCamera(pointer, camera)
        obj.setLegends()
        if (!legends_only) {
          updateSpotlight()
        }
      }, 10)
    }
  }

  function initObj() {
    const check = validateMatmulDims(
      params.left.h, params.left.w, params.right.h, params.right.w,
    )
    if (!check.ok) {
      setValidationMessage(check.message)
      // Preserve last valid scene — do not construct partial Three objects
      return
    }

    let oldmag
    if (obj) {
      const oldsz = util.bbhwd(obj.getBoundingBox())
      oldmag = oldsz.h + oldsz.w + oldsz.d
      scene.remove(obj.group)
      obj.disposeAll()
      obj = undefined
    }

    try {
      obj = new viz.MatMul(params, getContext())
      obj.group.rotation.x = Math.PI
      obj.center()
      lastValidParams = util.copyTree(params)
      setValidationMessage('')
    } catch (e) {
      setValidationMessage(e.message || String(e))
      // Restore last valid if available
      if (lastValidParams) {
        try {
          obj = new viz.MatMul(lastValidParams, getContext())
          obj.group.rotation.x = Math.PI
          obj.center()
        } catch (_) {
          obj = undefined
        }
      }
      return
    }

    if (oldmag) {
      const newsz = util.bbhwd(obj.getBoundingBox())
      const newmag = newsz.h + newsz.w + newsz.d
      const ratio = newmag / oldmag
      if (ratio != 1) {
        camera.position.set(camera.position.x * ratio, camera.position.y * ratio, camera.position.z * ratio)
        orbit.update()
        requestCameraPositionSave()
      }
    }

    obj.setLegends()
    obj.initAnimation()
    scene.add(obj.group)
    applySelectionHighlight()
    updateTitle()
  }

  orbit.addEventListener('start', () => {
    spin_stash = params.anim.spin
    view_stash = viewState()
    params.anim.spin = 0
    cam_changing = true
  })

  orbit.addEventListener('change', () => {
    params.cam = viewState()
  }, false)

  orbit.addEventListener('end', () => {
    params.anim.spin = spin_stash
    cam_changing = false
    requestLabelUpdate()
    if (JSON.stringify(view_stash) != JSON.stringify(viewState())) {
      requestCameraPositionSave()
    }
  })

  function initFromParams(save = true) {
    save && saveUrlInfo(params, url_info)
    camera.position.set(params.cam.x, params.cam.y, params.cam.z)
    params.cam.target && util.updateProps(orbit.target, params.cam.target)
    orbit.update()
    initAxes(params.deco.axes)
    initObj()

    const callbacks = {
      initObj,
      getObj,
      saveUrl,
      updateTitle,
      animPause,
      animStep,
      animPrevStep,
      resetCalculation,
      resetView,
      fitView,
      setCameraPreset,
      copyShareLink,
      setValidationMessage,
      clearSelection,
    }
    const info = { url_info, render_info }
    gui.initGui(params, callbacks, info)
  }

  function initFromSearchParams() {
    const searchParams = new URL(window.location.href).searchParams
    if (searchParams.size > 0) {
      try {
        util.updateObjectFromSearchParams(params, searchParams)
      } catch (e) {
        console.log('invalid URL params, falling back to defaults', e)
        resetParams()
        setValidationMessage('Invalid share URL — loaded defaults.')
      }
    } else {
      resetParams()
    }
    if (params.sync_expr !== undefined) {
      delete params.sync_expr
    }
    // Ensure leaf names for MVP
    params.left.name ||= 'A'
    params.right.name ||= 'B'
    params.name ||= 'C'
    params.expr = viz.genExpr(params)
    initFromParams(false)
  }

  window.addEventListener('popstate', initFromSearchParams, false)

  window.addEventListener('resize', () => {
    camera.fov = fov()
    camera.aspect = aspect()
    camera.updateProjectionMatrix()
    renderer.setSize(window.innerWidth, window.innerHeight)
    syncVizToRenderer(true)
  })

  let pointer_moved
  let pointer_move_timeout
  let pointer_down = false

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
    pointer_down = true
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

    // Click select C[i,j] path when little movement
    if (pointer_down && !pointer_moved && e.target === renderer.domElement && obj?.result?.points) {
      raycaster.setFromCamera(pointer, camera)
      raycaster.params.Points.threshold = Math.max(params.deco.spotlight || 0, 0.8)
      const hits = raycaster.intersectObject(obj.result.points)
      if (hits.length) {
        const index = hits[0].index
        const i = Math.floor(index / obj.result.W)
        const j = index % obj.result.W
        selection = selectOutput(i, j, obj.H, obj.W)
        applySelectionHighlight()
      }
    }
    pointer_down = false
  })

  const key_funcs = {
    Space: () => {
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
    ArrowUp: () => {
      orbit.domElement.dispatchEvent(new WheelEvent('wheel', { deltaY: 1 }))
    },
    ArrowDown: () => {
      orbit.domElement.dispatchEvent(new WheelEvent('wheel', { deltaY: -1 }))
    },
    KeyP: () => {
      if (params.anim.alg != 'none') {
        animPause(!anim_pause)
      }
    },
    KeyS: () => {
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

  window.addEventListener('keyup', () => {
    if (mag) {
      mag = false
      updateSpotlight()
    }
  })

  let messageEvent
  const RESPONDERS = {
    getUrlInfo: () => {
      ;(messageEvent?.source)?.postMessage({ url_info }, messageEvent.origin)
    },
    getParams: () => {
      ;(messageEvent?.source)?.postMessage({ params }, messageEvent.origin)
    },
    setParams: ({ props = {}, reset = false } = {}) => {
      reset && resetParams()
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
  }

  window.addEventListener('message', (event) => {
    messageEvent = event
    Object.entries(event.data).forEach(([k, v]) => {
      const r = RESPONDERS[k]
      r && r(v)
    })
  })

  const display_info = { x: 0, y: 0, z: 0, devicePixelRatio: 0 }

  function syncVizToRenderer(reinit = false) {
    renderer.setPixelRatio(window.devicePixelRatio)
    const size = renderer.getSize(new THREE.Vector3())
    viz.setElemSize(size, window.devicePixelRatio)
    util.updateProps(display_info, size)
    display_info.devicePixelRatio = window.devicePixelRatio
    reinit && initObj()
  }

  syncVizToRenderer()

  const pixel_ratio_watcher = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`)
  pixel_ratio_watcher.addEventListener('change', () => syncVizToRenderer(true))

  let last_render = 0
  let last_anim = 0

  function animate() {
    const t = performance.now()
    if (obj && params.anim.alg != 'none' &&
      (anim_step || !anim_pause) &&
      (t - last_render) > (1000 / params.anim.speed)) {
      obj.bump()
      last_render = t
      anim_step = false
      // advancing calculation clears sticky selection path
      if (selection.kind !== 'none') {
        selection = emptySelection()
      }
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
      const y = window.innerHeight - clientY
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

      viz.MATERIAL.uniforms.mag.value = m
      renderer.render(scene, mag_camera)
      viz.MATERIAL.uniforms.mag.value = 1.0

      renderer.setScissorTest(false)
    }

    requestAnimationFrame(animate)
  }

  window.onload = setupInstructions

  initFromSearchParams()
  animate()

  return { params, scene, camera, renderer, getObj }
}
