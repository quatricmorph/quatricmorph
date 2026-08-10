"use strict"

//
// Selection: the deepest primitive of the tensor editor.
//
// One SelectionManager is the single source of truth every surface consumes —
// viewport highlights, the outliner, the inspector, edits, framing. Selection
// state lives here and only here; the viewport renders it, it never owns it.
//
// A selection is a set of entities (by scene-tree path), each either selected
// whole (`ranges === null`) or in part (a disjoint list of CellRanges), plus
// one *active* entity — Blender's distinction: operations that need a single
// reference (frame, inspect, edit target) use the active entity; operations
// over "the selection" use them all.
//
// Deliberately NOT persisted into the params tree: util.flatten round-trips no
// arrays, so ranges cannot survive the URL. Selection is session state.
//
// No THREE, no DOM. Statistics are computed from the Mat's own Array2D — the
// exact FP32 values in memory — and are labelled exact for that reason. The
// *picture* may be a reduced heatmap; the numbers here never come from it.
//

import {
  CellRange, countCells, fullRange, invertRanges, rangesContain, rangesCover,
  addToRanges, subtractFromRanges, toggleRange, clampRange, forEachCell,
} from './address.js'
import { SceneTree, SceneEntity } from './scenetree.js'

export type SelectMode = 'set' | 'add' | 'toggle'

export interface EntitySelection {
  path: string
  /** null = the whole entity; otherwise a disjoint set of cell ranges. */
  ranges: CellRange[] | null
}

export interface SelectionSnapshot {
  items: EntitySelection[]
  active: string | null
}

/** Exact statistics over the selected cells of one Mat. */
export interface SelectionStats {
  cells: number
  finite: number
  min: number
  max: number
  absmax: number
  mean: number
  std: number
  l1: number
  l2: number
  zeros: number
  nans: number
  infs: number
  /** Always 'exact': computed from the FP32 Array2D, never from the picture. */
  exactness: 'exact'
}

const copyRanges = (r: CellRange[] | null): CellRange[] | null =>
  r === null ? null : r.map(q => ({ i: [q.i[0], q.i[1]] as [number, number], j: [q.j[0], q.j[1]] as [number, number] }))

export class SelectionManager {
  private items = new Map<string, CellRange[] | null>()
  private active: string | null = null
  private undo_stack: SelectionSnapshot[] = []
  private redo_stack: SelectionSnapshot[] = []
  private named = new Map<string, SelectionSnapshot>()
  private listeners = new Set<(type: string) => void>()
  private tree: SceneTree | null = null

  /** Lock Selection: guards against accidental change; all mutations no-op. */
  locked = false

  /** Undo depth. Selection undo is cheap; 128 steps is generous. */
  static readonly MAX_UNDO = 128

  //
  // wiring
  //

  onChange(f: (type: string) => void): () => void {
    this.listeners.add(f)
    return () => this.listeners.delete(f)
  }

  private emit(type: string) {
    this.listeners.forEach(f => f(type))
  }

  /**
   * Adopt a freshly rebuilt scene tree. Paths that no longer resolve are
   * re-anchored to their deepest surviving ancestor (selected whole), so a
   * selection survives a shape edit without silently pointing at nothing.
   */
  setTree(tree: SceneTree) {
    this.tree = tree
    const next = new Map<string, CellRange[] | null>()
    for (const [path, ranges] of this.items) {
      const e = tree.resolve(path)
      if (!e) continue
      if (e.path === path) {
        next.set(path, this.clampToEntity(e, ranges))
      } else if (!next.has(e.path)) {
        next.set(e.path, null)
      }
    }
    this.items = next
    if (this.active && !this.items.has(this.active)) {
      this.active = this.items.size ? [...this.items.keys()].pop() : null
    }
    this.emit('tree')
  }

  getTree(): SceneTree | null {
    return this.tree
  }

  private clampToEntity(e: SceneEntity, ranges: CellRange[] | null): CellRange[] | null {
    if (ranges === null || !e.mat) return null
    const out: CellRange[] = []
    for (const r of ranges) {
      const c = clampRange(r, e.mat.H, e.mat.W)
      if (c) out.push(c)
    }
    return out.length ? out : null    // an all-clipped selection falls back to whole
  }

  //
  // history
  //

  snapshot(): SelectionSnapshot {
    return {
      items: [...this.items].map(([path, ranges]) => ({ path, ranges: copyRanges(ranges) })),
      active: this.active,
    }
  }

