"use strict"

//
// Highlight rendering: what the selection looks like.
//
// One overlay group at scene level, never parented into the viz node groups —
// initViz() and Stack.setStage() dispose those wholesale, and an overlay that
// lived inside them would be torn down (and its geometry disposed out from
// under us) on every rebuild. Instead each overlay carries a baked matrix:
// the mat's inner_group.matrixWorld composed with a local translate/scale, so
// it sits exactly on the elements it covers. refresh() rebuilds everything
// from SelectionManager state; the renderer owns no state of its own.
//
// Overlays draw with depthTest off and a high renderOrder: selection reads
// through occluding matrices (through-selection / X-ray semantics — in a
// scene of layered planes an occluded selection that vanished would look like
// a dropped selection).
//
// Colours follow Blender's vocabulary: orange for selected, brighter for the
// active entity, cyan for the hover preview, magenta for the tensor cursor.
//

import * as THREE from 'three'
import { CellRange, fullRange } from './address.js'
import { SceneTree, SceneEntity, matLayoutInfo } from './scenetree.js'
import { elementPosition } from '../render/heatmap.js'
import { SelectionManager } from './selection.js'

export const COLOR_SELECTED = 0xff8c1a
export const COLOR_ACTIVE = 0xffc266
export const COLOR_HOVER = 0x55ccff
export const COLOR_CURSOR = 0xff3377

// Shared unit geometries; every overlay is a matrix-scaled instance of one of
// these three, so refresh() allocates no geometry at all.
const unitPlane = new THREE.PlaneGeometry(1, 1)

const unitRect = (() => {
  const g = new THREE.BufferGeometry()
  const p = 0.5
  g.setAttribute('position', new THREE.Float32BufferAttribute([
    -p, -p, 0, p, -p, 0,
    p, -p, 0, p, p, 0,
    p, p, 0, -p, p, 0,
    -p, p, 0, -p, -p, 0,
  ], 3))
  return g
})()

const unitBoxEdges = (() => {
  const g = new THREE.BufferGeometry()
  const p = 0.5
  const c = [[-p, -p, -p], [p, -p, -p], [p, p, -p], [-p, p, -p],
  [-p, -p, p], [p, -p, p], [p, p, p], [-p, p, p]]
  const e = [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]]
  const pos: number[] = []
  e.forEach(([a, b]) => pos.push(...c[a], ...c[b]))
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3))
  return g
})()

const lineMat = (color: number) => new THREE.LineBasicMaterial({
  color, transparent: true, opacity: 0.95, depthTest: false, depthWrite: false,
})

const fillMat = (color: number, opacity: number) => new THREE.MeshBasicMaterial({
  color, transparent: true, opacity, depthTest: false, depthWrite: false,
  side: THREE.DoubleSide,
})

const MAT_SEL_LINE = lineMat(COLOR_SELECTED)
const MAT_SEL_FILL = fillMat(COLOR_SELECTED, 0.18)
const MAT_ACT_LINE = lineMat(COLOR_ACTIVE)
const MAT_ACT_FILL = fillMat(COLOR_ACTIVE, 0.22)
const MAT_HOVER_LINE = lineMat(COLOR_HOVER)
const MAT_CURSOR_LINE = lineMat(COLOR_CURSOR)

const RENDER_ORDER = 1000

/**
 * The display-space rectangle covering a cell range on a mat, in inner_group
 * coordinates: element centres sit at elementPosition, cells are 1 wide, so
 * the rect runs half a cell beyond the first and last centres. A range that
 * crosses a block gap spans it — the marquee names the logical range, and the
 * gap is layout, not data.
 */
export function rangeRect(mat: any, r: CellRange) {
  const info = matLayoutInfo(mat)
  const a = elementPosition(r.i[0], r.j[0], info)
  const b = elementPosition(r.i[1] - 1, r.j[1] - 1, info)
  return {
    cx: (a.x + b.x) / 2, cy: (a.y + b.y) / 2,
    w: b.x - a.x + 1, h: b.y - a.y + 1,
  }
}

/** Bake world = inner_group.matrixWorld ∘ translate(cx,cy,0) ∘ scale(w,h,1). */
function bakeRectMatrix(obj: any, mat: any, rect: { cx: number, cy: number, w: number, h: number }) {
  mat.inner_group.updateWorldMatrix(true, false)
  obj.matrixAutoUpdate = false
  obj.matrix.copy(mat.inner_group.matrixWorld)
    .multiply(new THREE.Matrix4().makeTranslation(rect.cx, rect.cy, 0))
    .multiply(new THREE.Matrix4().makeScale(rect.w, rect.h, 1))
}

