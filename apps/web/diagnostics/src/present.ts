/**
 * What goes on the page for a given read.
 *
 * Extracted from `main.ts` so that it can be tested. The entry point is a
 * handful of lines that find the DOM nodes and hand them here; every path that
 * writes the page goes through this module, and `present.test.ts` drives all of
 * them against a fake screen.
 *
 * The rule this module exists to enforce: **a refusal never sits underneath a
 * picture.** The heat-map canvas is painted from one request; if the next read
 * is refused, the canvas still holds the previous manifest and looks current.
 * `index.html` puts that canvas above the text, so the reader's eye reaches a
 * live-looking map before it reaches "Nothing was rendered." Every refusal path
 * here therefore wipes the canvas before the refusal is written.
 */

import { buildSurface, type Surface } from './app.js'
import type { Read, Manifest } from './manifest-client.js'
import { paintHeatmap, refusalToSvg, surfaceToSvg, type Palette } from './render.js'

/** The element the surface SVG is written into. Satisfied by `HTMLElement`. */
export type RootTarget = { innerHTML: string; textContent: string | null }

/** The heat-map canvas. Satisfied by `HTMLCanvasElement`. */
export type CanvasTarget = {
  readonly width: number
  readonly height: number
  hidden: boolean
  getContext(contextId: '2d'): CanvasRenderingContext2D | null
}

/** The two nodes this page owns. `canvas` is `null` when the page has none. */
export type Screen = { root: RootTarget; canvas: CanvasTarget | null }

/** Query-string options. Nothing here reads the DOM. */
export type PageOptions = { runId: string | null; palette: Palette; daemon: string }

export const DEFAULT_DAEMON = 'http://127.0.0.1:8787'

export const NO_RUN_SELECTED =
  'No run selected. Open this page with ?run=<runId>; run selection against the daemon is QM-0152.'

export function readOptions(search: string): PageOptions {
  const params = new URLSearchParams(search)
  return {
    runId: params.get('run'),
    palette: params.get('palette') === 'greyscale' ? 'greyscale' : 'colour',
    daemon: params.get('daemon') ?? DEFAULT_DAEMON,
  }
}

/**
 * Erase the map and take its frame off the page.
 *
 * Called before anything that is not a heat-map is written, so that no pixel of
 * a previous request survives into a screen that does not describe it. Erasing
 * alone is not enough: an emptied 960 × 320 bordered box still reads as a
 * panel, so the canvas is hidden too.
 */
export function clearMap(canvas: CanvasTarget | null): void {
  if (canvas === null) return
  canvas.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height)
  canvas.hidden = true
}

/** Paint the surface onto the canvas, if the page has one and there is anything to paint. */
function paintMap(canvas: CanvasTarget | null, surface: Surface, palette: Palette): void {
  if (canvas === null) return
  const context = canvas.getContext('2d')
  if (context === null) return
  paintHeatmap(context, surface, { palette, width: canvas.width, height: canvas.height })
  // A surface with no cells paints nothing; showing its empty frame above the
  // explanation would be a panel with nothing in it.
  canvas.hidden = surface.heatmap.grid.cellCount === 0
}

/** Show a read: the surface when it read, the refusal when it did not. */
export function present(screen: Screen, read: Read<Manifest>, palette: Palette): Surface | null {
  if (!read.ok) {
    // Before the refusal is written, never after: a stale map must not be on
    // screen beside "Nothing was rendered." for even one frame.
    clearMap(screen.canvas)
    screen.root.innerHTML = refusalToSvg(read.refusal)
    return null
  }
  const surface = buildSurface(read.value)
  screen.root.innerHTML = surfaceToSvg(surface, { palette })
  paintMap(screen.canvas, surface, palette)
  return surface
}

/** No run in the query string: nothing was requested, so nothing is drawn. */
export function presentNoRun(screen: Screen): void {
  clearMap(screen.canvas)
  screen.root.textContent = NO_RUN_SELECTED
}
