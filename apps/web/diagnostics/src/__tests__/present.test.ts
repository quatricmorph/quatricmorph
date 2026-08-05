import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import {
  DEFAULT_DAEMON,
  NO_RUN_SELECTED,
  present,
  presentNoRun,
  readOptions,
  type CanvasTarget,
  type Screen,
} from '../present.js'
import {
  loadLayerDetail,
  loadSummary,
  readManifest,
  type Manifest,
  type ManifestTransport,
  type Read,
  type TransportResult,
} from '../manifest-client.js'
import { CELL_RECT_MARKER, type PaintOp } from '../render.js'
import { fixtureText } from './fixtures.js'

// QM-0150 — the wiring, which used to be the one file with no tests.
//
// The defect this file exists to prevent: `show()` returned early on a refusal
// without touching the canvas, so the heat-map painted from the PREVIOUS
// request stayed on screen — above the refusal text, because index.html puts
// the canvas first. A picture that looks live beside "Nothing was rendered." is
// the exact failure this package is built to avoid, and it was reachable on the
// first layer click, because the layer route is a declared gap.

const HERE = dirname(fileURLToPath(import.meta.url))
const PACKAGE_ROOT = resolve(HERE, '..', '..')

type FakeScreen = {
  screen: Screen
  /** Every instruction the canvas was given, in order. */
  ops: PaintOp[]
  canvas: CanvasTarget | null
  /** `ops.length` at the moment each write to the page happened. */
  pageWrites: number[]
}

/** A screen that records every drawing instruction and every write to the page. */
function fakeScreen(options: { canvas?: boolean } = {}): FakeScreen {
  const ops: PaintOp[] = []
  const pageWrites: number[] = []
  let fillStyle = ''
  let strokeStyle = ''
  const context = {
    get fillStyle() {
      return fillStyle
    },
    set fillStyle(value: string) {
      fillStyle = value
    },
    get strokeStyle() {
      return strokeStyle
    },
    set strokeStyle(value: string) {
      strokeStyle = value
    },
    lineWidth: 1,
    fillRect(x: number, y: number, w: number, h: number) {
      ops.push({ op: 'fillRect', x, y, w, h, style: fillStyle })
    },
    strokeRect(x: number, y: number, w: number, h: number) {
      ops.push({ op: 'strokeRect', x, y, w, h, style: strokeStyle })
    },
    clearRect(x: number, y: number, w: number, h: number) {
      ops.push({ op: 'clearRect', x, y, w, h, style: '' })
    },
  }

  const canvas: CanvasTarget | null =
    options.canvas === false
      ? null
      : {
          width: 960,
          height: 320,
          hidden: false,
          getContext: () => context as unknown as CanvasRenderingContext2D,
        }

  let html = ''
  let text: string | null = null
  const root = {
    get innerHTML() {
      return html
    },
    set innerHTML(value: string) {
      pageWrites.push(ops.length)
      html = value
    },
    get textContent() {
      return text
    },
    set textContent(value: string | null) {
      pageWrites.push(ops.length)
      text = value
    },
  }

  return { screen: { root, canvas }, ops, canvas, pageWrites }
}

function readFixture(name: string, projection: 'summary' | 'full' = 'summary'): Read<Manifest> {
  return readManifest(fixtureText(name), { projection })
}

/** Exactly what `HttpTransport` does today: no per-layer route exists. */
const DECLARED_GAP_TRANSPORT: ManifestTransport = {
  requestLog: [],
  async fetchSummary(): Promise<TransportResult> {
    return { kind: 'body', text: fixtureText('summary.v1.json') }
  },
  async fetchLayerDetail(): Promise<TransportResult> {
    return { kind: 'declared_gap', requirement: 'QM-0152', message: 'no per-layer route exists yet' }
  },
}

function paintedCells(ops: PaintOp[]): PaintOp[] {
  return ops.filter((op) => op.op === 'fillRect')
}