  restore(snap: SelectionSnapshot) {
    this.items = new Map(snap.items.map(s => [s.path, copyRanges(s.ranges)]))
    this.active = snap.active
    this.emit('restore')
  }

  /** Called before every mutation: selection undo is separate from edit undo. */
  private push() {
    this.undo_stack.push(this.snapshot())
    if (this.undo_stack.length > SelectionManager.MAX_UNDO) this.undo_stack.shift()
    this.redo_stack.length = 0
  }

  undoSelection(): boolean {
    const prev = this.undo_stack.pop()
    if (!prev) return false
    this.redo_stack.push(this.snapshot())
    this.restore(prev)
    return true
  }

  redoSelection(): boolean {
    const next = this.redo_stack.pop()
    if (!next) return false
    this.undo_stack.push(this.snapshot())
    this.restore(next)
    return true
  }

  //
  // named selection sets
  //

  saveSet(name: string) {
    this.named.set(name, this.snapshot())
  }

  applySet(name: string): boolean {
    const snap = this.named.get(name)
    if (!snap || this.locked) return false
    this.push()
    this.restore(snap)
    return true
  }

  deleteSet(name: string) {
    this.named.delete(name)
  }

  setNames(): string[] {
    return [...this.named.keys()]
  }

  //
  // queries
  //

  isEmpty(): boolean {
    return this.items.size === 0
  }

  paths(): string[] {
    return [...this.items.keys()]
  }

  has(path: string): boolean {
    return this.items.has(path)
  }

  /** undefined = not selected; null = whole; array = partial. */
  rangesOf(path: string): CellRange[] | null | undefined {
    const r = this.items.get(path)
    return r === undefined ? undefined : copyRanges(r)
  }

  activePath(): string | null {
    return this.active
  }

  activeEntity(): SceneEntity | null {
    return this.active && this.tree ? this.tree.get(this.active) : null
  }

  covers(path: string, i: number, j: number): boolean {
    const r = this.items.get(path)
    if (r === undefined) return false
    return r === null || rangesContain(r, i, j)
  }

  /** Total selected cells. Whole interior entities count their descendant mats. */
  countCells(): number {
    let n = 0
    for (const [path, ranges] of this.items) {
      if (ranges !== null) {
        n += countCells(ranges)
        continue
      }
      const e = this.tree?.get(path)
      if (!e) continue
      const stack = [e]
      while (stack.length) {
        const q = stack.pop()!
        if (q.mat) n += q.mat.H * q.mat.W
        else stack.push(...q.children)
      }
    }
    return n
  }

  //
  // mutations
  //

  private begin(mode: SelectMode): boolean {
    if (this.locked) return false
    this.push()
    if (mode === 'set') this.items.clear()
    return true
  }

  /** Select an entity whole. */
  selectEntity(path: string, mode: SelectMode = 'set') {
    if (!this.begin(mode)) return
    if (mode === 'toggle' && this.items.get(path) === null) {
      this.items.delete(path)
      if (this.active === path) this.active = [...this.items.keys()].pop() || null
    } else {
      this.items.set(path, null)
      this.active = path
    }
    this.emit('select')
  }

  /** Select cells within an entity. */
  selectRange(path: string, range: CellRange, mode: SelectMode = 'set') {
    if (!this.begin(mode)) return
    const e = this.tree?.get(path)
    const cur = this.items.get(path)
    let next: CellRange[] | null
    if (cur === null) {
      // The whole entity is selected; toggling a region carves it out, adding
      // is a no-op refinement that keeps 'whole'.
      next = mode === 'toggle' && e?.mat ?
        invertRanges([range], e.mat.H, e.mat.W) :
        null
    } else {
      const base = cur === undefined ? [] : cur
      next = mode === 'toggle' ? toggleRange(base, range) : addToRanges(base, range)
    }
    if (next !== null && next.length === 0) {
      this.items.delete(path)
      if (this.active === path) this.active = [...this.items.keys()].pop() || null
    } else {
      this.items.set(path, next)
      this.active = path
    }
    this.emit('select')
  }

  deselectEntity(path: string) {
    if (this.locked || !this.items.has(path)) return
    this.push()
    this.items.delete(path)
    if (this.active === path) this.active = [...this.items.keys()].pop() || null
    this.emit('select')
  }

  /** Select every mat in the tree, whole. */
  selectAll() {
    if (this.locked || !this.tree) return
    this.push()
    this.items.clear()
    for (const e of this.tree.mats()) this.items.set(e.path, null)
    this.active = this.tree.mats().slice(-1)[0]?.path || null
    this.emit('select')
  }

