"use strict"

//
// The tensor inspector: the editor's claims panel.
//
// A fixed dark HUD on the right edge printing what the managers know — the
// current selection and its exact statistics, the active entity and what its
// *picture* honestly is, the hovered and pinned cells, the edit stack with its
// per-op controls, and the named selection sets. Every fact here is owned by
// someone else: SelectionManager owns what is selected, EditStack owns the
// ops, the scene tree owns shapes, the Mat owns what its heatmap actually
// drew. The panel reads them at render time and owns no state beyond the last
// hover and its own collapse bit.
//
// Deliberately not here: THREE, the renderer, picking, or any pixel — the
// module renders no graphics and is importable and fully exercisable under
// jsdom. And one rule carried over from the rest of the repository: the
// picture and the values make separate claims. A reduced heatmap is labelled
// with its LOD and reducer; statistics are labelled exact because they are
// computed from the FP32 Array2D in memory (see selection.ts); a reduced
// picture is never allowed to read as exact.
//
// Entity names and paths come from checkpoint metadata, so they go into the
// DOM via textContent only — never innerHTML — a tensor named '<b>x</b>'
// must render as those seven characters, not as markup.
//

import { fmtValue, formatCell, formatPath, formatRanges } from './address.js'
import { SelectionManager, VisibilityState } from './selection.js'
import { EditStack, EditOp, OpKind, OP_KINDS } from './editops.js'
import { SceneTree } from './scenetree.js'

export interface HoverInfo {
  path: string
  name: string
  i: number
  j: number
  value: number
}

export interface InspectorDeps {
  selection: SelectionManager
  edits: EditStack
  visibility: VisibilityState
  getTree: () => SceneTree | null
  getCursor: () => { path: string, i: number, j: number, value: number } | null
  /** Current selection level name (address.ts LEVELS). */
  getLevel: () => string
  /** Ask the app to frame an entity; the inspector never touches the camera. */
  focusEntity: (path: string) => void
  /** Ask the app to add an op over the current selection; the inspector never
   *  writes into the EditStack itself — addressing is the controller's job. */
  applyOpToSelection: (kind: OpKind, params: { value?: number, min?: number, max?: number }) => void
}

//
// One <style> per document, shared by every inspector ever created and
// deliberately left in place by dispose() — removing it would restyle a
// surviving pane. The id guards against double injection.
//
const STYLE_ID = 'qme-inspector-style'

// Bottom-right, below where lil-gui sits (lil-gui docks top-right).
const CSS = `
.qme-inspector {
  position: fixed; right: 12px; bottom: 12px; width: 250px;
  max-height: 70vh; overflow-y: auto; z-index: 10; box-sizing: border-box;
  background: rgba(0,0,0,0.55); color: #cfd3e6; border-radius: 4px;
  font: 11px -apple-system, 'Segoe UI', sans-serif; padding: 6px 8px;
}
.qme-header { display: flex; justify-content: space-between; align-items: center; font-weight: 600; }
.qme-toggle { background: none !important; border: none !important; padding: 0 2px; }
.qme-section { margin-top: 6px; }
.qme-h { color: #8f95b2; font-size: 10px; text-transform: uppercase; letter-spacing: 0.4px; margin-bottom: 2px; }
.qme-line { margin: 1px 0; word-break: break-all; }
.qme-dim { opacity: 0.6; }
.qme-warn { color: #e0a33a; }
.qme-badge { border: 1px solid #4a5070; border-radius: 3px; padding: 0 3px; margin-left: 4px; font-size: 10px; }
.qme-inspector button {
  background: #2a2f45; color: inherit; border: 1px solid #4a5070;
  border-radius: 3px; font: inherit; cursor: pointer; padding: 1px 5px;
}
.qme-inspector button:hover { background: #3a4060; }
.qme-inspector input, .qme-inspector select, .qme-inspector textarea {
  background: #1a1e2e; color: inherit; border: 1px solid #4a5070; border-radius: 3px; font: inherit;
}
.qme-breadcrumb { display: block; width: 100%; text-align: left; word-break: break-all; }
.qme-op { display: flex; gap: 3px; align-items: center; margin: 2px 0; }
.qme-op-label { flex: 1; word-break: break-all; }
.qme-toolbar { display: flex; gap: 3px; flex-wrap: wrap; align-items: center; margin-top: 3px; }
.qme-value, .qme-min, .qme-max { width: 52px; }
.qme-set-name { width: 90px; }
.qme-export-out { width: 100%; height: 80px; margin-top: 3px; display: none; }
`

