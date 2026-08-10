"use strict"

//
// Camera control for the tensor editor: framing, presets, pivot, smooth
// transitions. Wraps the existing PerspectiveCamera + OrbitControls pair —
// it never replaces them, so every existing behaviour (URL camera state,
// label updates on orbit end, the magnifier's cloned camera) is untouched.
//
// Projection toggle (perspective ↔ orthographic) is deliberately NOT here:
// main.ts closes over one PerspectiveCamera in a dozen places (raycaster,
// lens pass, resize, context), and a half-swapped camera would render one
// projection while picking through another. That is a main.ts refactor, not
// a feature of this module; better absent than wrong.
//

import * as THREE from 'three'
import { CellRange, fullRange } from './address.js'
import { SceneTree, SceneEntity } from './scenetree.js'
import { SelectionManager } from './selection.js'
import { rangeRect } from './highlight.js'

/** World up. The rig starts here and returns here when nothing is selected. */
const WORLD_UP = new THREE.Vector3(0, 1, 0)

export const PRESET_DIRS: Record<string, [number, number, number]> = {
  front: [0, 0, 1],
  back: [0, 0, -1],
  right: [1, 0, 0],
  left: [-1, 0, 0],
  top: [0, 1, 0],
  bottom: [0, -1, 0],
}

/** World-space box of a cell range on a mat: the display rect, ±0.5 deep. */
export function rangeWorldBox(mat: any, r: CellRange): any {
  const rect = rangeRect(mat, r)
  mat.inner_group.updateWorldMatrix(true, false)
  const box = new THREE.Box3()
  const v = new THREE.Vector3()
  for (const dx of [-0.5, 0.5]) {
    for (const dy of [-0.5, 0.5]) {
      for (const dz of [-0.5, 0.5]) {
        v.set(rect.cx + dx * rect.w, rect.cy + dy * rect.h, dz)
        box.expandByPoint(v.applyMatrix4(mat.inner_group.matrixWorld))
      }
    }
  }
  return box
}

/** World-space box of a whole entity. */
export function entityWorldBox(e: SceneEntity): any {
  if (e.mat) return rangeWorldBox(e.mat, fullRange(e.mat.H, e.mat.W))
  e.node.group.updateWorldMatrix(true, false)
  return e.node.getBoundingBox()
}

/**
 * An axis of the entity's *own* frame to orbit about: whichever of its three
 * world-space basis axes points most nearly along `ref` (the up currently in
 * force), flipped into `ref`'s hemisphere.
 *
 * Not simply local +Y. A top-placed right operand is rotated a quarter turn
 * about x, so its +Y runs straight down the default view direction — making
 * that the orbit pole would leave OrbitControls at its gimbal clamp and hand
 * `camera.lookAt` a degenerate basis on the most ordinary click in the app.
 * Picking the best of three orthogonal axes bounds the error at 54.74°, so the
 * chosen axis is never closer than ~35° to the view direction, while an
 * upright mat still yields exactly its local +Y.
 */
export function entityUpAxis(e: SceneEntity, ref: any = WORLD_UP): any | null {
  const obj = e.mat ? e.mat.inner_group : e.node?.group
  if (!obj) return null
  obj.updateWorldMatrix(true, false)
  const want = new THREE.Vector3().copy(ref)
  if (want.lengthSq() < 1e-12) want.copy(WORLD_UP)
  want.normalize()
  let best: any = null, best_dot = -Infinity
  for (const col of [0, 1, 2]) {
    const axis = new THREE.Vector3().setFromMatrixColumn(obj.matrixWorld, col)
    if (axis.lengthSq() < 1e-12) continue      // a flattened scale: not an axis
    axis.normalize()
    const dot = axis.dot(want)
    const aligned = Math.abs(dot)
    if (aligned > best_dot) {
      best_dot = aligned
      best = dot < 0 ? axis.negate() : axis
    }
  }
  return best
}

export class CameraRig {
  camera: any
  orbit: any
  /** Transition duration, ms. 0 disables tweening (tests set 0). */
  duration = 320

  private tween: {
    t0: number, dur: number,
    from_pos: any, from_target: any, to_pos: any, to_target: any,
    from_up: any, to_up: any,
  } | null = null

  constructor(camera: any, orbit: any) {
    this.camera = camera
    this.orbit = orbit
  }

