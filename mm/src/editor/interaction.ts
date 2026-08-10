"use strict"

//
// The interaction controller: one factory that owns the tensor editor's
// managers and binds them to the viewer's input surfaces.
//
//   NAVIGATE → HOVER → SELECT → HIGHLIGHT → INSPECT → TRANSFORM
//
// main.ts calls createEditor() once, then attach(obj) after every scene
// rebuild and update(t) once per frame. Everything else — hover picking,
// click/box selection, the keymap, the tensor cursor, panel synchronization,
// edit application — happens in here, against the managers, never against
// THREE object identity.
//
// Keymap constraints inherited from main.ts (do not fight them):
//   * Ctrl is the magnifier chord and is intercepted before key_funcs — no
//     Ctrl bindings here, and metaKey is left to the browser.
//   * Space / ArrowUp / ArrowDown / KeyP / KeyS belong to the animation.
//   * ArrowLeft / ArrowRight belong to OrbitControls.
//   * A long (≥500ms) stationary press is the lens, so a click is a press
//     that ends quickly and near where it began.
//
// Bindings:  1-5 selection level (matrix/block/row/col/scalar) · Tab cycle ·
// click select · shift+click toggle · alt+click whole matrix · double-click
// frame · B box select · Esc cancel box select, else fit the whole model ·
// F fit selection to the viewport · Home frame all ·
// A all · alt+A none · I invert · U / shift+U selection undo/redo · H hide ·
// shift+H isolate · alt+H unhide · C cursor at hover · shift+C clear cursor ·
// . pivot to cursor/selection · X zero selection · shift+1/3/7 view presets.
//

import * as THREE from 'three'
import { LEVELS, Level, fmtValue } from './address.js'
import { SceneTree, cellLocal } from './scenetree.js'
import { SelectionManager, VisibilityState } from './selection.js'
import { Picker, PickHit, levelRange, rectSelect } from './picking.js'
import { HighlightRenderer } from './highlight.js'
import { CameraRig, entityWorldBox, entityUpAxis } from './cameractl.js'
import { EditStack, refreshTouched, OpKind } from './editops.js'
import { createInspector, HoverInfo } from './inspector.js'
import { createOutliner } from './outliner.js'

export interface EditorContext {
  scene: any
  camera: any
  orbit: any
  renderer: any
  raycaster: any
  getObj: () => any
  /** Where panels/overlay DOM land; document.body by default. */
  domParent?: any
  /** Test hook: skip DOM/window listeners entirely. */
  headless?: boolean
}

export interface Editor {
  selection: SelectionManager
  visibility: VisibilityState
  edits: EditStack
  rig: CameraRig
  highlight: HighlightRenderer
  getTree: () => SceneTree | null
  getLevel: () => Level
  setLevel: (l: Level) => void
  attach: (obj: any) => void
  refreshTree: (obj: any) => void
  layoutChanged: () => void
  update: (now: number) => void
  applyOpToSelection: (kind: OpKind, params: { value?: number, min?: number, max?: number }) => void
  focusEntity: (path: string) => void
  handleKey: (e: KeyboardEvent) => boolean
  dispose: () => void
}

const STYLE_ID = 'qme-interaction-style'

const CSS = `
.qme-tooltip {
  position: fixed; display: none; z-index: 30; pointer-events: none;
  background: rgba(0,0,0,0.78); border: 1px solid #444; border-radius: 3px;
  padding: 2px 7px; font: 11px -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  color: #e8ecff; white-space: pre;
}
.qme-band {
  position: fixed; display: none; z-index: 25; pointer-events: none;
  border: 1px dashed #55ccff; background: rgba(85,204,255,0.08);
}
.qme-status {
  position: fixed; bottom: 12px; left: 50%; transform: translateX(-50%);
  z-index: 10; background: rgba(0,0,0,0.55); border-radius: 4px;
  padding: 3px 10px; font: 11px -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  color: #8d93ad; user-select: none; white-space: pre;
}
.qme-status b { color: #cfd3e6; font-weight: 600; }
`

function injectStyle(parent: any) {
  if (document.getElementById(STYLE_ID)) return
  const s = document.createElement('style')
  s.id = STYLE_ID
  s.textContent = CSS
  document.head.appendChild(s)
}

const isEditable = (t: any) =>
  t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)

