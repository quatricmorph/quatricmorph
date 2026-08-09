//
// interaction.ts — the editor controller, exercised headless.
//
// What this file covers: the keymap contract (including the keys it must
// REFUSE: Ctrl chords belong to the magnifier, keystrokes inside form fields
// belong to the panels), selection-level cycling, and the end-to-end pipeline
// a keystroke can trigger: X zeroes the selection through the edit stack and
// the matmul result recomputes from its operands.
//
// What is deliberately untested: pointer handlers, tooltip/status DOM and the
// panels (headless: true skips all of them — they get their own suites), and
// anything that renders. The camera rig runs with duration 0 so framing lands
// instantly and assertions are exact.
//
import { describe, it, expect, vi, beforeEach } from 'vitest'
import * as THREE from 'three'

import * as viz from '../src/viz.js'
import { defaultParams } from '../src/params.js'
import { createEditor } from '../src/interaction.js'

// A 2×2 @ 2×2 matmul with hand-knowable data: left is the identity ('eye'),
// right is 'row major' over [0, 1] → [[0, 1/3], [2/3, 1]], so result === right.
function smallParams() {
  const p = defaultParams()
  p.name = 'out'
  p.expr = ''
  p.epilog = 'none'
  p.anim.alg = 'none'
  p.deco.legends = 0
  p.deco.shape = false
  p.block = { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 }
  p.left = {
    name: 'L', matmul: false, h: 2, w: 2, init: 'eye',
    url: '', min: 0, max: 1, dropout: 0, expr: '',
  }
  p.right = {
    name: 'R', matmul: false, h: 2, w: 2, init: 'row major',
    url: '', min: 0, max: 1, dropout: 0, expr: '',
  }
  return p
}

function makeEditor() {
  const camera = new THREE.PerspectiveCamera(45, 1, 5, 10000)
  camera.position.set(0, 0, 100)
  const ctx = {
    raycaster: new THREE.Raycaster(),
    camera,
    pointer: new THREE.Vector2(),
  }
  const mm = new viz.MatMul(smallParams(), ctx, true)
  const scene = new THREE.Scene()
  scene.add(mm.group)
  mm.group.updateMatrixWorld(true)
  const orbit = {
    target: new THREE.Vector3(),
    update: vi.fn(),
    enabled: true,
    addEventListener: vi.fn(),
  }
  const editor = createEditor({
    scene, camera, orbit,
    renderer: { domElement: document.createElement('canvas') },
    raycaster: ctx.raycaster,
    getObj: () => mm,
    headless: true,
  })
  editor.rig.duration = 0
  editor.attach(mm)
  return { editor, mm, camera, orbit, ctx }
}

const key = (code: string, mods: any = {}) => ({
  code, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false,
  target: null, preventDefault: () => { }, ...mods,
}) as any

describe('keymap ownership', () => {
  it('refuses Ctrl chords — Ctrl is the magnifier, main.ts intercepts it first', () => {
    const { editor } = makeEditor()
    expect(editor.handleKey(key('KeyA', { ctrlKey: true }))).toBe(false)
    expect(editor.selection.isEmpty()).toBe(true)
  })

  it('refuses keystrokes whose target is a form field, so typing in a panel never selects', () => {
    const { editor } = makeEditor()
    const input = document.createElement('input')
    expect(editor.handleKey(key('KeyA', { target: input }))).toBe(false)
    expect(editor.selection.isEmpty()).toBe(true)
  })
})

describe('selection level', () => {
  it('starts at matrix — the editor opens in object-mode terms, like Blender', () => {
    const { editor } = makeEditor()
    expect(editor.getLevel()).toBe('matrix')
  })

  it('Digit1..Digit5 map to matrix/block/row/col/scalar in coarse-to-fine order', () => {
    const { editor } = makeEditor()
    const want = ['matrix', 'block', 'row', 'col', 'scalar']
    want.forEach((lvl, k) => {
      expect(editor.handleKey(key(`Digit${k + 1}`))).toBe(true)
      expect(editor.getLevel()).toBe(lvl)
    })
  })

  it('Tab cycles one level finer and wraps from scalar back to matrix', () => {
    const { editor } = makeEditor()
    editor.setLevel('scalar')
    expect(editor.handleKey(key('Tab'))).toBe(true)
    expect(editor.getLevel()).toBe('matrix')
  })
})

describe('select all / none / invert / undo', () => {
  it('A selects every mat in the tree — 3 for a single matmul (left, right, result)', () => {
    const { editor } = makeEditor()
    editor.handleKey(key('KeyA'))
    expect(editor.selection.paths().sort()).toEqual(['out/left', 'out/result', 'out/right'])
    // whole 2×2 mats: 3 × 4 cells
    expect(editor.selection.countCells()).toBe(12)
  })

  it('Alt+A clears, and U (selection undo) brings the selection back', () => {
    const { editor } = makeEditor()
    editor.handleKey(key('KeyA'))
    editor.handleKey(key('KeyA', { altKey: true }))
    expect(editor.selection.isEmpty()).toBe(true)
    expect(editor.handleKey(key('KeyU'))).toBe(true)
    expect(editor.selection.paths()).toHaveLength(3)
    // and Shift+U re-applies the clear
    editor.handleKey(key('KeyU', { shiftKey: true }))
    expect(editor.selection.isEmpty()).toBe(true)
  })

  it('I inverts within the active entity: a selected first row becomes the second', () => {
    const { editor } = makeEditor()
    editor.selection.selectRange('out/right', { i: [0, 1], j: [0, 2] }, 'set')
    editor.handleKey(key('KeyI'))
    expect(editor.selection.rangesOf('out/right')).toEqual([{ i: [1, 2], j: [0, 2] }])
  })
})