describe('QM-0150 a refusal never sits underneath a picture', () => {
  it('a_refusal_wipes_the_heat_map_canvas_that_a_previous_read_painted', () => {
    const { screen, ops, canvas } = fakeScreen()

    const surface = present(screen, readFixture('summary.v1.json'), 'colour')
    expect(surface, 'the fixture must read, or this test proves nothing').not.toBeNull()
    expect(paintedCells(ops).length, 'nothing was painted, so nothing could go stale').toBeGreaterThan(0)

    const refused = present(screen, readFixture('version-2.json'), 'colour')
    expect(refused).toBeNull()

    // Everything painted before the refusal must have been wiped: the last
    // instruction the canvas received covers the whole of it and erases.
    const last = ops[ops.length - 1]
    expect(last.op, `the canvas kept ${paintedCells(ops).length} painted rectangles`).toBe('clearRect')
    expect([last.x, last.y, last.w, last.h]).toEqual([0, 0, 960, 320])
    expect(canvas?.hidden, 'the emptied canvas is still shown as a 960x320 frame').toBe(true)
  })

  it('the_first_layer_click_leaves_no_map_behind_it_on_the_default_transport', async () => {
    // The reachable path: the summary reads and paints, and the very first
    // layer click is refused because the layer route is a declared gap.
    const { screen, ops, canvas } = fakeScreen()

    const summary = await loadSummary(DECLARED_GAP_TRANSPORT, 'run-a')
    expect(present(screen, summary, 'colour')).not.toBeNull()
    expect(paintedCells(ops).length).toBeGreaterThan(0)

    const detail = await loadLayerDetail(DECLARED_GAP_TRANSPORT, 'run-a', 1)
    expect(detail.ok).toBe(false)
    expect(present(screen, detail, 'colour')).toBeNull()

    expect(ops[ops.length - 1].op).toBe('clearRect')
    expect(canvas?.hidden).toBe(true)
    expect(screen.root.innerHTML).toContain('Nothing was rendered.')
    expect(screen.root.innerHTML).toContain('QM-0152')
  })

  it('the_canvas_is_wiped_before_the_refusal_is_written_not_after', () => {
    // If the wipe came second, the stale map would sit beside the refusal for
    // as long as the repaint took. The order is the assertion.
    const { screen, ops, pageWrites } = fakeScreen()
    present(screen, readFixture('summary.v1.json'), 'colour')
    const opsBefore = ops.length
    const writesBefore = pageWrites.length

    present(screen, readFixture('version-2.json'), 'colour')

    expect(pageWrites.length, 'the refusal path never wrote the page').toBeGreaterThan(writesBefore)
    const refusalWrittenAt = pageWrites[writesBefore]
    expect(refusalWrittenAt, 'the refusal was written before the canvas was wiped').toBeGreaterThan(
      opsBefore,
    )
  })

  it('the_refusal_that_replaces_a_map_carries_no_heat_map_cell_of_its_own', () => {
    const { screen } = fakeScreen()
    present(screen, readFixture('summary.v1.json'), 'colour')
    expect(screen.root.innerHTML).toContain(CELL_RECT_MARKER)

    present(screen, readFixture('version-2.json'), 'colour')
    expect(screen.root.innerHTML).not.toContain(CELL_RECT_MARKER)
    expect(screen.root.innerHTML).not.toContain('data-fill-fraction')
  })

  it('a_page_with_no_canvas_still_refuses_without_throwing', () => {
    const { screen } = fakeScreen({ canvas: false })
    expect(present(screen, readFixture('version-2.json'), 'colour')).toBeNull()
    expect(screen.root.innerHTML).toContain('Nothing was rendered.')
  })

  it('a_readable_manifest_shows_the_canvas_again_after_a_refusal_hid_it', () => {
    const { screen, canvas } = fakeScreen()
    present(screen, readFixture('version-2.json'), 'colour')
    expect(canvas?.hidden).toBe(true)

    present(screen, readFixture('summary.v1.json'), 'colour')
    expect(canvas?.hidden, 'a readable manifest left its own map hidden').toBe(false)
  })

  it('a_surface_with_nothing_to_paint_hides_the_canvas_rather_than_framing_an_empty_box', () => {
    const { screen, ops, canvas } = fakeScreen()
    const surface = present(screen, readFixture('summary.empty.json'), 'colour')
    expect(surface?.heatmap.grid.cellCount).toBe(0)
    expect(paintedCells(ops)).toHaveLength(0)
    expect(canvas?.hidden).toBe(true)
    expect(screen.root.innerHTML).toContain('nothing to paint')
  })

  it('no_run_selected_draws_no_map_and_says_what_to_do_instead', () => {
    const { screen, ops, canvas } = fakeScreen()
    presentNoRun(screen)
    expect(screen.root.textContent).toBe(NO_RUN_SELECTED)
    expect(paintedCells(ops)).toHaveLength(0)
    expect(canvas?.hidden).toBe(true)
  })
})

describe('QM-0150 the entry point cannot draw a screen of its own', () => {
  it('main_ts_writes_nothing_to_the_page_except_through_the_tested_module', () => {
    // `main.ts` has no test of its own — it needs a browser. It is kept
    // incapable of the defect instead: it may not render, paint or assign
    // innerHTML itself. Every screen it shows comes from `present.ts`, which
    // the tests above drive on every path.
    const source = readFileSync(join(PACKAGE_ROOT, 'src', 'main.ts'), 'utf8')
    // Comments may name these — the file's own docstring explains why it does
    // not call them. Code may not.
    const code = source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '')
    expect(code.length, 'the comment stripper ate the file').toBeGreaterThan(400)
    for (const forbidden of ['surfaceToSvg', 'refusalToSvg', 'paintHeatmap', 'innerHTML', 'getContext']) {
      expect(code.includes(forbidden), `main.ts calls ${forbidden} itself`).toBe(false)
    }
    expect(code).toContain("from './present.js'")
  })

  it('the_pages_own_stylesheet_does_not_defeat_the_hidden_attribute', () => {
    // `hidden` hides an element through the UA stylesheet, and ANY author rule
    // setting `display` on that element overrides it. `index.html` styles
    // `canvas { display: block }`, so without an explicit rule the canvas the
    // refusal path hides would stay on screen — erased, but still a framed
    // 960x320 panel above "Nothing was rendered.". No test here runs a browser,
    // so the cascade is asserted against the stylesheet's own text.
    const html = readFileSync(join(PACKAGE_ROOT, 'index.html'), 'utf8')
    expect(html, 'the canvas is not styled at all; this test is checking the wrong file').toMatch(
      /canvas\s*\{[^}]*display\s*:\s*block/,
    )
    expect(
      html.replace(/\s+/g, ''),
      'an author `display` rule overrides the UA stylesheet, so setting `hidden` would not hide the canvas',
    ).toContain('canvas[hidden]{display:none')
  })

  it('the_query_string_options_are_read_where_they_can_be_tested', () => {
    expect(readOptions('?run=abc')).toEqual({
      runId: 'abc',
      palette: 'colour',
      daemon: DEFAULT_DAEMON,
    })
    expect(readOptions('?run=abc&palette=greyscale').palette).toBe('greyscale')
    expect(readOptions('?run=abc&palette=nonsense').palette).toBe('colour')
    expect(readOptions('?run=abc&daemon=http://host:1').daemon).toBe('http://host:1')
    expect(readOptions('').runId).toBeNull()
  })
})
