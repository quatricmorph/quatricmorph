//
// picking.ts — pixel → logical tensor coordinate.
//
// What this file pins: the element-index contract end to end (a ray through a
// known element centre reports that element, through the SceneTree, on real
// PointCloud geometry), the save/restore of the shared raycaster's knobs
// (main.ts uses far = 0 as the spotlight's off switch — a picker that left it
// changed would kill the lens), the level expansion, and the display-axis
// inverses that make box select exact across block gaps. Deliberately
// untested: nothing here renders; geometry and cameras are pure math.
//
import { describe, it, expect } from 'vitest'
import * as THREE from 'three'
import * as viz from '../src/viz.js'
import { SceneTree, cellLocal } from '../src/scenetree.js'
import {
  Picker, levelRange, cellCeil, cellFloor, rectRangeForMat, rectSelect,
  isWorldVisible,
} from '../src/picking.js'

const lf = (name, h, w, init = 'row major') => ({
  name, matmul: false, h, w, init, url: '', expr: '', min: 0, max: 1, dropout: 0,
})

const OPTS = () => ({
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
})

// A built 4×4 matmul: geometry exists (spheres), no text (legends 0), and a
// real camera in ctx because setLegends calls isFacing() before its size
// guard. Nothing renders.
function scene() {
  const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 10000)
  camera.position.set(0, 0, 60)
  const ctx = { raycaster: new THREE.Raycaster(), camera, pointer: new THREE.Vector2() }
  const mm = new viz.MatMul({
    ...OPTS(), name: 'out', left: lf('L', 4, 4), right: lf('R', 4, 4),
  }, ctx, true)
  mm.group.updateMatrixWorld(true)
  const tree = new SceneTree(mm, 'out')
  return { mm, tree, camera, raycaster: ctx.raycaster }
}

/** World centre of the result mat's element (i, j). */
function worldCell(mm, i, j) {
  const local = cellLocal(mm.result, i, j)
  mm.result.inner_group.updateWorldMatrix(true, false)
  return new THREE.Vector3(local.x, local.y, local.z)
    .applyMatrix4(mm.result.inner_group.matrixWorld)
}

describe('the element-index contract on real geometry', () => {
  it('a ray through element (1, 2) of the result reports index 1·W + 2', () => {
    const { mm } = scene()
    const p = worldCell(mm, 1, 2)
    const rc = new THREE.Raycaster(
      p.clone().add(new THREE.Vector3(0, 0, 10)), new THREE.Vector3(0, 0, -1))
    rc.params.Points.threshold = 0.4
    const hits: any[] = []
    rc.intersectObject(mm.result.points, true, hits)
    expect(hits.length).toBeGreaterThan(0)
    hits.sort((a, b) => (a.distanceToRay || 0) - (b.distanceToRay || 0))
    expect(hits[0].index).toBe(1 * 4 + 2)
  })
})

describe('Picker.pick through a camera', () => {
  it('resolves the entity and cell under the pointer, and restores the raycaster knobs', () => {
    const { mm, tree, camera } = scene()
    const p = worldCell(mm, 1, 2)
    camera.position.copy(p).add(new THREE.Vector3(0, 0, 25))
    camera.lookAt(p)
    camera.updateMatrixWorld(true)
    camera.updateProjectionMatrix()

    const rc = new THREE.Raycaster()
    rc.params.Points.threshold = 0.123   // the spotlight's own setting
    rc.far = 0                           // the spotlight's off switch
    const picker = new Picker(rc, camera)
    const hit = picker.pick({ x: 0, y: 0 }, tree)

    expect(hit).toBeTruthy()
    expect(hit!.entity.path).toBe('out/result')
    expect(hit!.i).toBe(1)
    expect(hit!.j).toBe(2)
    // main.ts's state, exactly as it was
    expect(rc.params.Points.threshold).toBe(0.123)
    expect(rc.far).toBe(0)
  })

  it('never picks through a hidden subtree', () => {
    const { mm, tree, camera } = scene()
    const p = worldCell(mm, 1, 2)
    camera.position.copy(p).add(new THREE.Vector3(0, 0, 25))
    camera.lookAt(p)
    camera.updateMatrixWorld(true)

    mm.result.group.visible = false
    const hit = new Picker(new THREE.Raycaster(), camera).pick({ x: 0, y: 0 }, tree)
    expect(hit?.entity.path).not.toBe('out/result')
    expect(isWorldVisible(mm.result.points)).toBe(false)
  })
})