describe('visibility keys', () => {
  it('H hides the selected entities and empties the selection; Alt+H unhides', () => {
    const { editor, mm } = makeEditor()
    editor.selection.selectEntity('out/left', 'set')
    editor.handleKey(key('KeyH'))
    expect(editor.visibility.isHidden('out/left')).toBe(true)
    expect(mm.left.group.visible).toBe(false)
    expect(editor.selection.isEmpty()).toBe(true)
    editor.handleKey(key('KeyH', { altKey: true }))
    expect(mm.left.group.visible).toBe(true)
  })

  it('Shift+H isolates: everything but the selection (and its ancestors) hides', () => {
    const { editor, mm } = makeEditor()
    editor.selection.selectEntity('out/result', 'set')
    editor.handleKey(key('KeyH', { shiftKey: true }))
    expect(mm.result.group.visible).toBe(true)
    expect(mm.left.group.visible).toBe(false)
    expect(mm.right.group.visible).toBe(false)
  })
})

describe('box-select arming', () => {
  it('B arms, Esc disarms and reports the key consumed; Esc with nothing armed is not consumed', () => {
    const { editor } = makeEditor()
    expect(editor.handleKey(key('Escape'))).toBe(false)
    expect(editor.handleKey(key('KeyB'))).toBe(true)
    expect(editor.handleKey(key('Escape'))).toBe(true)
  })
})

describe('camera keys', () => {
  it('F frames the selection: the camera lands looking at the selected mat', () => {
    const { editor, camera, orbit } = makeEditor()
    editor.selection.selectEntity('out/result', 'set')
    const before = camera.position.clone()
    expect(editor.handleKey(key('KeyF'))).toBe(true)
    expect(camera.position.equals(before)).toBe(false)
    // the new orbit target is the selection's own centre, not the origin
    expect(orbit.target.length()).toBeGreaterThan(0)
  })

  it('Shift+1 flies to the front preset, preserving distance to the target', () => {
    const { editor, camera, orbit } = makeEditor()
    const dist = camera.position.distanceTo(orbit.target)
    editor.handleKey(key('Digit1', { shiftKey: true }))
    expect(camera.position.distanceTo(orbit.target)).toBeCloseTo(dist, 5)
    // front = +z of the target
    expect(camera.position.z).toBeGreaterThan(orbit.target.z)
    expect(Math.abs(camera.position.x - orbit.target.x)).toBeLessThan(1e-9)
  })
})

describe('X — zero the selection through the edit stack', () => {
  it('zeroes the left operand and the matmul result recomputes to zero from its operands', () => {
    const { editor, mm } = makeEditor()
    editor.selection.selectEntity('out/left', 'set')
    editor.handleKey(key('KeyX'))
    // left is now all zeros…
    for (let k = 0; k < 4; k++) expect(mm.left.getDataArray()[k]).toBe(0)
    // …and the product 0 @ R is zero everywhere: the stale-product lie is the
    // thing editops exists to prevent.
    for (let k = 0; k < 4; k++) expect(mm.result.getDataArray()[k]).toBe(0)
    expect(editor.edits.ops).toHaveLength(1)
  })

  it('a ranged add on the identity-fed product edits exactly the selected row of the result chain', () => {
    const { editor, mm } = makeEditor()
    // right = [[0, 1/3], [2/3, 1]] ('row major' over 4 elements); left = I,
    // so result === right before the edit.
    editor.selection.selectRange('out/right', { i: [0, 1], j: [0, 2] }, 'set')
    editor.applyOpToSelection('add', { value: 5 })
    const r = mm.right.data
    expect(r.get(0, 0)).toBeCloseTo(5, 6)
    expect(r.get(0, 1)).toBeCloseTo(5 + 1 / 3, 6)
    expect(r.get(1, 0)).toBeCloseTo(2 / 3, 6)   // untouched row
    // result recomputed: I @ R === R, including the edit
    expect(mm.result.getData(0, 1)).toBeCloseTo(5 + 1 / 3, 6)
    expect(mm.result.getData(1, 1)).toBeCloseTo(1, 6)
  })
})

describe('attach across rebuilds', () => {
  it('selection survives a scene rebuild by path, never by object identity', () => {
    const { editor, ctx } = makeEditor()
    editor.selection.selectEntity('out/left', 'set')
    const mm2 = new viz.MatMul(smallParams(), ctx, true)
    editor.attach(mm2)
    expect(editor.selection.has('out/left')).toBe(true)
    expect(editor.getTree()!.get('out/left')!.mat).toBe(mm2.left)
  })

  it('edit ops reapply onto a rebuilt scene: the description outlives the data', () => {
    const { editor, ctx } = makeEditor()
    editor.selection.selectEntity('out/left', 'set')
    editor.applyOpToSelection('scale', { value: 3 })
    const mm2 = new viz.MatMul(smallParams(), ctx, true)
    editor.attach(mm2)
    // 'eye' scaled by 3: diagonal 3, off-diagonal 0 — and the product follows.
    expect(mm2.left.getData(0, 0)).toBeCloseTo(3, 6)
    expect(mm2.result.getData(0, 1)).toBeCloseTo(1, 6)   // 3·I @ R row scaling: 3·R[0][1] = 1
  })
})
