//
// cameractl.ts — framing, presets, tweens.
//
// What this file pins: the fitting arithmetic (per-corner frustum fitting, so
// the binding dimension exactly fills the frame — hand-computed for a wide box
// and a unit cube, and checked as a containment property for a box seen down a
// diagonal), the up-axis swap that makes the orbit rotate about an object's
// own axis, smoothstep interpolation at its exact midpoint, preset geometry,
// and that duration 0 lands instantly (the mode every other suite relies on).
// Deliberately untested: interaction with a real OrbitControls (a fake with
// target/update/_quat is the whole surface the rig touches).
//
import { describe, it, expect, vi } from 'vitest'
import * as THREE from 'three'
import { CameraRig, PRESET_DIRS, rangeWorldBox, entityUpAxis, fitBox } from '../src/cameractl.js'
import * as viz from '../src/viz.js'

const rig = (duration = 0) => {
  const camera = new THREE.PerspectiveCamera(45, 1, 5, 10000)
  camera.position.set(0, 0, 10)
  // _quat / _quatInverse are OrbitControls' orbit frame: it derives them from
  // camera.up once, in its constructor, so the rig has to re-derive them.
  const orbit = {
    target: new THREE.Vector3(),
    update: vi.fn(),
    _quat: new THREE.Quaternion().setFromUnitVectors(camera.up, new THREE.Vector3(0, 1, 0)),
    _quatInverse: new THREE.Quaternion(),
  }
  const r = new CameraRig(camera, orbit)
  r.duration = duration
  return { r, camera, orbit }
}

const HALF = Math.tan(THREE.MathUtils.degToRad(22.5))   // tan of the 45° fov's half-angle

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
  it('fits a box at pad·max(halfW/tan(hfov/2), halfH/tan(vfov/2)) + halfDepth', () => {
    const { r, camera } = rig(0)
    // unit cube [−1,1]³ down +z: hw = hh = hd = 1, fov 45 / aspect 1 → both
    // half-angles 22.5°, so dist = 1.05·1/tan 22.5° + 1 ≈ 3.5349.
    const box = new THREE.Box3(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1))
    r.frameBox(box)
    const want = 1.05 / HALF + 1
    expect(camera.position.length()).toBeCloseTo(want, 4)
    // view direction preserved: camera was on +z looking at the origin
    expect(camera.position.z).toBeCloseTo(want, 4)
  })

  it('fills the viewport width for a box wider than it is tall', () => {
    const { r, camera } = rig(0)
    // 16 × 2 × 1 box: width binds, so the near face should span the frame
    // horizontally bar the 5% margin — the old bounding-sphere fit left it
    // floating at ~24 units back instead of ~20.8.
    const box = new THREE.Box3(new THREE.Vector3(-8, -1, -0.5), new THREE.Vector3(8, 1, 0.5))
    r.frameBox(box)
    expect(camera.position.z).toBeCloseTo(1.05 * 8 / HALF + 0.5, 4)
    // half-width visible at the depth of the box's near face, from where the
    // camera actually landed
    const visible = (camera.position.z - 0.5) * HALF
    expect(8 / visible).toBeCloseTo(1 / 1.05, 6)
    const radius = box.getBoundingSphere(new THREE.Sphere()).radius
    expect(camera.position.z).toBeLessThan(1.15 * radius / Math.sin(THREE.MathUtils.degToRad(22.5)))
  })

  it('opens the near plane so a tight fit cannot clip the thing it framed', () => {
    const { r, camera } = rig(0)
    expect(camera.near).toBe(5)             // main.ts ships near = 5
    const box = new THREE.Box3(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1))
    r.frameBox(box)
    // the box's near face sits at |pos| − 1; the near plane must be in front of it
    expect(camera.near).toBeLessThan(camera.position.length() - 1)
    expect(camera.near).toBeGreaterThan(0)
  })

  it('measures the box against the rig\'s up axis, not the world\'s', () => {
    const { r, camera } = rig(0)
    // a box wide in y, thin in x, viewed down +z. With world up it is tall and
    // height binds; rolled 90° it is wide on screen and width binds — same
    // number either way at aspect 1, so tilt the aspect to tell them apart.
    camera.aspect = 2
    camera.updateProjectionMatrix()
    const box = new THREE.Box3(new THREE.Vector3(-1, -8, -0.5), new THREE.Vector3(1, 8, 0.5))
    r.frameBox(box)
    const upright = camera.position.z
    r.setUpAxis(new THREE.Vector3(1, 0, 0))   // roll: the long side is now horizontal
    r.frameBox(box)
    expect(camera.position.length()).toBeLessThan(upright)
  })

  it('fits a box viewed down a diagonal without backing off for a corner that is not there', () => {
    // The staged model scene: wide, shallow in y, very deep, seen from (-1,1,1).
    // Bounding the maximum lateral extent and the maximum depth *independently*
    // assumes one corner holds both, which no corner of this box does — and the
    // model came out small in the middle of the frame. Per corner it does not.
    const camera = new THREE.PerspectiveCamera(45, 1.9, 5, 10000)
    const box = new THREE.Box3(
      new THREE.Vector3(-809, -557, -1560), new THREE.Vector3(809, 557, 1560))
    const up = new THREE.Vector3(0, 1, 0)
    const f = fitBox(box, camera, new THREE.Vector3(-1, 1, 1), up)

    const d = f.dir
    const right = new THREE.Vector3().crossVectors(up, d).normalize()
    const vup = new THREE.Vector3().crossVectors(d, right).normalize()
    const ty = Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2)
    const tx = Math.tan(Math.atan(ty * camera.aspect))
    const half = box.getSize(new THREE.Vector3()).multiplyScalar(0.5)
    const ext = (a: any) =>
      Math.abs(half.x * a.x) + Math.abs(half.y * a.y) + Math.abs(half.z * a.z)

    // strictly closer than the per-axis bound this replaced
    const per_axis = 1.05 * Math.max(ext(right) / tx, ext(vup) / ty) + ext(d)
    expect(f.dist).toBeLessThan(per_axis * 0.95)

    // …and still contains every corner, which is the property that matters
    let tightest = 0
    for (const sx of [-1, 1]) for (const sy of [-1, 1]) for (const sz of [-1, 1]) {
      const c = new THREE.Vector3(sx * half.x, sy * half.y, sz * half.z)
      const [x, y, z] = [c.dot(right), c.dot(vup), c.dot(d)]
      expect(Math.abs(x)).toBeLessThanOrEqual((f.dist - z) * tx * (1 + 1e-9))
      expect(Math.abs(y)).toBeLessThanOrEqual((f.dist - z) * ty * (1 + 1e-9))
      tightest = Math.max(tightest,
        Math.abs(x) / ((f.dist - z) * tx), Math.abs(y) / ((f.dist - z) * ty))
    }
    // one corner sits right on the 5% margin: fitted, not merely contained
    expect(tightest).toBeCloseTo(1 / 1.05, 3)
  })

  it('ignores an empty box', () => {
    const { r, camera } = rig(0)
    const before = camera.position.clone()
    r.frameBox(new THREE.Box3())
    expect(camera.position.equals(before)).toBe(true)
  })
})

