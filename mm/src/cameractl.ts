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

export class CameraRig {
  camera: any
  orbit: any
  /** Transition duration, ms. 0 disables tweening (tests set 0). */
  duration = 320

  private tween: {
    t0: number, dur: number,
    from_pos: any, from_target: any, to_pos: any, to_target: any,
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
    this.orbit.update()
    if (t >= 1) this.tween = null
    return true
  }

  isMoving(): boolean {
    return this.tween !== null
  }

  flyTo(pos: any, target: any, now = performance.now()) {
    if (this.duration <= 0) {
      this.camera.position.copy(pos)
      this.orbit.target.copy(target)
      this.orbit.update()
      return
    }
    this.tween = {
      t0: now, dur: this.duration,
      from_pos: this.camera.position.clone(),
      from_target: this.orbit.target.clone(),
      to_pos: new THREE.Vector3().copy(pos),
      to_target: new THREE.Vector3().copy(target),
    }
  }

  /**
   * Fit a world box in view along the current view direction. Same fitting
   * arithmetic as main.ts's frameStagedScene: the bounding sphere against the
   * narrower half-angle, with a little air.
   */
  frameBox(box: any, pad = 1.15) {
    if (!box || box.isEmpty()) return
    const centre = box.getCenter(new THREE.Vector3())
    const radius = Math.max(box.getBoundingSphere(new THREE.Sphere()).radius, 1)
    const vfov = THREE.MathUtils.degToRad(this.camera.fov)
    const hfov = 2 * Math.atan(Math.tan(vfov / 2) * this.camera.aspect)
    const dist = pad * radius / Math.sin(Math.min(vfov, hfov) / 2)
    const dir = this.camera.position.clone().sub(this.orbit.target)
    if (dir.lengthSq() < 1e-6) dir.set(-1, 1, 1)
    dir.normalize()
    if (this.camera.far < dist + 2 * radius) {
      this.camera.far = dist + 2 * radius
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
    const pos = target.clone().addScaledVector(new THREE.Vector3(...dir), dist)
    // A pure top/bottom view is gimbal-degenerate for OrbitControls' up
    // vector; nudge it off the pole so 'up' stays defined.
    if (name === 'top' || name === 'bottom') pos.z += dist * 1e-3
    this.flyTo(pos, target)
  }

  /** Move only the orbit pivot (world origin, selection centre, or cursor). */
  setPivot(point: any) {
    this.flyTo(this.camera.position.clone(), new THREE.Vector3().copy(point))
  }
}
