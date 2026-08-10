"use strict"

//
// The edit stack: non-destructive tensor operations.
//
// The interaction UX resembles Blender's modifier stack, and the internals
// follow this repository's immutability rule rather than mesh editing: no op
// ever mutates a matrix as *the* data. Instead:
//
//   pristine values (captured before the first op touches a matrix)
//     → enabled ops, in stack order
//       → recomputed downstream products (a matmul result is a claim about
//         its operands; leaving a stale product on screen after editing an
//         operand would be a picture of an arithmetic that never happened)
//         → visual refresh
//
// Disabling, reordering or removing an op therefore *recomputes from
// pristine* — an op is a description of a transform, never an accumulated
// mutation. Undo/redo snapshots the op list, not the data.
//
// Two honest limitations, stated rather than papered over:
//  * Cross-stage links in a staged model view are baked by the page from
//    served CSVs; edits propagate within a stage's own expression tree and
//    stop at the stack boundary. The inspector labels this.
//  * A Mat's colour range (absmax) is measured at construction and pinned —
//    mm-wide behaviour. Edited values outside it clamp in the picture; the
//    inspector's statistics show the exact numbers.
//

import { CellRange, fullRange, forEachCell } from './address.js'
import { SceneTree, SceneEntity } from './scenetree.js'
import { applyUnary } from '../scene/viz.js'

export const OP_KINDS = ['zero', 'fill', 'scale', 'add', 'clamp'] as const
export type OpKind = typeof OP_KINDS[number]

export interface EditOp {
  id: number
  /** Scene-tree path of the target mat. */
  path: string
  /** null = the whole matrix. */
  ranges: CellRange[] | null
  kind: OpKind
  /** zero: —; fill/scale/add: value; clamp: min, max. */
  params: { value?: number, min?: number, max?: number }
  enabled: boolean
}

interface OpsSnapshot { ops: EditOp[], next_id: number }

const copyRanges = (ranges: CellRange[] | null): CellRange[] | null =>
  ranges === null ? null :
    ranges.map(r => ({ i: [r.i[0], r.i[1]] as [number, number], j: [r.j[0], r.j[1]] as [number, number] }))

const copyOps = (ops: EditOp[]): EditOp[] => ops.map(o => ({
  ...o,
  params: { ...o.params },
  ranges: copyRanges(o.ranges),
}))

/** Apply one op to an Array2D in place. Pure arithmetic; hand-testable. */
export function applyOpToData(data: any, op: EditOp) {
  const ranges = op.ranges === null ? [fullRange(data.h, data.w)] : op.ranges
  const p = op.params
  const f =
    op.kind === 'zero' ? (_: number) => 0 :
      op.kind === 'fill' ? (_: number) => (p.value ?? 0) :
        op.kind === 'scale' ? (x: number) => x * (p.value ?? 1) :
          op.kind === 'add' ? (x: number) => x + (p.value ?? 0) :
            (x: number) => Math.min(p.max ?? Infinity, Math.max(p.min ?? -Infinity, x))
  const arr = data.data
  forEachCell(ranges, (i, j) => {
    if (i < data.h && j < data.w) {
      const a = i * data.w + j
      arr[a] = f(arr[a])
    }
  })
}

/** Do any of `paths` fall inside this entity's operand subtrees (not its result)? */
function operandsEdited(e: SceneEntity, edited: Set<string>): boolean {
  const operands = e.children.filter(c => c.role !== 'result')
  const under = (q: SceneEntity): boolean => {
    if (edited.has(q.path)) return true
    return q.children.some(under)
  }
  return operands.some(under)
}

export class EditStack {
  ops: EditOp[] = []
  /** Paths whose data changed in the last recompute — what needs repainting. */
  lastTouched: Set<string> = new Set()
  private next_id = 1
  private pristine = new Map<string, Float32Array>()
  private undo_stack: OpsSnapshot[] = []
  private redo_stack: OpsSnapshot[] = []
  private listeners = new Set<() => void>()
  private getTree: () => SceneTree | null

  constructor(getTree: () => SceneTree | null) {
    this.getTree = getTree
  }

  onChange(f: () => void): () => void {
    this.listeners.add(f)
    return () => this.listeners.delete(f)
  }

  private emit() {
    this.listeners.forEach(f => f())
  }

  /** The scene was rebuilt: captured baselines describe freed arrays. Drop
   *  them and re-derive from the fresh data — ops are descriptions and
   *  reapply, exactly like modifiers across a mesh reload. */
  onTreeRebuilt(): Set<string> {
    this.pristine.clear()
    return this.ops.length ? this.recomputeAll() : new Set()
  }

  private snapshot(): OpsSnapshot {
    return { ops: copyOps(this.ops), next_id: this.next_id }
  }

  private push() {
    this.undo_stack.push(this.snapshot())
    if (this.undo_stack.length > 128) this.undo_stack.shift()
    this.redo_stack.length = 0
  }

  private restore(s: OpsSnapshot) {
    this.ops = copyOps(s.ops)
    this.next_id = s.next_id
    this.recomputeAll()
    this.emit()
  }

  undo(): boolean {
    const prev = this.undo_stack.pop()
    if (!prev) return false
    this.redo_stack.push(this.snapshot())
    this.restore(prev)
    return true
  }

  redo(): boolean {
    const next = this.redo_stack.pop()
    if (!next) return false
    this.undo_stack.push(this.snapshot())
    this.restore(next)
    return true
  }