describe('levelRange', () => {
  // A fake mat is enough: levelRange is index arithmetic over H/W/block info.
  const mat = {
    H: 4, W: 4,
    getBlockInfo: () => ({ i: { size: 2 }, j: { size: 4 } }),
  }

  it('expands a cell to its row, column, display block or the whole matrix', () => {
    expect(levelRange(mat, 3, 1, 'scalar')).toEqual({ i: [3, 4], j: [1, 2] })
    expect(levelRange(mat, 3, 1, 'row')).toEqual({ i: [3, 4], j: [0, 4] })
    expect(levelRange(mat, 3, 1, 'col')).toEqual({ i: [0, 4], j: [1, 2] })
    // si = 2: cell (3, 1) sits in the second i-block → rows [2, 4); sj = 4:
    // one j-block → all columns.
    expect(levelRange(mat, 3, 1, 'block')).toEqual({ i: [2, 4], j: [0, 4] })
    expect(levelRange(mat, 3, 1, 'matrix')).toEqual({ i: [0, 4], j: [0, 4] })
  })
})

describe('cellCeil / cellFloor across block gaps (size 4, gap 4, n 8)', () => {
  // display x: block 0 → 0..3, gap 4..7, block 1 → 8..11.
  it('cellCeil finds the first centre at or right of x', () => {
    expect(cellCeil(0, 4, 4, 8)).toBe(0)
    expect(cellCeil(3, 4, 4, 8)).toBe(3)      // exactly on a centre
    expect(cellCeil(3.2, 4, 4, 8)).toBe(4)    // past block 0's last centre → next block
    expect(cellCeil(8, 4, 4, 8)).toBe(4)      // block 1's first centre
    expect(cellCeil(-5, 4, 4, 8)).toBe(0)
    expect(cellCeil(100, 4, 4, 8)).toBe(8)    // n: nothing right of x
  })

  it('cellFloor finds the last centre at or left of x', () => {
    expect(cellFloor(3.2, 4, 4, 8)).toBe(3)
    expect(cellFloor(2.5, 4, 4, 8)).toBe(2)
    expect(cellFloor(7.9, 4, 4, 8)).toBe(3)   // still in the gap after block 0
    expect(cellFloor(8, 4, 4, 8)).toBe(4)
    expect(cellFloor(-0.1, 4, 4, 8)).toBe(-1) // nothing left of x
    expect(cellFloor(11, 4, 4, 8)).toBe(7)
  })
})

describe('rectRangeForMat / rectSelect', () => {
  it('an NDC rect padded around cells (1,1)…(2,2) selects exactly the middle 2×2', () => {
    const { mm, tree, camera } = scene()
    // face-on, far enough that perspective barely shears the rect
    const centre = worldCell(mm, 1.5 as any, 1.5 as any)   // between the four cells
    camera.position.copy(worldCell(mm, 1, 1)).lerp(worldCell(mm, 2, 2), 0.5)
    camera.position.z += 200
    camera.lookAt(centre)
    camera.updateMatrixWorld(true)
    camera.updateProjectionMatrix()

    const a = worldCell(mm, 1, 1).project(camera)
    const b = worldCell(mm, 2, 2).project(camera)
    const c = worldCell(mm, 1, 2).project(camera)
    const pad = Math.abs(c.x - a.x) / 2      // half the projected cell pitch
    const rect = {
      x0: Math.min(a.x, b.x) - pad, x1: Math.max(a.x, b.x) + pad,
      y0: Math.min(a.y, b.y) - pad, y1: Math.max(a.y, b.y) + pad,
    }
    expect(rectRangeForMat(camera, mm.result, rect)).toEqual({ i: [1, 3], j: [1, 3] })

    // and through the scene-wide sweep, the result mat is among those found
    const found = rectSelect(camera, tree, rect)
    const forResult = found.find(f => f.entity.path === 'out/result')
    expect(forResult).toBeTruthy()
    expect(forResult!.range).toEqual({ i: [1, 3], j: [1, 3] })
  })

  it('returns null for a rect wholly beside the matrix instead of clamping into it', () => {
    const { mm, camera } = scene()
    camera.position.copy(worldCell(mm, 1, 1)).add(new THREE.Vector3(0, 0, 100))
    camera.lookAt(worldCell(mm, 1, 1))
    camera.updateMatrixWorld(true)
    // far off to the side in NDC
    expect(rectRangeForMat(camera, mm.result, { x0: 0.8, x1: 0.99, y0: 0.8, y1: 0.99 }))
      .toBeNull()
  })
})