export function createEditor(ctx: EditorContext): Editor {
  const selection = new SelectionManager()
  const visibility = new VisibilityState()
  let tree: SceneTree | null = null
  const getTree = () => tree
  const edits = new EditStack(getTree)
  const picker = new Picker(ctx.raycaster, ctx.camera)
  const highlight = new HighlightRenderer()
  const rig = new CameraRig(ctx.camera, ctx.orbit)
  ctx.scene.add(highlight.group)

  let level: Level = 'matrix'
  let hover: PickHit | null = null
  let cursor: { path: string, i: number, j: number } | null = null

  // box select: armed by B, active while dragging
  let box_armed = false
  let box_active = false
  let box_start = { x: 0, y: 0 }

  // click discrimination
  let down_at = { x: 0, y: 0, t: 0 }
  let buttons_down = 0
  let orbit_dragging = false

  const parent = ctx.domParent || (typeof document !== 'undefined' ? document.body : null)
  let tooltip: any = null, band: any = null, status: any = null
  const unsubs: (() => void)[] = []
  const dom_listeners: [any, string, any][] = []

  //
  // shared actions
  //

  const cursorInfo = () => {
    if (!cursor || !tree) return null
    const e = tree.get(cursor.path)
    if (!e?.mat) return null
    return { path: cursor.path, i: cursor.i, j: cursor.j, value: e.mat.getData(cursor.i, cursor.j) }
  }

  const applyOpToSelection = (kind: OpKind, params: { value?: number, min?: number, max?: number }) => {
    if (!tree) return
    for (const path of selection.paths()) {
      const e = tree.get(path)
      if (!e?.mat) continue
      edits.addOp(path, selection.rangesOf(path) ?? null, kind, params)
    }
  }

  const focusEntity = (path: string) => {
    const e = tree?.get(path)
    if (!e) return
    const box = entityWorldBox(e)
    if (box && !box.isEmpty()) rig.frameBox(box)
  }

  /**
   * Orbit about the selected object's own axis rather than world +Y — with
   * nothing selected, back to world up. The rig no-ops when the axis is
   * already in force, so this is safe to call on every selection change
   * (including each step of a box drag).
   */
  const syncOrbitAxis = () => {
    const path = selection.activePath()
    const e = path ? tree?.get(path) : null
    // With nothing selected the subject is the whole model, so the pole follows
    // the *root's* own axis rather than snapping back to the world's. Only the
    // axis — not the pivot: an empty-canvas click clears the selection, and a
    // camera jump on every one of those is a move nobody asked for. Esc is the
    // gesture that reframes the model.
    const axis = e ? entityUpAxis(e, rig.upTarget())
      : tree ? entityUpAxis(tree.root, rig.upTarget()) : null
    if (axis) rig.setUpAxis(axis)
    else rig.resetUpAxis()
  }

  const refreshHighlights = () => {
    highlight.refresh(tree, selection)
    const ci = cursorInfo()
    highlight.setCursor(ci && tree ? { mat: tree.get(ci.path)!.mat, i: ci.i, j: ci.j } : null)
  }

  const setHover = (h: PickHit | null, ev: { clientX: number, clientY: number } | null) => {
    hover = h
    if (!h) {
      highlight.setHover(null)
      if (tooltip) tooltip.style.display = 'none'
      inspector?.setHover(null)
      return
    }
    const range = levelRange(h.entity.mat, h.i, h.j, level)
    highlight.setHover({ mat: h.entity.mat, range })
    const value = h.entity.mat.getData(h.i, h.j)
    if (tooltip && ev) {
      tooltip.textContent = level === 'matrix' ?
        `${h.entity.name}  [${h.entity.mat.H} × ${h.entity.mat.W}]` :
        `${h.entity.name}[${h.i}, ${h.j}] = ${fmtValue(value)}`
      tooltip.style.display = 'block'
      tooltip.style.left = `${ev.clientX + 14}px`
      tooltip.style.top = `${ev.clientY + 14}px`
    }
    inspector?.setHover({ path: h.entity.path, name: h.entity.name, i: h.i, j: h.j, value })
  }

  const updateStatus = () => {
    if (!status) return
    const n = selection.paths().length
    const cells = n ? selection.countCells() : 0
    const mode = box_armed || box_active ? 'BOX SELECT (drag, Esc cancels)' : `level: ${level}`
    status.innerHTML = ''
    const b = document.createElement('b')
    b.textContent = mode
    status.appendChild(b)
    status.appendChild(document.createTextNode(
      `  ·  ${n} selected · ${cells.toLocaleString()} cells  ·  ` +
      `[1-5] level  B box  F fit  Esc model  H hide  C cursor  X zero  U undo-sel`))
  }

  //
  // panels
  //

  const inspector = ctx.headless ? null : createInspector({
    selection, edits, visibility, getTree,
    getCursor: cursorInfo,
    getLevel: () => level,
    focusEntity,
    applyOpToSelection,
  })
  const outliner = ctx.headless ? null : createOutliner({
    selection, visibility, getTree, focusEntity,
  })

  //
  // manager subscriptions
  //

  unsubs.push(selection.onChange(type => {
    refreshHighlights()
    updateStatus()
    syncOrbitAxis()
    if (type === 'select' && selection.activePath()) {
      outliner?.revealPath(selection.activePath()!)
    }
  }))
  unsubs.push(visibility.onChange(() => {
    if (tree) visibility.apply(tree)
  }))
  unsubs.push(edits.onChange(() => {
    refreshTouched(tree, edits.lastTouched)
  }))

  //
  // lifecycle
  //

  const rebuildIndex = () => {
    selection.setTree(tree!)
    visibility.apply(tree!)
    if (cursor && !tree!.get(cursor.path)?.mat) cursor = null
    setHover(null, null)
    refreshHighlights()
    outliner?.refresh()
    inspector?.refresh()
    updateStatus()
    // A rebuild can move the selected mat (stage change, layout knob), so its
    // own axis — the one the orbit is pivoting about — may have moved with it.
    syncOrbitAxis()
  }

  /** A brand-new scene object (initObj): data arrays are fresh, so edit
   *  baselines are stale — drop them and reapply the stack as descriptions. */
  const attach = (obj: any) => {
    tree = new SceneTree(obj, obj?.params?.name || 'model')
    refreshTouched(tree, edits.onTreeRebuilt())
    rebuildIndex()
  }

  /** Same object, rebuilt visuals (Stack.setStage): the data arrays — and any
   *  edits already applied to them — survive, so baselines must NOT reset;
   *  only the object index and overlay transforms went stale. */
  const refreshTree = (obj: any) => {
    tree = new SceneTree(obj, obj?.params?.name || 'model')
    rebuildIndex()
  }

  /** Matrices moved (stage change, layout knob): re-bake overlay transforms. */
  const layoutChanged = () => {
    setHover(null, null)
    refreshHighlights()
  }

  //
  // hover picking, throttled to one raycast per frame
  //

  let hover_pending: { ndc: { x: number, y: number }, ev: any } | null = null
  let hover_scheduled = false

  const processHover = () => {
    hover_scheduled = false
    if (!hover_pending || !tree) return
    const { ndc, ev } = hover_pending
    hover_pending = null
    if (buttons_down > 0 || orbit_dragging || box_active) return
    setHover(picker.pick(ndc, tree), ev)
  }

  const ndcOf = (e: { clientX: number, clientY: number }) => ({
    x: e.clientX / window.innerWidth * 2 - 1,
    y: -(e.clientY / window.innerHeight * 2 - 1),
  })

  //
  // pointer handling
  //

  const onPointerMove = (e: PointerEvent) => {
    if (box_active && band) {
      const x0 = Math.min(box_start.x, e.clientX), x1 = Math.max(box_start.x, e.clientX)
      const y0 = Math.min(box_start.y, e.clientY), y1 = Math.max(box_start.y, e.clientY)
      band.style.left = `${x0}px`
      band.style.top = `${y0}px`
      band.style.width = `${x1 - x0}px`
      band.style.height = `${y1 - y0}px`
      return
    }
    hover_pending = { ndc: ndcOf(e), ev: e }
    if (!hover_scheduled) {
      hover_scheduled = true
      requestAnimationFrame(processHover)
    }
  }

  const onPointerDown = (e: PointerEvent) => {
    buttons_down++
    down_at = { x: e.clientX, y: e.clientY, t: performance.now() }
    setHover(null, null)
    if (box_armed && e.button === 0 && e.target === ctx.renderer.domElement) {
      box_armed = false
      box_active = true
      box_start = { x: e.clientX, y: e.clientY }
      ctx.orbit.enabled = false
      if (band) {
        band.style.display = 'block'
        band.style.left = `${e.clientX}px`
        band.style.top = `${e.clientY}px`
        band.style.width = '0px'
        band.style.height = '0px'
      }
      updateStatus()
    }
  }

  const finishBox = (e: PointerEvent) => {
    box_active = false
    ctx.orbit.enabled = true
    if (band) band.style.display = 'none'
    if (!tree) return
    const a = ndcOf({ clientX: box_start.x, clientY: box_start.y })
    const b = ndcOf(e)
    const rect = {
      x0: Math.min(a.x, b.x), x1: Math.max(a.x, b.x),
      y0: Math.min(a.y, b.y), y1: Math.max(a.y, b.y),
    }
    const found = rectSelect(ctx.camera, tree, rect)
    // Box select adds; shift-box removes nothing (Blender's B adds too).
    for (const { entity, range } of found) {
      if (level === 'matrix') selection.selectEntity(entity.path, 'add')
      else selection.selectRange(entity.path, range, 'add')
    }
    updateStatus()
  }

  // A drag that ends outside the window delivers pointercancel (or nothing);
  // a stuck non-zero button counter would disable hover for the session.
  const onPointerCancel = () => {
    buttons_down = 0
    if (box_active) {
      box_active = false
      ctx.orbit.enabled = true
      if (band) band.style.display = 'none'
      updateStatus()
    }
  }

  const onPointerUp = (e: PointerEvent) => {
    buttons_down = Math.max(0, buttons_down - 1)
    if (box_active) {
      finishBox(e)
      return
    }
    // A click: primary button, on the canvas, short and stationary — anything
    // longer is the lens, anything travelled is an orbit.
    if (e.button !== 0 || e.target !== ctx.renderer.domElement) return
    const dt = performance.now() - down_at.t
    const dist = Math.hypot(e.clientX - down_at.x, e.clientY - down_at.y)
    if (dt > 350 || dist > 5 || !tree) return
    const hit = picker.pick(ndcOf(e), tree)
    if (!hit) {
      if (!e.shiftKey) selection.clear()
      return
    }
    if (e.detail === 2) {
      focusEntity(hit.entity.path)
      return
    }
    const mode = e.shiftKey ? 'toggle' : 'set'
    if (e.altKey || level === 'matrix') {
      selection.selectEntity(hit.entity.path, mode)
    } else {
      selection.selectRange(hit.entity.path, levelRange(hit.entity.mat, hit.i, hit.j, level), mode)
    }
  }

  //
  // keymap
  //

  const setLevel = (l: Level) => {
    level = l
    if (hover && tree) {
      // re-preview at the new granularity without waiting for a move
      highlight.setHover({ mat: hover.entity.mat, range: levelRange(hover.entity.mat, hover.i, hover.j, l) })
    }
    updateStatus()
    inspector?.refresh()
  }

  const selectionWorldCenter = () => {
    if (!tree || selection.isEmpty()) return null
    const box = new THREE.Box3()
    for (const path of selection.paths()) {
      const e = tree.get(path)
      if (e) {
        const b = entityWorldBox(e)
        if (b && !b.isEmpty()) box.union(b)
      }
    }
    return box.isEmpty() ? null : box.getCenter(new THREE.Vector3())
  }

  /** Returns true when the key was consumed. Exposed for tests. */
  const handleKey = (e: KeyboardEvent): boolean => {
    if (e.ctrlKey || e.metaKey || isEditable(e.target)) return false

    if (e.shiftKey) {
      switch (e.code) {
        case 'Digit1': rig.preset('front'); return true
        case 'Digit3': rig.preset('right'); return true
        case 'Digit7': rig.preset('top'); return true
        case 'KeyH':
          if (tree && !selection.isEmpty()) visibility.isolate(new Set(selection.paths()), tree)
          return true
        case 'KeyU': selection.redoSelection(); return true
        case 'KeyC':
          cursor = null
          refreshHighlights()
          inspector?.refresh()
          return true
      }
      return false
    }

    if (e.altKey) {
      switch (e.code) {
        case 'KeyA': selection.clear(); return true
        case 'KeyH': visibility.showAll(); return true
      }
      return false
    }

    const digit = { Digit1: 0, Digit2: 1, Digit3: 2, Digit4: 3, Digit5: 4 }[e.code]
    if (digit !== undefined) {
      setLevel(LEVELS[digit])
      return true
    }

    switch (e.code) {
      case 'Tab':
        e.preventDefault()
        setLevel(LEVELS[(LEVELS.indexOf(level) + 1) % LEVELS.length])
        return true
      case 'KeyF':
        if (!rig.frameSelection(tree, selection) && hover) focusEntity(hover.entity.path)
        return true
      case 'Home':
        rig.frameAll(ctx.getObj())
        return true
      case 'KeyA': selection.selectAll(); return true
      case 'KeyI': selection.invertActive(); return true
      case 'KeyU': selection.undoSelection(); return true
      case 'KeyB':
        box_armed = !box_armed
        updateStatus()
        return true
      case 'Escape':
        if (box_armed || box_active) {
          box_armed = false
          box_active = false
          ctx.orbit.enabled = true
          if (band) band.style.display = 'none'
          updateStatus()
          return true
        }
        // Escape out to the whole model: drop the selection (which returns the
        // orbit pole to the model's own axis through syncOrbitAxis) and fit the
        // model's box to the viewport. One key back to the overview, whatever
        // the camera was doing.
        if (!selection.isEmpty()) selection.clear()
        rig.frameAll(ctx.getObj())
        return true
      case 'KeyH':
        if (!selection.isEmpty()) {
          visibility.hide(selection.paths())
          selection.clear()
        }
        return true
      case 'KeyC':
        if (hover) {
          cursor = { path: hover.entity.path, i: hover.i, j: hover.j }
          refreshHighlights()
          inspector?.refresh()
        }
        return true
      case 'Period': {
        const ci = cursorInfo()
        if (ci && tree) {
          const e = tree.get(ci.path)!
          const local = cellLocal(e.mat, ci.i, ci.j)
          e.mat.inner_group.updateWorldMatrix(true, false)
          const p = new THREE.Vector3(local.x, local.y, local.z)
            .applyMatrix4(e.mat.inner_group.matrixWorld)
          rig.setPivot(p)
        } else {
          const c = selectionWorldCenter()
          if (c) rig.setPivot(c)
        }
        return true
      }
      case 'KeyX':
        if (!selection.isEmpty()) applyOpToSelection('zero', {})
        return true
    }
    return false
  }

  const onKeyDown = (e: KeyboardEvent) => {
    handleKey(e)
  }

  //
  // DOM setup
  //

  if (!ctx.headless && parent) {
    injectStyle(parent)
    tooltip = document.createElement('div')
    tooltip.className = 'qme-tooltip'
    band = document.createElement('div')
    band.className = 'qme-band'
    status = document.createElement('div')
    status.className = 'qme-status'
    parent.appendChild(tooltip)
    parent.appendChild(band)
    parent.appendChild(status)
    if (inspector) parent.appendChild(inspector.root)
    if (outliner) parent.appendChild(outliner.root)

    const on = (target: any, type: string, fn: any) => {
      target.addEventListener(type, fn)
      dom_listeners.push([target, type, fn])
    }
    on(window, 'pointermove', onPointerMove)
    on(window, 'pointerdown', onPointerDown)
    on(window, 'pointerup', onPointerUp)
    on(window, 'pointercancel', onPointerCancel)
    on(window, 'blur', onPointerCancel)
    on(window, 'keydown', onKeyDown)
    ctx.orbit.addEventListener?.('start', () => { orbit_dragging = true; setHover(null, null) })
    ctx.orbit.addEventListener?.('end', () => { orbit_dragging = false })
    updateStatus()
  }

  const dispose = () => {
    unsubs.forEach(u => u())
    dom_listeners.forEach(([t, ty, f]) => t.removeEventListener(ty, f))
    tooltip?.remove()
    band?.remove()
    status?.remove()
    inspector?.dispose()
    outliner?.dispose()
    ctx.scene.remove(highlight.group)
    highlight.dispose()
  }

  return {
    selection, visibility, edits, rig, highlight,
    getTree, getLevel: () => level, setLevel,
    attach, refreshTree, layoutChanged, update: (now: number) => { rig.update(now) },
    applyOpToSelection, focusEntity, handleKey, dispose,
  }
}
