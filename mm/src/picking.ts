"use strict"

//
// Picking: pixel → logical tensor coordinate.
//
// Both render paths already report hits with `index` in *element* order —
// PointCloud by construction, HeatmapBlock by converting the hit UV back to
// the cell (element, not texel: LOD-independent). See test/points.test.ts,
// which pins that contract. This module leans on it and adds:
//
//   * hit → (entity, i, j) resolution through the SceneTree,
//   * save/restore of the shared raycaster's knobs (main.ts uses `far = 0` as
//     the spotlight's off switch and sets Points.threshold per call — a picker
//     that left either changed would silently break the lens),
//   * level expansion: the cell under the cursor → the row / column / display
//     block / whole matrix it belongs to,
//   * box select: an NDC rectangle → per-matrix cell ranges, by intersecting
//     the corner rays with each matrix's own plane. Through-selection (X-ray)
//     semantics: occlusion does not shrink the box.
//

import * as THREE from 'three'
import { CellRange, Level, cellAt, cellRange, fullRange } from './address.js'
import { SceneTree, SceneEntity, matLayoutInfo } from './scenetree.js'

export interface PickHit {
  entity: SceneEntity
  i: number
  j: number
  /** World-space point of the hit. */
  point: any
  distance: number
}

/** THREE renders visibility per subtree; raycast does not. Do it ourselves. */
export function isWorldVisible(obj: any): boolean {
  for (let o = obj; o; o = o.parent) {
    if (o.visible === false) return false
  }
  return true
}

export class Picker {
  raycaster: any
  camera: any
  /** Element pick radius in cell units; elements sit 1 apart. */
  threshold = 0.6

  constructor(raycaster: any, camera: any) {
    this.raycaster = raycaster
    this.camera = camera
  }

  /**
   * The logical cell under an NDC pointer, or null. Nearest hit wins; among
   * hits at effectively the same depth the one closest to the ray wins, so a
   * face-on matrix picks the element under the cursor rather than the first
   * of several the threshold swept up.
   */
  pick(ndc: { x: number, y: number }, tree: SceneTree): PickHit | null {
    const rc = this.raycaster
    const saved = { threshold: rc.params.Points.threshold, far: rc.far, near: rc.near }
    rc.setFromCamera(ndc as any, this.camera)
    rc.params.Points.threshold = this.threshold
    rc.far = Infinity
    rc.near = 0
    const hits: any[] = []
    try {
      for (const e of tree.mats()) {
        if (!e.mat.points || !isWorldVisible(e.mat.points)) continue
        rc.intersectObject(e.mat.points, true, hits)
      }
    } finally {
      rc.params.Points.threshold = saved.threshold
      rc.far = saved.far
      rc.near = saved.near
    }
    if (!hits.length) return null
    hits.sort((a, b) => a.distance - b.distance)
    const near = hits.filter(h => h.distance <= hits[0].distance + 1.0)
    near.sort((a, b) => (a.distanceToRay || 0) - (b.distanceToRay || 0))
    const h = near[0]
    if (h.index === undefined) return null
    const entity = tree.entityForObject(h.object)
    if (!entity || !entity.mat) return null
    const W = entity.mat.W
    return {
      entity,
      i: Math.floor(h.index / W),
      j: h.index % W,
      point: h.point,
      distance: h.distance,
    }
  }
}

/**
 * Expand a picked cell to the current selection level's range: the cell, its
 * row, its column, its *display* block (the same block structure the layout
 * gaps draw), or the whole matrix.
 */
export function levelRange(mat: any, i: number, j: number, level: Level): CellRange {
  const H = mat.H, W = mat.W
  switch (level) {
    case 'scalar': return cellAt(i, j)
    case 'row': return cellRange(i, i + 1, 0, W)
    case 'col': return cellRange(0, H, j, j + 1)
    case 'block': {
      const { i: { size: si }, j: { size: sj } } = mat.getBlockInfo()
      const bi = Math.floor(i / si), bj = Math.floor(j / sj)
      return cellRange(bi * si, Math.min(H, (bi + 1) * si), bj * sj, Math.min(W, (bj + 1) * sj))
    }
    case 'matrix': return fullRange(H, W)
  }
}