function ensureStyle() {
  if (document.getElementById(STYLE_ID)) return
  const style = document.createElement('style')
  style.id = STYLE_ID
  style.textContent = CSS
  document.head.appendChild(style)
}

//
// DOM helpers. Everything is createElement + textContent: user-controlled
// strings (names, paths) never meet innerHTML.
//

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K, cls = '', text?: string): HTMLElementTagNameMap[K] => {
  const e = document.createElement(tag)
  if (cls) e.className = cls
  if (text !== undefined) e.textContent = text
  return e
}

const btn = (cls: string, label: string, onclick: () => void): HTMLButtonElement => {
  const b = el('button', cls, label)
  b.type = 'button'
  b.addEventListener('click', onclick)
  return b
}

const line = (parent: HTMLElement, text: string, cls = 'qme-line'): HTMLDivElement => {
  const d = el('div', cls, text)
  parent.appendChild(d)
  return d
}

/** `#id kind(params) on <last path segment> [ranges or 'whole']`. */
const opSummary = (op: EditOp): string => {
  const params = Object.entries(op.params)
    .map(([k, v]) => `${k}=${fmtValue(v as number)}`).join(', ')
  const where = op.ranges === null ? 'whole' : formatRanges(op.ranges)
  return `#${op.id} ${op.kind}(${params}) on ${op.path.split('/').pop()} [${where}]`
}