describe('up axis — orbiting about the object instead of the world', () => {
  it('re-derives OrbitControls\' orbit frame, which it computes only in its constructor', () => {
    const { r, camera, orbit } = rig(0)
    expect(r.setUpAxis(new THREE.Vector3(1, 0, 0))).toBe(true)
    expect(camera.up.toArray().map(v => +v.toFixed(6))).toEqual([1, 0, 0])
    // _quat is the map from the orbit axis to +Y: it must now send +X there
    const probe = new THREE.Vector3(1, 0, 0).applyQuaternion(orbit._quat)
    expect(probe.y).toBeCloseTo(1, 6)
    // and _quatInverse must undo it
    expect(probe.applyQuaternion(orbit._quatInverse).x).toBeCloseTo(1, 6)
  })

  it('flips an axis that points away from the current up, so selecting never turns the view over', () => {
    const { r, camera } = rig(0)
    expect(r.setUpAxis(new THREE.Vector3(0, -1, 0))).toBe(false)
    expect(camera.up.y).toBeCloseTo(1, 6)
    r.setUpAxis(new THREE.Vector3(-1, -1, 0))
    expect(camera.up.y).toBeCloseTo(Math.SQRT1_2, 6)   // negated into the +Y hemisphere
    expect(camera.up.x).toBeCloseTo(Math.SQRT1_2, 6)
  })

  it('no-ops when the axis is already in force — selection changes fire on every box-drag step', () => {
    const { r, orbit } = rig(0)
    expect(r.setUpAxis(new THREE.Vector3(0, 1, 0))).toBe(false)
    expect(r.setUpAxis(null)).toBe(false)
    expect(r.setUpAxis(new THREE.Vector3(0, 0, 0))).toBe(false)
    expect(orbit.update).not.toHaveBeenCalled()
    expect(r.isMoving()).toBe(false)
  })

  it('resetUpAxis returns to world +Y, which is what an empty selection means', () => {
    const { r, camera } = rig(0)
    r.setUpAxis(new THREE.Vector3(1, 0, 0))
    expect(r.resetUpAxis()).toBe(true)
    expect(camera.up.toArray().map(v => +v.toFixed(6))).toEqual([0, 1, 0])
  })

  it('rolls smoothly rather than snapping, and the framing basis leads the roll', () => {
    const { r, camera } = rig(320)
    r.setUpAxis(new THREE.Vector3(1, 0, 0), 1000)
    // the destination is in force for framing before the roll has played out
    expect(r.upTarget().x).toBeCloseTo(1, 6)
    r.update(1000 + 160)
    expect(camera.up.length()).toBeCloseTo(1, 6)
    expect(camera.up.x).toBeCloseTo(Math.SQRT1_2, 6)   // halfway: 45° of roll
    r.update(1000 + 320)
    expect(camera.up.x).toBeCloseTo(1, 6)
    expect(camera.up.y).toBeCloseTo(0, 6)
    expect(r.isMoving()).toBe(false)
  })
})