//
// Box select.
//
// dispX(j) = j + floor(j / sj) * gap is strictly increasing, so the cells
// whose centres fall inside a display-space interval [x0, x1] are exactly
// cellCeil(x0) … cellFloor(x1). The two inverses handle the gaps: a bound
// landing in the gap after block b snaps outward to the neighbouring cell.
//

/** Smallest index with display coordinate ≥ x (may be n: nothing right of x). */
export function cellCeil(x: number, size: number, gap: number, n: number): number {
  if (x <= 0) return 0
  const stride = size + gap
  const b = Math.floor(x / stride)
  const local = x - b * stride
  const j = local <= size - 1 ? b * size + Math.ceil(local) : (b + 1) * size
  return Math.min(j, n)
}

/** Largest index with display coordinate ≤ x (may be -1: nothing left of x). */
export function cellFloor(x: number, size: number, gap: number, n: number): number {
  if (x < 0) return -1
  const stride = size + gap
  const b = Math.floor(x / stride)
  const local = x - b * stride
  const j = local >= size - 1 ? b * size + size - 1 : b * size + Math.floor(local)
  return Math.min(j, n - 1)
}

/**
 * The cell range of `mat` whose element centres project inside an NDC rect,
 * or null. Corner rays are intersected with the matrix's own plane in
 * inner_group space; the resulting quad's AABB is the selection window. For a
 * matrix rotated against the rect this over-approximates toward the AABB —
 * the price of exact index arithmetic on an axis-aligned lattice.
 */
export function rectRangeForMat(
  camera: any, mat: any,
  rect: { x0: number, y0: number, x1: number, y1: number },
): CellRange | null {
  if (!mat.inner_group) return null
  mat.inner_group.updateWorldMatrix(true, false)
  const inv = new THREE.Matrix4().copy(mat.inner_group.matrixWorld).invert()
  const rc = new THREE.Raycaster()
  const corners = [
    [rect.x0, rect.y0], [rect.x1, rect.y0], [rect.x0, rect.y1], [rect.x1, rect.y1],
  ]
  const pts: any[] = []
  for (const [nx, ny] of corners) {
    rc.setFromCamera(new THREE.Vector2(nx, ny) as any, camera)
    const ray = rc.ray.clone().applyMatrix4(inv)
    if (Math.abs(ray.direction.z) < 1e-9) return null   // plane edge-on
    const t = -ray.origin.z / ray.direction.z
    if (t < 0) return null                              // plane behind camera
    pts.push(ray.origin.clone().addScaledVector(ray.direction, t))
  }
  const x0 = Math.min(...pts.map(p => p.x)), x1 = Math.max(...pts.map(p => p.x))
  const y0 = Math.min(...pts.map(p => p.y)), y1 = Math.max(...pts.map(p => p.y))

  const info = matLayoutInfo(mat)
  const jLo = cellCeil(x0, info.j.size, info.gap, mat.W)
  const jHi = cellFloor(x1, info.j.size, info.gap, mat.W)
  const iLo = cellCeil(y0, info.i.size, info.gap, mat.H)
  const iHi = cellFloor(y1, info.i.size, info.gap, mat.H)
  if (jLo > jHi || iLo > iHi) return null
  return cellRange(iLo, iHi + 1, jLo, jHi + 1)
}

/** Box select across the scene: every visible mat clipped against the rect. */
export function rectSelect(
  camera: any, tree: SceneTree,
  rect: { x0: number, y0: number, x1: number, y1: number },
): { entity: SceneEntity, range: CellRange }[] {
  const out: { entity: SceneEntity, range: CellRange }[] = []
  for (const e of tree.mats()) {
    if (!e.mat.points || !isWorldVisible(e.mat.points)) continue
    const r = rectRangeForMat(camera, e.mat, rect)
    if (r) out.push({ entity: e, range: r })
  }
  return out
}