export class HighlightRenderer {
  /** Add to the scene once; everything else hangs under it. */
  group = new THREE.Group()

  private sel_group = new THREE.Group()
  private hover_group = new THREE.Group()
  private cursor_group = new THREE.Group()

  constructor() {
    this.group.name = 'editor.highlights'
    this.group.add(this.sel_group, this.hover_group, this.cursor_group)
  }

  private clearGroup(g: any) {
    // Shared geometries/materials: children are plain wrappers, nothing owns
    // GPU resources of its own, so removal is enough.
    while (g.children.length) g.remove(g.children[0])
  }

  private addRect(g: any, mat: any, r: CellRange, line: any, fill: any | null) {
    const rect = rangeRect(mat, r)
    if (fill) {
      const m = new THREE.Mesh(unitPlane, fill)
      m.renderOrder = RENDER_ORDER
      bakeRectMatrix(m, mat, rect)
      g.add(m)
    }
    const o = new THREE.LineSegments(unitRect, line)
    o.renderOrder = RENDER_ORDER + 1
    bakeRectMatrix(o, mat, rect)
    g.add(o)
  }

  private addEntityBox(g: any, e: SceneEntity, line: any) {
    const node = e.node
    if (!node.group || !node.getBoundingBox) return
    node.group.updateWorldMatrix(true, false)
    const box = node.getBoundingBox()
    if (!box || box.isEmpty()) return
    const c = box.getCenter(new THREE.Vector3())
    const s = box.getSize(new THREE.Vector3())
    const o = new THREE.LineSegments(unitBoxEdges, line)
    o.renderOrder = RENDER_ORDER + 1
    o.matrixAutoUpdate = false
    o.matrix.makeTranslation(c.x, c.y, c.z)
      .multiply(new THREE.Matrix4().makeScale(Math.max(s.x, 0.5), Math.max(s.y, 0.5), Math.max(s.z, 0.5)))
    g.add(o)
  }

  /** Rebuild all selection overlays from state. Cheap: shared geometry, one
   *  object per range. */
  refresh(tree: SceneTree | null, selection: SelectionManager) {
    this.clearGroup(this.sel_group)
    if (!tree) return
    const active = selection.activePath()
    for (const path of selection.paths()) {
      const e = tree.get(path)
      if (!e) continue
      const is_active = path === active
      const line = is_active ? MAT_ACT_LINE : MAT_SEL_LINE
      const fill = is_active ? MAT_ACT_FILL : MAT_SEL_FILL
      const ranges = selection.rangesOf(path)
      if (e.mat && e.mat.points) {
        const rs = ranges === null ? [fullRange(e.mat.H, e.mat.W)] : ranges || []
        for (const r of rs) this.addRect(this.sel_group, e.mat, r, line, fill)
      } else if (!e.mat) {
        this.addEntityBox(this.sel_group, e, line)
      }
    }
  }

  /** The hover preview: what a click at the current level would select. */
  setHover(hover: { mat: any, range: CellRange } | null) {
    this.clearGroup(this.hover_group)
    if (hover && hover.mat.points) {
      this.addRect(this.hover_group, hover.mat, hover.range, MAT_HOVER_LINE, null)
    }
  }

  /** The tensor cursor: a marked cell that outlives the selection. */
  setCursor(cursor: { mat: any, i: number, j: number } | null) {
    this.clearGroup(this.cursor_group)
    if (!cursor || !cursor.mat.points) return
    const r: CellRange = { i: [cursor.i, cursor.i + 1], j: [cursor.j, cursor.j + 1] }
    this.addRect(this.cursor_group, cursor.mat, r, MAT_CURSOR_LINE, null)
    // Crosshair arms, one cell long each side, so the cursor stays findable
    // when the marked cell is sub-pixel.
    const rect = rangeRect(cursor.mat, r)
    for (const [w, h] of [[3, 0.08], [0.08, 3]]) {
      const o = new THREE.LineSegments(unitRect, MAT_CURSOR_LINE)
      o.renderOrder = RENDER_ORDER + 2
      bakeRectMatrix(o, cursor.mat, { cx: rect.cx, cy: rect.cy, w, h })
      this.cursor_group.add(o)
    }
  }

  dispose() {
    this.clearGroup(this.sel_group)
    this.clearGroup(this.hover_group)
    this.clearGroup(this.cursor_group)
  }
}
