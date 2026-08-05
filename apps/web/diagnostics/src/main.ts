/**
 * Browser entry point.
 *
 * Deliberately thin: it finds the two DOM nodes, reads the query string and
 * hands both to `present.ts`. It renders nothing itself — no `surfaceToSvg`, no
 * `paintHeatmap`, no `innerHTML` — because this file needs a browser and
 * therefore has no test of its own. `present.test.ts` asserts that property
 * over this file's source, so the untested file cannot draw a screen that the
 * tested one has never been asked about.
 *
 * The one behavioural rule enforced here: the initial load asks for the summary
 * projection, and per-layer detail is requested only from a click handler.
 * Nothing about the view state triggers a fetch.
 */

import { HttpTransport, loadLayerDetail, loadSummary } from './manifest-client.js'
import { present, presentNoRun, readOptions, type Screen } from './present.js'

function mount(): { screen: Screen; root: HTMLElement } {
  const root = window.document.querySelector<HTMLElement>('#diagnostics')
  if (root === null) throw new Error('#diagnostics is missing from the page')
  const canvas = window.document.querySelector<HTMLCanvasElement>('#heatmap-canvas')
  return { screen: { root, canvas }, root }
}

async function start(): Promise<void> {
  const { screen, root } = mount()
  const { runId, palette, daemon } = readOptions(window.location.search)
  const transport = new HttpTransport(daemon)

  if (runId === null) {
    presentNoRun(screen)
    return
  }

  // Exactly one request: the summary projection.
  const surface = present(screen, await loadSummary(transport, runId), palette)
  if (surface === null) return

  // Detail is requested only from an explicit act, never from view state.
  root.addEventListener('click', (event) => {
    const target = event.target
    if (!(target instanceof Element)) return
    const layer = target.getAttribute('data-layer')
    if (layer === null || layer === 'none') return
    void loadLayerDetail(transport, runId, Number.parseInt(layer, 10)).then((detail) => {
      present(screen, detail, palette)
    })
  })
}

void start()