  /**
   * Advance the transition; called from the animate loop with a timestamp.
   * Returns true while a transition is running.
   */
  update(now: number): boolean {
    if (!this.tween) return false
    const tw = this.tween
    const t = Math.min(1, (now - tw.t0) / tw.dur)
    const s = t * t * (3 - 2 * t)     // smoothstep: no snap at either end
    this.camera.position.lerpVectors(tw.from_pos, tw.to_pos, s)
    this.orbit.target.lerpVectors(tw.from_target, tw.to_target, s)
    this.applyUp(new THREE.Vector3().lerpVectors(tw.from_up, tw.to_up, s), tw.to_up)
    this.orbit.update()
    if (t >= 1) this.tween = null
    return true
  }

  isMoving(): boolean {
    return this.tween !== null
  }

  /**
   * The up axis the rig is heading for: the tween's destination while one is
   * in flight, otherwise the camera's current up. Framing measures the box
   * against *this* basis, so an F pressed mid-roll fits what will be on screen
   * when the roll finishes, not what is on screen now.
   */
  upTarget(): any {
    return this.tween ? this.tween.to_up.clone() : this.camera.up.clone()
  }

  /**
   * Point the orbit's pole along `axis`, so dragging rotates the camera about
   * that axis instead of world +Y. OrbitControls derives its rotation frame
   * from `camera.up` **once, in its constructor** (`_quat` / `_quatInverse`),
   * so setting `camera.up` alone would silently do nothing — the quaternions
   * have to be re-derived here. Pan reads `object.up` live, so panning becomes
   * object-aligned too.
   *
   * Returns false when the axis is degenerate or already in force (selection
   * changes fire constantly during a box drag; restarting the tween on each
   * would fight the user).
   */
  setUpAxis(axis: any, now = performance.now()): boolean {
    if (!axis) return false
    const up = new THREE.Vector3().copy(axis)
    if (up.lengthSq() < 1e-12) return false
    up.normalize()
    const cur = this.upTarget()
    // A mat's Y column can point down relative to the view (stacks tilt their
    // groups); taking it raw would flip the world upside down on selection.
    if (up.dot(cur) < 0) up.negate()
    if (up.dot(cur) > 0.9999) return false
    this.flyTo(this.camera.position.clone(), this.orbit.target.clone(), now, up)
    return true
  }

  /** Back to orbiting about world +Y — what an empty selection means. */
  resetUpAxis(now = performance.now()): boolean {
    return this.setUpAxis(WORLD_UP, now)
  }

  /** Write an up vector through to the camera and OrbitControls' orbit frame. */
  private applyUp(up: any, fallback: any) {
    if (up.lengthSq() < 1e-8) up.copy(fallback)
    up.normalize()
    this.camera.up.copy(up)
    const o = this.orbit as any
    if (o?._quat?.setFromUnitVectors && o?._quatInverse) {
      o._quat.setFromUnitVectors(up, WORLD_UP)
      o._quatInverse.copy(o._quat).invert()
    }
  }

  flyTo(pos: any, target: any, now = performance.now(), up: any = null) {
    const to_up = up ? new THREE.Vector3().copy(up).normalize() : this.upTarget()
    if (this.duration <= 0) {
      this.camera.position.copy(pos)
      this.orbit.target.copy(target)
      this.applyUp(to_up, to_up)
      this.orbit.update()
      this.tween = null
      return
    }
    this.tween = {
      t0: now, dur: this.duration,
      from_pos: this.camera.position.clone(),
      from_target: this.orbit.target.clone(),
      to_pos: new THREE.Vector3().copy(pos),
      to_target: new THREE.Vector3().copy(target),
      from_up: this.camera.up.clone(),
      to_up,
    }
  }