export function createInspector(deps: InspectorDeps): {
  root: HTMLElement
  setHover(h: HoverInfo | null): void
  refresh(): void
  dispose(): void
} {
  ensureStyle()

  let hover: HoverInfo | null = null
  let collapsed = false

  const root = el('div', 'qme-inspector')
  const body = el('div', 'qme-body')
  const header = el('div', 'qme-header')
  const toggle = btn('qme-toggle', '▾', () => {
    collapsed = !collapsed
    body.style.display = collapsed ? 'none' : ''
    toggle.textContent = collapsed ? '▸' : '▾'
  })
  header.append(el('span', '', 'Tensor Inspector'), toggle)
  root.append(header, body)

  const section = (cls: string, heading: string): HTMLDivElement => {
    const s = el('div', `qme-section ${cls}`)
    s.appendChild(el('div', 'qme-h', heading))
    const content = el('div', 'qme-content')
    s.appendChild(content)
    body.appendChild(s)
    return content
  }

  const selection_c = section('qme-selection', 'Selection')
  const active_c = section('qme-active', 'Active')
  const stats_c = section('qme-stats', 'Statistics')
  const hover_c = section('qme-hover', 'Hover')
  const cursor_c = section('qme-cursor', 'Cursor')
  const edits_c = section('qme-edits', 'Edit stack')
  const sets_c = section('qme-sets', 'Selection sets')

  //
  // Edit stack: a dynamic ops list plus a persistent toolbar. The toolbar is
  // built once so a half-typed value survives the refreshes that selection
  // and edit events trigger.
  //
  const ops_list = el('div', 'qme-ops')
  const notes = el('div', 'qme-notes')
  edits_c.append(ops_list, notes)

  const toolbar = el('div', 'qme-toolbar')
  const kind_sel = el('select', 'qme-kind')
  for (const k of OP_KINDS) {
    const o = el('option', '', k)
    o.value = k
    kind_sel.appendChild(o)
  }
  const value_in = el('input', 'qme-value')
  value_in.type = 'number'
  value_in.placeholder = 'value'
  const min_in = el('input', 'qme-min')
  min_in.type = 'number'
  min_in.placeholder = 'min'
  const max_in = el('input', 'qme-max')
  max_in.type = 'number'
  max_in.placeholder = 'max'

  const syncToolbar = () => {
    const k = kind_sel.value
    value_in.style.display = (k === 'fill' || k === 'scale' || k === 'add') ? '' : 'none'
    min_in.style.display = k === 'clamp' ? '' : 'none'
    max_in.style.display = k === 'clamp' ? '' : 'none'
  }
  kind_sel.addEventListener('change', syncToolbar)
  syncToolbar()

  const apply_b = btn('qme-apply', 'Apply', () => {
    const kind = kind_sel.value as OpKind
    if (kind === 'zero') {
      deps.applyOpToSelection(kind, {})
      return
    }
    if (kind === 'clamp') {
      const min = parseFloat(min_in.value)
      const max = parseFloat(max_in.value)
      const params: { min?: number, max?: number } = {}
      if (!isNaN(min)) params.min = min
      if (!isNaN(max)) params.max = max
      // A clamp with neither bound is a no-op request, and EditStack would
      // refuse NaN params anyway — skip before it gets that far.
      if (params.min === undefined && params.max === undefined) return
      deps.applyOpToSelection(kind, params)
      return
    }
    const value = parseFloat(value_in.value)
    if (isNaN(value)) return          // an unparseable value applies nothing
    deps.applyOpToSelection(kind, { value })
  })
  toolbar.append(kind_sel, value_in, min_in, max_in, apply_b)

  // Export writes into a textarea rather than the clipboard: the clipboard
  // API needs permissions and does not exist under jsdom; a visible textarea
  // is inspectable in both places.
  const export_out = el('textarea', 'qme-export-out')
  // Inline, not only the stylesheet rule: the hidden/revealed state is part
  // of the panel's observable behaviour, and inline style is what a test
  // (and a reader) can actually see under jsdom.
  export_out.style.display = 'none'
  export_out.readOnly = true
  const history = el('div', 'qme-toolbar')
  history.append(
    btn('qme-undo', 'Undo', () => deps.edits.undo()),
    btn('qme-redo', 'Redo', () => deps.edits.redo()),
    btn('qme-clear', 'Clear', () => deps.edits.clearAll()),
    btn('qme-export', 'Export', () => {
      export_out.value = deps.edits.serialize()
      export_out.style.display = 'block'
    }),
  )
  edits_c.append(toolbar, history, export_out)

  //
  // Selection sets: persistent controls, options refilled on refresh —
  // saveSet does not emit a selection change, so Save refreshes explicitly.
  //
  const sets_bar = el('div', 'qme-toolbar')
  const set_name = el('input', 'qme-set-name')
  set_name.type = 'text'
  set_name.placeholder = 'name'
  const set_sel = el('select', 'qme-set-select')
  sets_bar.append(
    set_name,
    btn('qme-set-save', 'Save', () => {
      const name = set_name.value.trim()
      if (!name) return
      deps.selection.saveSet(name)
      refresh()
    }),
    set_sel,
    btn('qme-set-apply', 'Apply', () => {
      if (set_sel.value) deps.selection.applySet(set_sel.value)
    }),
  )
  sets_c.appendChild(sets_bar)

  //
  // Section renderers. Each reads the managers fresh — nothing here caches a
  // number a manager could have changed behind it.
  //

  const renderSelection = () => {
    selection_c.replaceChildren()
    line(selection_c, `level: ${deps.getLevel()}`)
    const s = deps.selection
    if (s.isEmpty()) {
      line(selection_c, 'nothing selected', 'qme-line qme-dim')
      return
    }
    const n = s.paths().length
    line(selection_c, `${n} ${n === 1 ? 'entity' : 'entities'}, ${s.countCells()} cells`)
    const hidden = deps.visibility.hidden.size
    if (hidden > 0) line(selection_c, `${hidden} hidden`, 'qme-line qme-dim')
  }

  const renderActive = () => {
    active_c.replaceChildren()
    const path = deps.selection.activePath()
    const tree = deps.getTree()
    const e = path && tree ? tree.get(path) : null
    if (!e) {
      line(active_c, 'none', 'qme-line qme-dim')
      return
    }
    const crumb = btn('qme-breadcrumb', formatPath(e.path), () => deps.focusEntity(e.path))
    active_c.appendChild(crumb)
    line(active_c, `kind: ${e.kind}`)
    if (!e.mat) return
    line(active_c, `shape: ${e.mat.H} × ${e.mat.W}`)
    // The picture's claim. getHeatmapInfo only exists on a built Mat, and
    // `heat` only after initViz — hence the points guard: a data-only mat
    // (init_viz=false, or a fake in a test) is drawn as nothing yet.
    let picture = 'picture: elements'
    if (e.mat.points) {
      const hm = e.mat.getHeatmapInfo()
      if (hm) {
        picture = hm.lod > 1 ?
          `picture: LOD ${Math.log2(hm.lod)} (${hm.reducer}), ${hm.texels} texels` :
          'picture: exact (1 texel/element)'
      }
    }
    line(active_c, picture)
    // The values' claim — always, so a reduced picture never reads as exact.
    line(active_c, 'values: exact FP32 in memory')
  }

  const renderStats = () => {
    stats_c.replaceChildren()
    const path = deps.selection.activePath()
    const tree = deps.getTree()
    const e = path && tree ? tree.get(path) : null
    const st = e && e.mat ? deps.selection.statsFor(e.path) : null
    if (!st) {
      line(stats_c, '—', 'qme-line qme-dim')
      return
    }
    const head = el('div', 'qme-line', `cells ${st.cells}, finite ${st.finite}`)
    head.appendChild(el('span', 'qme-badge', st.exactness))
    stats_c.appendChild(head)
    line(stats_c, `min ${fmtValue(st.min)}`)
    line(stats_c, `max ${fmtValue(st.max)}`)
    line(stats_c, `mean ${fmtValue(st.mean)}`)
    line(stats_c, `std ${fmtValue(st.std)}`)
    line(stats_c, `L1 ${fmtValue(st.l1)}`)
    line(stats_c, `L2 ${fmtValue(st.l2)}`)
    line(stats_c, `zeros ${st.zeros}`)
    const bad = line(stats_c, `NaN ${st.nans}, Inf ${st.infs}`)
    if (st.nans > 0 || st.infs > 0) bad.classList.add('qme-warn')
  }

  const renderHover = () => {
    hover_c.replaceChildren()
    if (!hover) return
    line(hover_c, `${formatCell(hover.name, hover.i, hover.j)} = ${fmtValue(hover.value)}`)
    const tree = deps.getTree()
    const e = tree ? tree.get(hover.path) : null
    if (e && e.mat) {
      const { i, j } = e.mat.getBlockInfo()
      line(hover_c, `block [${Math.floor(hover.i / i.size)}, ${Math.floor(hover.j / j.size)}]`)
    }
  }

  const renderCursor = () => {
    cursor_c.replaceChildren()
    const c = deps.getCursor()
    if (!c) {
      line(cursor_c, 'not set (press C over a cell)', 'qme-line qme-dim')
      return
    }
    line(cursor_c, `${formatCell(c.path, c.i, c.j)} = ${fmtValue(c.value)}`)
  }

  const renderOps = () => {
    ops_list.replaceChildren()
    notes.replaceChildren()
    const ops = deps.edits.ops
    if (!ops.length) {
      line(ops_list, 'no edits', 'qme-line qme-dim')
      return
    }
    for (const op of ops) {
      const row = el('div', 'qme-op')
      const enabled = el('input', 'qme-op-enabled')
      enabled.type = 'checkbox'
      enabled.checked = op.enabled
      enabled.addEventListener('change', () => deps.edits.setEnabled(op.id, enabled.checked))
      row.append(
        enabled,
        el('span', 'qme-op-label', opSummary(op)),
        btn('qme-op-up', '▲', () => deps.edits.moveOp(op.id, -1)),
        btn('qme-op-down', '▼', () => deps.edits.moveOp(op.id, +1)),
        btn('qme-op-del', '✕', () => deps.edits.removeOp(op.id)),
      )
      ops_list.appendChild(row)
    }
    // The two honest limitations editops.ts states; repeated where the user
    // is looking when they bite.
    line(notes, 'colour range pinned at load; stats are exact', 'qme-line qme-note qme-dim')
    line(notes, 'edits propagate within a stage, not across stack stages', 'qme-line qme-note qme-dim')
  }

  const renderSets = () => {
    const names = deps.selection.setNames()
    const current = set_sel.value
    set_sel.replaceChildren()
    for (const n of names) {
      const o = el('option', '', n)      // textContent — names are user input
      o.value = n
      set_sel.appendChild(o)
    }
    if (names.indexOf(current) >= 0) set_sel.value = current
  }

  const refresh = () => {
    renderSelection()
    renderActive()
    renderStats()
    renderHover()
    renderCursor()
    renderOps()
    renderSets()
  }

  const setHover = (h: HoverInfo | null) => {
    hover = h
    renderHover()
  }

  const unsubs = [
    deps.selection.onChange(() => refresh()),
    deps.edits.onChange(() => refresh()),
  ]

  const dispose = () => {
    unsubs.forEach(u => u())
    unsubs.length = 0
    root.remove()
    // The shared <style> stays: another inspector on the page may be using it.
  }

  refresh()
  return { root, setHover, refresh, dispose }
}