const matAt = (rot: (o: any) => void) => {
  const inner = new THREE.Object3D()
  rot(inner)
  inner.updateMatrixWorld(true)
  return { mat: { inner_group: inner } } as any
}

describe('entityUpAxis', () => {
  it('is the entity\'s own +Y for an upright mat, tilted with it', () => {
    expect(entityUpAxis(matAt(() => { })).toArray().map(v => +v.toFixed(6)))
      .toEqual([0, 1, 0])
    const axis = entityUpAxis(matAt(o => { o.rotation.z = Math.PI / 6 }))
    expect(axis.x).toBeCloseTo(-0.5, 6)
    expect(axis.y).toBeCloseTo(Math.sqrt(3) / 2, 6)
  })

  it('picks a different basis axis rather than a pole: a quarter-turned mat\'s +Y points at the viewer', () => {
    // this is the top-placed right operand — local +Y lands on world −Z, which
    // is exactly the default view direction
    const e = matAt(o => { o.rotation.x = -Math.PI / 2 })
    expect(new THREE.Vector3().setFromMatrixColumn(e.mat.inner_group.matrixWorld, 1)
      .toArray().map(v => +v.toFixed(6))).toEqual([0, 0, -1])
    const axis = entityUpAxis(e)
    expect(axis.toArray().map(v => +v.toFixed(6))).toEqual([0, 1, 0])   // its local +Z
  })

  it('measures against the up in force, not the world\'s', () => {
    // rolled 60°: local +X is 60° off world +X, local +Y is 30° off world +X
    const e = matAt(o => { o.rotation.z = Math.PI / 3 })
    expect(entityUpAxis(e, new THREE.Vector3(1, 0, 0)).x).toBeCloseTo(Math.sqrt(3) / 2, 6)
    expect(entityUpAxis(e, new THREE.Vector3(0, 1, 0)).y).toBeCloseTo(Math.sqrt(3) / 2, 6)
    // and they are different axes, 90° apart
    expect(entityUpAxis(e, new THREE.Vector3(1, 0, 0))
      .dot(entityUpAxis(e, new THREE.Vector3(0, 1, 0)))).toBeCloseTo(0, 6)
  })

  it('flips the chosen axis into the reference hemisphere, so selecting never turns the view over', () => {
    const axis = entityUpAxis(matAt(o => { o.rotation.z = Math.PI }))   // +Y is now −Y
    expect(axis.y).toBeCloseTo(1, 6)
  })

  it('falls back to the node group for a non-mat entity, and skips degenerate columns', () => {
    const group = new THREE.Object3D()
    group.rotation.x = Math.PI / 2
    group.updateMatrixWorld(true)
    const axis = entityUpAxis({ node: { group } } as any)   // its own −Z, flipped up
    expect(axis.x).toBeCloseTo(0, 6)
    expect(axis.y).toBeCloseTo(1, 6)
    expect(axis.z).toBeCloseTo(0, 6)
    const flat = new THREE.Object3D()
    flat.scale.set(0, 0, 0)
    flat.updateMatrixWorld(true)
    expect(entityUpAxis({ mat: { inner_group: flat } } as any)).toBe(null)
    expect(entityUpAxis({} as any)).toBe(null)
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
