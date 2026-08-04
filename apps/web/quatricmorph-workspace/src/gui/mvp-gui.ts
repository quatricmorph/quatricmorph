// @ts-nocheck
/**
 * Quatricmorph MVP GUI (VIZ-09) — hides nested expr / attention / LoRA / diagnostics.
 * Algorithm fixed to output-cell dot product.
 */
import { GUI } from 'lil-gui'
import * as viz from '../viz.js'
import {
  parseMatrixText,
  matrixToText,
  fillPreset,
  DEFAULT_A,
  DEFAULT_B,
} from '../math/index.js'
import { validateMatmulDims } from '../math/validate.js'

let gui

function leafText(leaf) {
  if (typeof leaf.valuesText === 'string' && leaf.valuesText.trim()) {
    return leaf.valuesText
  }
  return ''
}

function applyTextToLeaf(leaf, text) {
  const parsed = parseMatrixText(text)
  if (!parsed.ok) return parsed
  leaf.h = parsed.rows
  leaf.w = parsed.cols
  leaf.valuesText = matrixToText(parsed.data)
  leaf.init = 'values'
  return parsed
}

export function initGui(params, callbacks, info) {
  const {
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
  } = callbacks

  gui && gui.destroy()
  gui = new GUI({ title: 'Quatricmorph' }).open(true)

  const state = {
    aRows: params.left.h,
    aCols: params.left.w,
    bRows: params.right.h,
    bCols: params.right.w,
    aText: leafText(params.left) || matrixToText(DEFAULT_A),
    bText: leafText(params.right) || matrixToText(DEFAULT_B),
    unlockBRows: !(params.mvp?.syncBk ?? true),
    preset: 'small example',
    showValues: params.deco.spotlight > 0,
    showFrames: !!params.deco.shape,
    showRowGuides: params.deco['row guides'] > 0,
    showFlowGuides: params.deco['flow guides'] > 0,
    animSpeed: params.anim.speed,
    markerScale: params.viz['elem scale'],
    cellSize: params.layout.cellSize ?? 1,
    gap: params.layout.gap,
    cameraPreset: params.mvp?.cameraPreset || 'volume',
  }

  function syncDimsFromLeaves() {
    state.aRows = params.left.h
    state.aCols = params.left.w
    state.bRows = params.right.h
    state.bCols = params.right.w
    state.aText = leafText(params.left)
    state.bText = leafText(params.right)
  }

  function resizeText(text, rows, cols) {
    const parsed = parseMatrixText(text)
    const old = parsed.ok ? parsed.data : []
    const next = []
    for (let i = 0; i < rows; i++) {
      const row = []
      for (let j = 0; j < cols; j++) {
        row.push(old[i]?.[j] ?? 0)
      }
      next.push(row)
    }
    return matrixToText(next)
  }

  function applyShapesAndRebuild() {
    params.left.h = Math.max(1, Math.floor(state.aRows))
    params.left.w = Math.max(1, Math.floor(state.aCols))
    if (!state.unlockBRows) {
      state.bRows = params.left.w
    }
    params.right.h = Math.max(1, Math.floor(state.bRows))
    params.right.w = Math.max(1, Math.floor(state.bCols))

    const check = validateMatmulDims(
      params.left.h, params.left.w, params.right.h, params.right.w,
    )
    if (!check.ok) {
      setValidationMessage?.(check.message)
      return
    }
    setValidationMessage?.('')

    params.left.valuesText = resizeText(state.aText, params.left.h, params.left.w)
    params.right.valuesText = resizeText(state.bText, params.right.h, params.right.w)
    params.left.init = 'values'
    params.right.init = 'values'
    state.aText = params.left.valuesText
    state.bText = params.right.valuesText
    params.expr = viz.genExpr(params)
    clearSelection?.()
    initObj()
    updateTitle?.()
    saveUrl?.()
    syncDimsFromLeaves()
  }

  function applyMatrixTexts() {
    const a = applyTextToLeaf(params.left, state.aText)
    if (!a.ok) {
      setValidationMessage?.(a.message)
      return
    }
    const b = applyTextToLeaf(params.right, state.bText)
    if (!b.ok) {
      setValidationMessage?.(b.message)
      return
    }
    if (!state.unlockBRows && params.right.h !== params.left.w) {
      setValidationMessage?.(
        `B rows (${params.right.h}) must equal A columns (${params.left.w}). ` +
        `Enable Unlock B rows or fix the text.`,
      )
      return
    }
    const check = validateMatmulDims(
      params.left.h, params.left.w, params.right.h, params.right.w,
    )
    if (!check.ok) {
      setValidationMessage?.(check.message)
      return
    }
    setValidationMessage?.('')
    state.aRows = params.left.h
    state.aCols = params.left.w
    state.bRows = params.right.h
    state.bCols = params.right.w
    params.expr = viz.genExpr(params)
    clearSelection?.()
    initObj()
    updateTitle?.()
    saveUrl?.()
  }

  const input = gui.addFolder('Input A @ B')

  /** Live dim check so invalid shapes surface before/without waiting for blur (VIZ-01). */
  function validateDimsMessage() {
    const check = validateMatmulDims(
      Math.max(1, Math.floor(state.aRows)),
      Math.max(1, Math.floor(state.aCols)),
      Math.max(1, Math.floor(state.bRows)),
      Math.max(1, Math.floor(state.bCols)),
    )
    setValidationMessage?.(check.ok ? '' : check.message)
  }

  input.add(state, 'aRows', 1, 32, 1).name('A rows')
    .onChange(validateDimsMessage)
    .onFinishChange(applyShapesAndRebuild)
  input.add(state, 'aCols', 1, 32, 1).name('A columns')
    .onChange(() => {
      if (!state.unlockBRows) state.bRows = state.aCols
      validateDimsMessage()
    })
    .onFinishChange(() => {
      if (!state.unlockBRows) state.bRows = state.aCols
      applyShapesAndRebuild()
    })
  input.add(state, 'bRows', 1, 32, 1).name('B rows')
    .onChange(validateDimsMessage)
    .onFinishChange(applyShapesAndRebuild)
  input.add(state, 'bCols', 1, 32, 1).name('B columns')
    .onChange(validateDimsMessage)
    .onFinishChange(applyShapesAndRebuild)
  input.add(state, 'unlockBRows').name('Unlock B rows').onChange((v) => {
    params.mvp = params.mvp || {}
    params.mvp.syncBk = !v
    if (!v) {
      state.bRows = state.aCols
      applyShapesAndRebuild()
    }
    saveUrl?.()
  })

  input.add(state, 'aText').name('A values').onFinishChange(applyMatrixTexts)
  input.add(state, 'bText').name('B values').onFinishChange(applyMatrixTexts)

  const presets = {
    random: 'random',
    identity: 'identity',
    sequential: 'sequential',
    zeros: 'zeros',
    ones: 'ones',
    'small example': 'small',
  }
  input.add(state, 'preset', Object.keys(presets)).name('Preset').onChange((label) => {
    const name = presets[label]
    if (name === 'small') {
      params.left.h = DEFAULT_A.length
      params.left.w = DEFAULT_A[0].length
      params.left.valuesText = matrixToText(DEFAULT_A)
      params.right.h = DEFAULT_B.length
      params.right.w = DEFAULT_B[0].length
      params.right.valuesText = matrixToText(DEFAULT_B)
    } else {
      params.left.valuesText = matrixToText(fillPreset(params.left.h, params.left.w, name))
      params.right.valuesText = matrixToText(fillPreset(params.right.h, params.right.w, name))
    }
    params.left.init = 'values'
    params.right.init = 'values'
    syncDimsFromLeaves()
    setValidationMessage?.('')
    clearSelection?.()
    initObj()
    updateTitle?.()
    saveUrl?.()
  })

  const anim = gui.addFolder('Calculation')
  const animApi = {
    Play: () => {
      params.anim.alg = 'dotprod (row major)'
      animPause?.(false)
      initObj()
      saveUrl?.()
    },
    Pause: () => animPause?.(true),
    Step: () => {
      if (params.anim.alg === 'none') {
        params.anim.alg = 'dotprod (row major)'
        initObj()
      }
      animPause?.(true)
      animStep?.()
    },
    'Previous Step': () => animPrevStep?.(),
    'Reset Calculation': () => resetCalculation?.(),
  }
  Object.keys(animApi).forEach((k) => anim.add(animApi, k))
  anim.add(state, 'animSpeed', 1, 60, 1).name('Speed').onChange((v) => {
    params.anim.speed = v
    saveUrl?.()
  })

  const view = gui.addFolder('View')
  const viewApi = {
    'Reset View': () => resetView?.(),
    'Fit View': () => fitView?.(),
    'Copy Share Link': () => copyShareLink?.(),
    'Clear Selection': () => clearSelection?.(),
  }
  Object.keys(viewApi).forEach((k) => view.add(viewApi, k))
  view.add(state, 'cameraPreset', ['isometric', 'front', 'top', 'volume'])
    .name('Camera')
    .onChange((p) => {
      params.mvp = params.mvp || {}
      params.mvp.cameraPreset = p
      setCameraPreset?.(p)
      saveUrl?.()
    })

  const deco = gui.addFolder('Display')
  deco.add(state, 'showValues').name('Show values').onChange((v) => {
    params.deco.spotlight = v ? 2 : 0
    getObj()?.updateLabels?.(params)
    saveUrl?.()
  })
  deco.add(state, 'showFrames').name('Show frames / shape').onChange((v) => {
    params.deco.shape = v
    getObj()?.setLegends?.(undefined, v)
    saveUrl?.()
  })
  deco.add(state, 'showRowGuides').name('Row guides').onChange((v) => {
    params.deco['row guides'] = v ? 0.6 : 0
    getObj()?.setRowGuides?.(params.deco['row guides'])
    saveUrl?.()
  })
  deco.add(state, 'showFlowGuides').name('Multiplication guides').onChange((v) => {
    params.deco['flow guides'] = v ? 0.5 : 0
    getObj()?.setFlowGuide?.(params.deco['flow guides'])
    saveUrl?.()
  })
  deco.add(state, 'markerScale', 0.5, 2, 0.05).name('Marker scale').onChange((v) => {
    params.viz['elem scale'] = v
    initObj()
    saveUrl?.()
  })
  deco.add(state, 'gap', 1, 16, 1).name('Operand gap').onFinishChange((v) => {
    params.layout.gap = v
    initObj()
    saveUrl?.()
  })
  deco.add(state, 'cellSize', 1, 4, 1).name('Grid cell size').onFinishChange((v) => {
    params.layout.cellSize = v
    initObj()
    saveUrl?.()
  })

  gui.onFinishChange(saveUrl)
  return gui
}
