"use strict"

//
// Outliner: the scene tree as a panel of rows.
//
// Plain DOM, no THREE, no renderer — the panel is a *view* over SceneTree +
// SelectionManager + VisibilityState and owns none of that state itself.
// Rows are identified by scene-tree path, never by element identity:
// refresh() throws the whole row list away and rebuilds it from getTree(),
// the same contract the highlight renderer follows, so a scene rebuild, a
// selection change and a visibility change are all one code path.
//
// The one piece of state that genuinely belongs to the outliner — which
// subtrees the user has opened — is a Set of expanded *paths* kept across
// refresh(), for the same reason selections are kept by path: paths survive
// initViz rebuilds, DOM nodes and viz objects do not.
//
// Names are written with textContent, never innerHTML: entity names come
// from the params tree, which rides in URLs, and a URL must not be able to
// inject markup into the panel.
//
// The eye button stops propagation deliberately: hiding is view state, not
// selection state (see VisibilityState), and a click that both hid a matrix
// and reselected it would conflate the two.
//

import { SceneTree, SceneEntity } from './scenetree.js'
import { SelectionManager, VisibilityState } from './selection.js'

export interface OutlinerDeps {
  selection: SelectionManager
  visibility: VisibilityState
  getTree: () => SceneTree | null
  focusEntity: (path: string) => void
}

const STYLE_ID = 'qme-outliner-style'

// One <style> per document, guarded by id — a second outliner (or a rebuild
// that constructs a new one) must not stack duplicate rules.
const CSS = `
.qme-outliner {
  position: fixed; left: 12px; top: 48px; width: 230px;
  max-height: 60vh; overflow-y: auto; z-index: 10;
  background: rgba(0,0,0,0.55); color: #cfd3e6;
  font: 11px -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  border-radius: 4px; user-select: none;
}
.qme-outliner-header {
  display: flex; align-items: center; gap: 6px; padding: 5px 8px;
  font-weight: 600; cursor: pointer;
  position: sticky; top: 0; background: rgba(0,0,0,0.35);
}
.qme-outliner-filter {
  display: block; width: calc(100% - 12px); margin: 4px 6px;
  box-sizing: border-box; padding: 2px 4px;
  background: rgba(255,255,255,0.08); color: inherit;
  border: 1px solid rgba(255,255,255,0.15); border-radius: 3px;
  font: inherit; outline: none;
}
.qme-row {
  display: flex; align-items: center; gap: 4px;
  padding: 1px 6px 1px 4px; cursor: pointer; white-space: nowrap;
  border-left: 2px solid transparent; /* so qme-active does not shift layout */
}
.qme-row:hover { background: rgba(255,255,255,0.06); }
.qme-row.qme-sel { background: rgba(255,140,26,0.25); }
.qme-row.qme-active { border-left: 2px solid #ffc266; }
.qme-row.qme-reveal { outline: 1px solid #ffc266; outline-offset: -1px; }
.qme-toggle { flex: none; width: 10px; text-align: center; color: #8b90a8; }
.qme-name { overflow: hidden; text-overflow: ellipsis; }
.qme-dims { color: #8b90a8; }
.qme-eye {
  flex: none; margin-left: auto; padding: 0 2px;
  background: none; border: none; color: inherit; font: inherit;
  cursor: pointer; opacity: 0.8;
}
`

function injectStyle() {
  if (document.getElementById(STYLE_ID)) return
  const style = document.createElement('style')
  style.id = STYLE_ID
  style.textContent = CSS
  document.head.appendChild(style)
}