  /**
   * Fit a world box to the viewport along the current view direction.
   *
   * Measures the box's actual extent on the screen axes (right / up / view,
   * taken from the up axis the rig is heading for) rather than its bounding
   * sphere, then backs off until whichever of width or height binds first
   * exactly fills the frame:
   *
   *     dist = pad · max(halfW / tan(hfov/2), halfH / tan(vfov/2)) + halfDepth
   *
   * `pad` multiplies the fitting term only — the half-depth is added after, so
   * a deep box clears the frame instead of eating the margin. A wide matrix,
   * which the sphere formula used to leave floating in the middle of the
   * screen, now spans the full width.
   */
  frameBox(box: any, pad = 1.05) {
    if (!box || box.isEmpty()) return
    const centre = box.getCenter(new THREE.Vector3())
    const dir = this.camera.position.clone().sub(this.orbit.target)
    if (dir.lengthSq() < 1e-6) dir.set(-1, 1, 1)
    dir.normalize()

    // Screen basis. Near the pole the up axis is parallel to the view, so fall
    // back to any perpendicular rather than dividing by a zero-length cross.
    const up = this.upTarget()
    const right = new THREE.Vector3().crossVectors(up, dir)
    if (right.lengthSq() < 1e-8) right.crossVectors(new THREE.Vector3(0, 0, 1), dir)
    if (right.lengthSq() < 1e-8) right.set(1, 0, 0)
    right.normalize()
    const vup = new THREE.Vector3().crossVectors(dir, right).normalize()

    // Half-extent of an axis-aligned box projected on a unit axis.
    const half = box.getSize(new THREE.Vector3()).multiplyScalar(0.5)
    const extent = (a: any) =>
      Math.abs(half.x * a.x) + Math.abs(half.y * a.y) + Math.abs(half.z * a.z)
    const hw = extent(right), hh = extent(vup), hd = extent(dir)

    const vfov = THREE.MathUtils.degToRad(this.camera.fov)
    const hfov = 2 * Math.atan(Math.tan(vfov / 2) * this.camera.aspect)
    const fit = Math.max(hw / Math.tan(hfov / 2), hh / Math.tan(vfov / 2), 1e-3)
    const dist = pad * fit + hd

    // A tight fit can pull the camera inside its own near plane (main.ts ships
    // near = 5), which would hide the very thing being framed. Clip planes only
    // ever open outwards, as with far below.
    const near = Math.max(0.05, (dist - hd) * 0.5)
    if (this.camera.near > near) {
      this.camera.near = near
      this.camera.updateProjectionMatrix()
    }
    if (this.camera.far < dist + 2 * hd + 2 * fit) {
      this.camera.far = dist + 2 * hd + 2 * fit
      this.camera.updateProjectionMatrix()
    }
    this.flyTo(centre.clone().addScaledVector(dir, dist), centre)
  }

  /** Frame the selection: union of all selected ranges/entities. */
  frameSelection(tree: SceneTree | null, selection: SelectionManager): boolean {
    if (!tree || selection.isEmpty()) return false
    const box = new THREE.Box3()
    for (const path of selection.paths()) {
      const e = tree.get(path)
      if (!e) continue
      const ranges = selection.rangesOf(path)
      if (e.mat && ranges) {
        for (const r of ranges) box.union(rangeWorldBox(e.mat, r))
      } else {
        const b = entityWorldBox(e)
        if (b && !b.isEmpty()) box.union(b)
      }
    }
    if (box.isEmpty()) return false
    this.frameBox(box)
    return true
  }

  /** Frame everything under the scene root node. */
  frameAll(rootNode: any) {
    if (!rootNode?.group) return
    rootNode.group.updateMatrixWorld(true)
    const box = new THREE.Box3().setFromObject(rootNode.group)
    this.frameBox(box)
  }

  /** Axis-aligned view of the current target, keeping the current distance. */
  preset(name: keyof typeof PRESET_DIRS) {
    const dir = PRESET_DIRS[name]
    if (!dir) return
    const target = this.orbit.target.clone()
    const dist = Math.max(this.camera.position.distanceTo(target), 1)
    const axis = new THREE.Vector3(...dir)
    const pos = target.clone().addScaledVector(axis, dist)
    // A view straight down the up axis is gimbal-degenerate for OrbitControls;
    // nudge it off the pole so 'up' stays defined. With world up that is
    // top/bottom, but once the pole follows an object axis it can be any
    // preset, so test the geometry rather than the name.
    if (Math.abs(axis.dot(this.upTarget())) > 1 - 1e-6) {
      const perp = new THREE.Vector3().crossVectors(axis, WORLD_UP)
      if (perp.lengthSq() < 1e-8) perp.set(0, 0, 1)   // top/bottom: nudge in z
      pos.addScaledVector(perp.normalize(), dist * 1e-3)
    }
    this.flyTo(pos, target)
  }

  /** Move only the orbit pivot (world origin, selection centre, or cursor). */
  setPivot(point: any) {
    this.flyTo(this.camera.position.clone(), new THREE.Vector3().copy(point))
  }
}
