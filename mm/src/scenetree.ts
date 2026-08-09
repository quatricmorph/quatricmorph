"use strict"

//
// SceneTree: the logical index over a built viz node tree.
//
// Selection and rendering are deliberately decoupled: a selection names an
// entity by *path* — the chain of roles from the root ('out/left/right/result')
// — never by THREE object identity, because initViz() disposes and rebuilds
// every object under it and Stack.setStage() does so per stage. The paths are a
// function of the params tree's shape, so they survive every rebuild; this
// index is rebuilt after each one and selections re-resolve by path.
//
// The walk covers every node kind viz.ts can build (see "Node kinds beyond the
// matmul" there): MatMul, Mat, UnaryOp, AddOp, Stack. Animation intermediates
// (`anim_mats`) are transient and deliberately not indexed — a pick lands on
// scene structure, not on a sweep's moving plane.
//
// No THREE import: the tree holds references to viz objects and their groups,
// but builds nothing of its own. Geometry helpers live in picking/highlight.
//

import { elementPosition } from './heatmap.js'

export type EntityKind = 'stack' | 'matmul' | 'unary' | 'add' | 'mat'

export interface SceneEntity {
  /** Stable id: role segments joined with '/'. Never parsed, only compared. */
  path: string
  /** Display name: the node's params.name where it has one, else its role. */
  name: string
  kind: EntityKind
  role: string
  depth: number
  parent: SceneEntity | null
  children: SceneEntity[]
  /** The viz node (MatMul | Mat | UnaryOp | AddOp | Stack). */
  node: any
  /** For kind 'mat': the Mat itself (node === mat). */
  mat: any
  /** For stack stages: the stage record (key, name, kind, note, row). */
  stage: any
}

/** Duck-typed kind of a built viz node. One place, so a new node kind fails here loudly. */
export function nodeKind(node: any): EntityKind {
  if (node.stages) return 'stack'
  if (typeof node.dotprod === 'function') return 'matmul'
  if (node.input && node.result) return 'unary'
  if (node.left && node.right && node.result) return 'add'
  // A Mat always carries its Array2D from construction; `points` only exists
  // after initViz, so it must not be part of the discriminator.
  if (node.data) return 'mat'
  throw new Error(`scenetree: unrecognized node kind (keys: ${Object.keys(node).slice(0, 8)})`)
}

const displayName = (node: any, role: string) =>
  (node.params && node.params.name) ? String(node.params.name) : role

export class SceneTree {
  root: SceneEntity
  entities: SceneEntity[] = []
  byPath: Map<string, SceneEntity> = new Map()
  /** THREE object → mat entity, for resolving raycast hits. */
  byObject: Map<any, SceneEntity> = new Map()

  constructor(rootNode: any, rootName = undefined) {
    this.root = this.walk(rootNode, rootName || displayName(rootNode, 'model'), null, null)
  }

  private register(e: SceneEntity): SceneEntity {
    this.entities.push(e)
    this.byPath.set(e.path, e)
    if (e.kind === 'mat' && e.mat.points) {
      // Both render paths hang off `mat.points`: a PointCloud is one pickable
      // object, a HeatmapMesh is a Group whose HeatmapBlock children are what
      // the raycaster actually reports. Register whichever exist.
      this.byObject.set(e.mat.points, e)
      const blocks = e.mat.points.blocks
      if (blocks) for (const b of blocks) this.byObject.set(b, e)
    }
    return e
  }

  private walk(node: any, role: string, parent: SceneEntity | null, stage: any): SceneEntity {
    const kind = nodeKind(node)
    const path = parent ? `${parent.path}/${role}` : role
    const e: SceneEntity = {
      path, role, kind, stage,
      name: stage ? String(stage.name || role) : displayName(node, role),
      depth: parent ? parent.depth + 1 : 0,
      parent, children: [],
      node, mat: kind === 'mat' ? node : null,
    }
    this.register(e)
    const child = (n: any, r: string, st: any = null) =>
      e.children.push(this.walk(n, r, e, st))
    switch (kind) {
      case 'stack':
        node.stages.forEach((st: any) => child(st.obj, String(st.key), st))
        break
      case 'matmul':
      case 'add':
        child(node.left, 'left')
        child(node.right, 'right')
        child(node.result, 'result')
        break
      case 'unary':
        child(node.input, 'input')
        child(node.result, 'result')
        break
      case 'mat':
        break
    }
    return e
  }

  mats(): SceneEntity[] {
    return this.entities.filter(e => e.kind === 'mat')
  }

  entityForObject(obj: any): SceneEntity | null {
    return this.byObject.get(obj) || null
  }

  get(path: string): SceneEntity | null {
    return this.byPath.get(path) || null
  }

  /**
   * The nearest surviving entity for a path from a previous tree: the path
   * itself if the rebuilt scene still has it, else its deepest surviving
   * ancestor. What makes selections outlive a shape edit without silently
   * jumping to an unrelated node.
   */
  resolve(path: string): SceneEntity | null {
    const segs = path.split('/')
    while (segs.length) {
      const e = this.byPath.get(segs.join('/'))
      if (e) return e
      segs.pop()
    }
    return null
  }
}

//
// Geometry over entities — index arithmetic only; world transforms are applied
// by callers through the mat's own groups.
//

/** Block sizes + gap for a mat, in the shape elementPosition expects. */
export function matLayoutInfo(mat: any) {
  const { i, j } = mat.getBlockInfo()
  return { i, j, gap: mat.params.layout.gap }
}

/**
 * Element (i, j)'s centre in the mat's inner_group space. Same arithmetic as
 * emptyPoints / blockQuad — via the shared elementPosition, so the overlay a
 * selection draws sits exactly on the element the data says it covers.
 */
export function cellLocal(mat: any, i: number, j: number) {
  const { x, y } = elementPosition(i, j, matLayoutInfo(mat))
  return { x, y, z: 0 }
}

/**
 * Inverse of elementPosition along one axis: display coordinate → cell index.
 *
 * Display x for column j is `j + floor(j / sj) * gap`, so block b spans
 * [b*(sj+gap), b*(sj+gap) + sj). A coordinate landing in the gap after block b
 * belongs to no cell; `round` selects the nearest cell centre first, and the
 * result is clamped to the axis so a drag that overshoots the matrix edge
 * still selects the edge cell.
 */
export function dispToCell(x: number, size: number, gap: number, n: number): number {
  const stride = size + gap
  const b = Math.max(0, Math.floor(x / stride))
  const within = Math.min(size - 1, Math.max(0, Math.round(x - b * stride)))
  return Math.max(0, Math.min(n - 1, b * size + within))
}

/** Local inner_group point → cell indices, clamped into the matrix. */
export function localToCell(mat: any, x: number, y: number) {
  const info = matLayoutInfo(mat)
  return {
    i: dispToCell(y, info.i.size, info.gap, mat.H),
    j: dispToCell(x, info.j.size, info.gap, mat.W),
  }
}