export function createOutliner(deps: OutlinerDeps) {
  injectStyle()

  // Paths whose subtrees are open. `seen` records which paths have had the
  // depth default applied, so "expanded because depth < 2" happens once per
  // path and a collapse the user made is not undone by the next refresh.
  const expanded = new Set<string>()
  const seen = new Set<string>()

  // Row lookup by path, rebuilt with the rows. Kept as a Map rather than a
  // querySelector so revealPath never has to escape a path into a selector.
  const row_by_path = new Map<string, HTMLElement>()

  const root = document.createElement('div')
  root.className = 'qme-outliner'

  const header = document.createElement('div')
  header.className = 'qme-outliner-header'
  const collapse = document.createElement('span')
  collapse.className = 'qme-outliner-collapse'
  collapse.textContent = '▾'
  const title = document.createElement('span')
  title.textContent = 'Outliner'
  header.appendChild(collapse)
  header.appendChild(title)

  const body = document.createElement('div')
  body.className = 'qme-outliner-body'
  // The filter input lives outside the rebuilt row list on purpose: refresh()
  // runs on every keystroke, and rebuilding the input would drop its focus
  // and caret mid-word.
  const filter = document.createElement('input')
  filter.className = 'qme-outliner-filter'
  filter.type = 'text'
  filter.placeholder = 'filter…'
  const rows = document.createElement('div')
  rows.className = 'qme-outliner-rows'
  body.appendChild(filter)
  body.appendChild(rows)

  root.appendChild(header)
  root.appendChild(body)

  header.addEventListener('click', () => {
    const hidden = body.style.display === 'none'
    body.style.display = hidden ? '' : 'none'
    collapse.textContent = hidden ? '▾' : '▸'
  })

  filter.addEventListener('input', () => refresh())

  /** Case-insensitive substring match against name AND path. */
  const matches = (e: SceneEntity, needle: string) =>
    e.name.toLowerCase().includes(needle) || e.path.toLowerCase().includes(needle)

  /**
   * Matching entities plus every ancestor — the ancestors are shown even when
   * they do not match, so a filtered tree still reads as a tree rather than a
   * pile of orphaned leaves.
   */
  const matchSet = (tree: SceneTree, needle: string): Set<string> => {
    const keep = new Set<string>()
    for (const e of tree.entities) {
      if (!matches(e, needle)) continue
      for (let q: SceneEntity | null = e; q && !keep.has(q.path); q = q.parent) {
        keep.add(q.path)
      }
    }
    return keep
  }

  const buildRow = (e: SceneEntity, filtering: boolean): HTMLElement => {
    const row = document.createElement('div')
    row.className = 'qme-row'
    row.setAttribute('data-path', e.path)
    row.style.paddingLeft = `${4 + e.depth * 12}px`
    if (deps.selection.has(e.path)) row.classList.add('qme-sel')
    if (deps.selection.activePath() === e.path) row.classList.add('qme-active')

    const toggle = document.createElement('span')
    toggle.className = 'qme-toggle'
    if (e.children.length) {
      toggle.textContent = filtering || expanded.has(e.path) ? '▾' : '▸'
      toggle.addEventListener('click', ev => {
        ev.stopPropagation()
        expanded.has(e.path) ? expanded.delete(e.path) : expanded.add(e.path)
        refresh()
      })
    } else {
      toggle.textContent = ' '   // spacer, keeps sibling names aligned
    }
    row.appendChild(toggle)

    const name = document.createElement('span')
    name.className = 'qme-name'
    name.textContent = e.name    // textContent, never innerHTML — see header
    row.appendChild(name)

    if (e.mat) {
      const dims = document.createElement('span')
      dims.className = 'qme-dims'
      dims.textContent = `${e.mat.H}×${e.mat.W}`
      row.appendChild(dims)
    }

    const eye = document.createElement('button')
    eye.className = 'qme-eye'
    eye.type = 'button'
    eye.textContent = deps.visibility.isHidden(e.path) ? '–' : '👁'
    eye.title = 'toggle visibility'
    eye.addEventListener('click', ev => {
      ev.stopPropagation()             // visibility must not disturb selection
      deps.visibility.toggle(e.path)   // its onChange refreshes the panel
    })
    row.appendChild(eye)

    row.addEventListener('click', ev => {
      deps.selection.selectEntity(e.path, ev.shiftKey ? 'toggle' : 'set')
    })
    row.addEventListener('dblclick', () => deps.focusEntity(e.path))

    row_by_path.set(e.path, row)
    return row
  }

  function refresh() {
    row_by_path.clear()
    rows.textContent = ''
    const tree = deps.getTree()
    if (!tree) return
    const needle = filter.value.trim().toLowerCase()
    const keep = needle ? matchSet(tree, needle) : null
    const emit = (e: SceneEntity) => {
      if (keep && !keep.has(e.path)) return
      if (!seen.has(e.path)) {
        seen.add(e.path)
        if (e.children.length && e.depth < 2) expanded.add(e.path)
      }
      rows.appendChild(buildRow(e, keep !== null))
      // While filtering, everything kept is shown expanded — a match hidden
      // under a collapsed ancestor would look like no match at all.
      if (keep !== null || expanded.has(e.path)) e.children.forEach(emit)
    }
    emit(tree.root)
  }

  function revealPath(path: string) {
    const tree = deps.getTree()
    const e = tree ? tree.get(path) : null
    if (!e) return
    for (let q = e.parent; q; q = q.parent) expanded.add(q.path)
    refresh()
    const row = row_by_path.get(path)
    if (!row) return                   // filtered out; the expansion still took
    row.classList.add('qme-reveal')
    // jsdom has no scrollIntoView; the guard is the difference between a
    // module the tests can exercise and one they cannot.
    if (typeof row.scrollIntoView === 'function') row.scrollIntoView({ block: 'nearest' })
  }

  const unsub_sel = deps.selection.onChange(() => refresh())
  const unsub_vis = deps.visibility.onChange(() => refresh())

  refresh()

  return {
    root,
    refresh,
    revealPath,
    dispose() {
      unsub_sel()
      unsub_vis()
      root.remove()   // the <style> stays: it is per-document, not per-panel
    },
  }
}