  addOp(path: string, ranges: CellRange[] | null, kind: OpKind,
    params: EditOp['params'] = {}): EditOp | null {
    const tree = this.getTree()
    const e = tree?.get(path)
    if (!e?.mat) return null
    // Refuse a transform whose parameters cannot mean anything, before it
    // lands in the stack: NaN would silently poison every downstream product.
    for (const v of Object.values(params)) {
      if (typeof v === 'number' && isNaN(v)) return null
    }
    this.push()
    const op: EditOp = {
      id: this.next_id++, path,
      ranges: copyRanges(ranges),
      kind, params: { ...params }, enabled: true,
    }
    this.ops.push(op)
    this.recomputeAll()
    this.emit()
    return op
  }

  removeOp(id: number) {
    if (!this.ops.some(o => o.id === id)) return
    this.push()
    this.ops = this.ops.filter(o => o.id !== id)
    this.recomputeAll()
    this.emit()
  }

  setEnabled(id: number, enabled: boolean) {
    const op = this.ops.find(o => o.id === id)
    if (!op || op.enabled === enabled) return
    this.push()
    op.enabled = enabled
    this.recomputeAll()
    this.emit()
  }

  /** Move an op ±1 in stack order. Order matters: scale then add ≠ add then scale. */
  moveOp(id: number, delta: number) {
    const k = this.ops.findIndex(o => o.id === id)
    const to = k + delta
    if (k < 0 || to < 0 || to >= this.ops.length) return
    this.push()
    const [op] = this.ops.splice(k, 1)
    this.ops.splice(to, 0, op)
    this.recomputeAll()
    this.emit()
  }

  clearAll() {
    if (!this.ops.length) return
    this.push()
    this.ops = []
    this.recomputeAll()
    this.emit()
  }

  opsFor(path: string): EditOp[] {
    return this.ops.filter(o => o.path === path)
  }

  /** Provenance: the whole stack as JSON — every operation recorded. */
  serialize(): string {
    return JSON.stringify({ version: 1, ops: this.ops }, null, 2)
  }

  /**
   * Recompute the scene from pristine + stack. Also the reconciliation point
   * after undo/enable/reorder — data state is always f(pristine, ops), never
   * an accumulation.
   *
   * Returns the set of paths whose data changed hands (edited + recomputed),
   * so the caller can refresh exactly those visuals.
   */
  recomputeAll(): Set<string> {
    const tree = this.getTree()
    const touched = new Set<string>()
    this.lastTouched = touched
    if (!tree) return touched

    // 1. Capture baselines for newly-touched paths, restore all known ones.
    for (const op of this.ops) {
      const e = tree.get(op.path)
      if (e?.mat && !this.pristine.has(op.path)) {
        this.pristine.set(op.path, new Float32Array(e.mat.data.data))
      }
    }
    for (const [path, arr] of this.pristine) {
      const e = tree.get(path)
      if (e?.mat && e.mat.data.data.length === arr.length) {
        e.mat.data.data.set(arr)
        touched.add(path)
      }
    }

    const enabled = this.ops.filter(o => o.enabled)

    // Downstream recompute follows every path that ever had a baseline
    // captured, not only currently-enabled ops: disabling or removing an op
    // restores its operand from pristine, and the product computed from the
    // *edited* operand is still on screen — only a recompute from the
    // restored operand takes the lie back down. Recomputed results join the
    // set so the chain climbs all the way up.
    const editedOrRecomputed = new Set([
      ...enabled.map(o => o.path),
      ...this.pristine.keys(),
    ])

    // 2. Post-order walk: operands first, then the product they feed, then
    // any ops targeting that product — so an op on a result survives the
    // recompute that an operand edit forces.
    const applyPathOps = (path: string) => {
      const e = tree.get(path)
      if (!e?.mat) return
      for (const op of enabled) {
        if (op.path === path) {
          applyOpToData(e.mat.data, op)
          touched.add(path)
        }
      }
    }

    const process = (e: SceneEntity) => {
      e.children.forEach(process)
      if (e.kind === 'mat') {
        applyPathOps(e.path)
        return
      }
      if (e.kind === 'stack') return    // stages are independent; stated above
      if (!operandsEdited(e, editedOrRecomputed)) return
      this.recomputeNodeResult(e)
      const result = e.children.find(c => c.role === 'result')
      if (result) {
        touched.add(result.path)
        editedOrRecomputed.add(result.path)
        applyPathOps(result.path)
      }
    }

    process(tree.root)
    return touched
  }

  /** result ← f(operands), by node kind. The same arithmetic construction ran. */
  private recomputeNodeResult(e: SceneEntity) {
    const node = e.node
    if (e.kind === 'matmul') {
      node.result.data.reinit(
        (i: number, j: number) => node.dotprod(i, j, 0, node.D),
        node.params.epilog)
    } else if (e.kind === 'unary') {
      const d = node.result.data.data
      d.set(node.input.getDataArray().subarray(0, d.length))
      applyUnary(node.fn, node.result.data.h, node.result.data.w, d)
    } else if (e.kind === 'add') {
      const l = node.left.getDataArray(), r = node.right.getDataArray()
      const d = node.result.data.data
      for (let k = 0; k < d.length; k++) d[k] = l[k] + r[k]
    }
  }
}

/**
 * Refresh the picture of every touched mat. Separate from recomputeAll so the
 * pure recompute stays testable with init_viz=false nodes (no geometry).
 */
export function refreshTouched(tree: SceneTree | null, touched: Set<string>) {
  if (!tree) return
  for (const path of touched) {
    const e = tree.get(path)
    if (e?.mat?.points) e.mat.setColorsAndSizes()
  }
}
