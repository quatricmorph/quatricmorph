//
// cameractl.ts — framing, presets, tweens.
//
// What this file pins: the fitting arithmetic (the same bounding-sphere/
// half-angle formula frameStagedScene uses, hand-computed for a unit case),
// smoothstep interpolation at its exact midpoint, preset geometry, and that
// duration 0 lands instantly (the mode every other suite relies on).
// Deliberately untested: interaction with a real OrbitControls (a fake with
// target/update is the whole surface the rig touches).
//
import { describe, it, expect, vi } from 'vitest'
import * as THREE from 'three'
import { CameraRig, PRESET_DIRS, rangeWorldBox } from '../src/cameractl.js'
import * as viz from '../src/viz.js'

const rig = (duration = 0) => {
  const camera = new THREE.PerspectiveCamera(45, 1, 5, 10000)
  camera.position.set(0, 0, 10)
  const orbit = { target: new THREE.Vector3(), update: vi.fn() }
  const r = new CameraRig(camera, orbit)
  r.duration = duration
  return { r, camera, orbit }
}

describe('flyTo', () => {
  it('lands instantly at duration 0 and tells the controls', () => {
    const { r, camera, orbit } = rig(0)
    r.flyTo(new THREE.Vector3(1, 2, 3), new THREE.Vector3(4, 5, 6))
    expect(camera.position.toArray()).toEqual([1, 2, 3])
    expect(orbit.target.toArray()).toEqual([4, 5, 6])
    expect(orbit.update).toHaveBeenCalled()
    expect(r.isMoving()).toBe(false)
  })

  it('is exactly half-blended at the tween midpoint — smoothstep(0.5) = 0.5', () => {
    const { r, camera, orbit } = rig(320)
    r.flyTo(new THREE.Vector3(10, 0, 10), new THREE.Vector3(2, 0, 0), 1000)
    expect(r.update(1000 + 160)).toBe(true)
    // from (0,0,10) to (10,0,10): x = 5 at the midpoint; target x = 1
    expect(camera.position.x).toBeCloseTo(5, 6)
    expect(orbit.target.x).toBeCloseTo(1, 6)
    r.update(1000 + 320)
    expect(camera.position.x).toBeCloseTo(10, 6)
    expect(r.isMoving()).toBe(false)
    expect(r.update(2000)).toBe(false)
  })
})

describe('frameBox', () => {
  it('fits a box at pad·radius/sin(half-angle) along the current view direction', () => {
    const { r, camera } = rig(0)
    // unit cube [−1,1]³: bounding sphere radius √3. fov 45, aspect 1 →
    // half-angle 22.5°, so dist = 1.15·√3 / sin 22.5° ≈ 5.2050.
    const box = new THREE.Box3(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1))
    r.frameBox(box)
    const want = 1.15 * Math.sqrt(3) / Math.sin(THREE.MathUtils.degToRad(22.5))
    expect(camera.position.length()).toBeCloseTo(want, 4)
    // view direction preserved: camera was on +z looking at the origin
    expect(camera.position.z).toBeCloseTo(want, 4)
  })

  it('ignores an empty box', () => {
    const { r, camera } = rig(0)
    const before = camera.position.clone()
    r.frameBox(new THREE.Box3())
    expect(camera.position.equals(before)).toBe(true)
  })
})

describe('presets', () => {
  it('front keeps the distance and lands on the target\'s +z axis', () => {
    const { r, camera, orbit } = rig(0)
    orbit.target.set(3, 0, 0)
    camera.position.set(3, 4, 0)          // distance 4 from target
    r.preset('front')
    expect(camera.position.distanceTo(orbit.target)).toBeCloseTo(4, 6)
    expect(camera.position.x).toBeCloseTo(3, 6)
    expect(camera.position.z).toBeCloseTo(4, 6)
  })

  it('top nudges off the pole so OrbitControls\' up vector stays defined', () => {
    const { r, camera, orbit } = rig(0)
    r.preset('top')
    expect(camera.position.y).toBeGreaterThan(0)
    expect(camera.position.z).not.toBe(orbit.target.z)
    expect(Object.keys(PRESET_DIRS).sort()).toEqual(
      ['back', 'bottom', 'front', 'left', 'right', 'top'])
  })
})

describe('rangeWorldBox', () => {
  it('measures a whole 4×4 single-block mat as a 4×4×1 world box', () => {
    const lf = (name, h, w) => ({
      name, matmul: false, h, w, init: 'row major',
      url: '', expr: '', min: 0, max: 1, dropout: 0,
    })
    const p = {
      epilog: 'none',
      anim: { alg: 'none', speed: 16, fuse: 'none', 'hide inputs': false, spin: 0 },
      block: { 'i blocks': 1, 'k blocks': 1, 'j blocks': 1 },
      layout: {
        scheme: 'blocks', gap: 2, scatter: 0, molecule: 1, blast: 0,
        polarity: 'negative', 'left placement': 'left',
        'right placement': 'top', 'result placement': 'front',
      },
      deco: { legends: 0, shape: false, spotlight: 0, 'row guides': 0, 'flow guides': 0, grid: 0 },
      viz: {
        sensitivity: 'local', 'min size': 0.05, 'min light': 0.2, 'max light': 0.9,
        'elem scale': 2, 'zero hue': 0.75, 'hue gap': 0.75, 'hue spread': 0.03,
        'render mode': 'spheres', 'heatmap encoding': 'magnitude',
        'heatmap filter': 'nearest', 'lod reduce': 'maxAbs', 'texel budget': 0,
      },
      name: 'out', left: lf('L', 4, 4), right: lf('R', 4, 4),
    }
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000)
    const mm = new viz.MatMul(p, { raycaster: new THREE.Raycaster(), camera, pointer: new THREE.Vector2() }, true)
    mm.group.updateMatrixWorld(true)
    const box = rangeWorldBox(mm.result, { i: [0, 4], j: [0, 4] })
    const size = box.getSize(new THREE.Vector3())
    // centres 0..3 plus half a cell each side = 4; depth ±0.5 = 1
    expect(size.x).toBeCloseTo(4, 6)
    expect(size.y).toBeCloseTo(4, 6)
    expect(size.z).toBeCloseTo(1, 6)
  })
})