  clear() {
    if (this.locked || (this.items.size === 0 && !this.active)) return
    this.push()
    this.items.clear()
    this.active = null
    this.emit('select')
  }

  /** Invert within the active entity (needs its dims). */
  invertActive() {
    if (this.locked || !this.active || !this.tree) return
    const e = this.tree.get(this.active)
    if (!e?.mat) return
    this.push()
    const cur = this.items.get(this.active)
    const { H, W } = e.mat
    if (cur === undefined) {
      this.items.set(this.active, null)
    } else if (cur === null) {
      this.items.delete(this.active)
      this.active = [...this.items.keys()].pop() || null
    } else {
      const inv = invertRanges(cur, H, W)
      if (inv.length === 0) {
        this.items.delete(this.active)
        this.active = [...this.items.keys()].pop() || null
      } else {
        this.items.set(this.active, inv)
      }
    }
    this.emit('select')
  }

  setActive(path: string | null) {
    if (this.locked) return
    this.active = path
    if (path && !this.items.has(path)) this.items.set(path, null)
    this.emit('select')
  }

  //
  // statistics — exact, from the Array2D
  //

  statsFor(path: string): SelectionStats | null {
    const e = this.tree?.get(path)
    if (!e?.mat) return null
    const sel = this.items.get(path)
    if (sel === undefined) return null
    const ranges = sel === null ? [fullRange(e.mat.H, e.mat.W)] : sel
    return selectionStats(e.mat.data, ranges)
  }
}

/** Exact stats over disjoint ranges of an Array2D. Pure; hand-testable. */
export function selectionStats(data: any, ranges: CellRange[]): SelectionStats {
  let cells = 0, finite = 0, zeros = 0, nans = 0, infs = 0
  let min = Infinity, max = -Infinity, sum = 0, sum2 = 0, l1 = 0
  forEachCell(ranges, (i, j) => {
    cells++
    const x = data.get(i, j)
    if (isNaN(x)) { nans++; return }
    if (!isFinite(x)) { infs++; return }
    finite++
    if (x === 0) zeros++
    if (x < min) min = x
    if (x > max) max = x
    sum += x
    sum2 += x * x
    l1 += Math.abs(x)
  })
  const mean = finite ? sum / finite : 0
  const variance = finite ? Math.max(0, sum2 / finite - mean * mean) : 0
  return {
    cells, finite, zeros, nans, infs,
    min: finite ? min : NaN,
    max: finite ? max : NaN,
    absmax: finite ? Math.max(Math.abs(min), Math.abs(max)) : NaN,
    mean, std: Math.sqrt(variance), l1, l2: Math.sqrt(sum2),
    exactness: 'exact',
  }
}

//
// Visibility: session state applied onto THREE groups by path, so it survives
// the group-identity churn of initViz rebuilds. Hiding is view state, not
// selection state, but the two travel together through every surface.
//

export class VisibilityState {
  hidden = new Set<string>()
  private listeners = new Set<() => void>()

  onChange(f: () => void): () => void {
    this.listeners.add(f)
    return () => this.listeners.delete(f)
  }

  private emit() {
    this.listeners.forEach(f => f())
  }

  isHidden(path: string): boolean {
    return this.hidden.has(path)
  }

  hide(paths: string[]) {
    paths.forEach(p => this.hidden.add(p))
    this.emit()
  }

  show(paths: string[]) {
    paths.forEach(p => this.hidden.delete(p))
    this.emit()
  }

  toggle(path: string) {
    this.hidden.has(path) ? this.hidden.delete(path) : this.hidden.add(path)
    this.emit()
  }

  showAll() {
    if (!this.hidden.size) return
    this.hidden.clear()
    this.emit()
  }

  /** Hide every mat NOT in `keep` (isolate / local view). */
  isolate(keep: Set<string>, tree: SceneTree) {
    for (const e of tree.mats()) {
      if (!keep.has(e.path) && !this.coveredBy(keep, e)) this.hidden.add(e.path)
    }
    this.emit()
  }

  /** Is some ancestor of e (or e itself) in the set? */
  private coveredBy(keep: Set<string>, e: SceneEntity): boolean {
    for (let q: SceneEntity | null = e; q; q = q.parent) if (keep.has(q.path)) return true
    return false
  }

  /**
   * Write the state onto the built scene. Called after every rebuild — group
   * identity does not survive initViz, paths do.
   */
  apply(tree: SceneTree) {
    for (const e of tree.entities) {
      if (e.node.group) e.node.group.visible = !this.hidden.has(e.path)
    }
  }
}
