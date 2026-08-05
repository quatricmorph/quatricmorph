import { describe, expect, it } from 'vitest'
import { buildSurface } from '../app.js'
import { readManifest, type Manifest, type Refusal } from '../manifest-client.js'
import { relativeLuminance } from '../heatmap.js'
import {
  CELL_RECT_MARKER,
  paintHeatmap,
  refusalToSvg,
  surfaceToSvg,
  type PaintOp,
} from '../render.js'
import { fixtureText } from './fixtures.js'

// QM-0150 — rendering. Two outputs from one draw plan: a 2-D canvas for the
// browser and a deterministic SVG for the evidence artifact, so that what a
// reviewer looks at is produced by the code the browser runs and not by a
// separate drawing made for the screenshot.

function manifestFrom(name: string, projection: 'summary' | 'full' = 'summary'): Manifest {
  const read = readManifest(fixtureText(name), { projection })
  if (!read.ok) throw new Error(`fixture ${name} did not read: ${read.refusal.message}`)
  return read.value
}

const SUMMARY = buildSurface(manifestFrom('summary.v1.json'))
const FULL = buildSurface(manifestFrom('full.v1.json', 'full'))
const SAMPLED = buildSurface(manifestFrom('summary.sampled.json'))
const ZERO_BASE = buildSurface(manifestFrom('summary.zero-base.json'))
const EMPTY = buildSurface(manifestFrom('summary.empty.json'))

function cellFills(svg: string): string[] {
  return [...svg.matchAll(new RegExp(`${CELL_RECT_MARKER}[^>]*?fill="([^"]+)"`, 'g'))].map((m) => m[1])
}

function countCells(svg: string): number {
  return [...svg.matchAll(new RegExp(CELL_RECT_MARKER, 'g'))].length
}

describe('QM-0150 the SVG rendering', () => {
  it('one_cell_is_drawn_for_every_cell_in_the_grid_and_no_more', () => {
    expect(countCells(surfaceToSvg(SUMMARY, { palette: 'colour' }))).toBe(SUMMARY.heatmap.grid.cellCount)
    expect(countCells(surfaceToSvg(FULL, { palette: 'colour' }))).toBe(FULL.heatmap.grid.cellCount)
  })

  it('the_same_surface_renders_byte_identically_twice', () => {
    expect(surfaceToSvg(SUMMARY, { palette: 'colour' })).toBe(surfaceToSvg(SUMMARY, { palette: 'colour' }))
  })

  it('the_fidelity_word_is_drawn_as_text_in_the_image_not_only_carried_in_the_model', () => {
    expect(surfaceToSvg(SAMPLED, { palette: 'colour' })).toContain('sampled')
    expect(surfaceToSvg(SUMMARY, { palette: 'colour' })).toContain('exact')
  })

  it('the_legends_not_a_claim_line_is_drawn_in_the_image', () => {
    const svg = surfaceToSvg(SUMMARY, { palette: 'colour' })
    expect(svg).toContain('A colour is not a finding')
  })

  it('the_frontier_claim_is_drawn_in_the_image', () => {
    expect(surfaceToSvg(SUMMARY, { palette: 'colour' })).toContain('not proven optimal')
  })

  it('every_refusal_id_is_drawn_in_the_image', () => {
    const svg = surfaceToSvg(SUMMARY, { palette: 'colour' })
    expect(svg).toContain('EVAL-001')
    expect(svg).toContain('GRID-007')
  })

  it('text_drawn_into_the_image_is_xml_escaped', () => {
    const surface = buildSurface({
      ...manifestFrom('summary.v1.json'),
      refusals: [{ requirement_id: 'X-1', what: 'a < b & c', why: '"quoted"' }],
    })
    const svg = surfaceToSvg(surface, { palette: 'colour' })
    expect(svg).toContain('a &lt; b &amp; c')
    expect(svg).not.toContain('a < b & c')
  })
})

describe('QM-0150 the rendering survives greyscale', () => {
  it('the_greyscale_palette_emits_no_cell_fill_with_unequal_colour_channels', () => {
    const svg = surfaceToSvg(FULL, { palette: 'greyscale' })
    const fills = cellFills(svg)
    expect(fills.length).toBeGreaterThan(0)
    for (const fill of fills) {
      expect(/^#([0-9a-f]{2})\1\1$/.test(fill), `${fill} is not a grey`).toBe(true)
    }
  })

  it('the_colour_palette_does_emit_colour_so_the_greyscale_check_is_not_vacuous', () => {
    const fills = cellFills(surfaceToSvg(FULL, { palette: 'colour' }))
    expect(fills.some((fill) => !/^#([0-9a-f]{2})\1\1$/.test(fill))).toBe(true)
  })

  it('the_darkest_greyscale_cell_is_the_layer_with_the_largest_relative_error', () => {
    // Fixture: layer 1 is sqrt(25/100) = 0.5, the largest of the three.
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    const fills = cellFills(svg)
    expect(fills).toHaveLength(3)
    const luminances = fills.map(relativeLuminance)
    const darkest = luminances.indexOf(Math.min(...luminances))
    expect(darkest).toBe(1)
  })

  it('the_fill_bar_width_of_each_cell_orders_the_layers_the_same_way_the_colour_does', () => {
    // The redundant channel, read straight out of the image.
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    const bars = [...svg.matchAll(/data-fill-fraction="([0-9.]+)"/g)].map((m) => Number(m[1]))
    expect(bars).toHaveLength(3)
    expect(bars.indexOf(Math.max(...bars))).toBe(1)
  })
})

describe('QM-0150 markers a reader can see without hovering', () => {
  it('an_aggregated_cell_carries_a_persistent_marker_attribute', () => {
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    expect(svg).toContain('data-aggregated="true"')
  })

  it('an_aggregated_cell_is_drawn_with_a_dashed_border_a_reader_can_see_without_hovering', () => {
    // The attribute is for tests; the dash is for the reader. Both, or the
    // aggregation is tracked rather than surfaced.
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    const cells = [...svg.matchAll(/<rect class="cell"[^>]*>/g)].map((m) => m[0])
    expect(cells).toHaveLength(3)
    for (const cell of cells) {
      expect(cell).toContain('data-aggregated="true"')
      expect(cell).toContain('stroke-dasharray="3 2"')
    }
  })

  it('the_legend_names_the_mark_the_renderer_actually_draws_and_puts_that_mark_in_the_key', () => {
    // The legend said "hatched", the renderer drew a dashed border, and the
    // swatch showed neither. A key that describes a mark the reader cannot
    // find is worse than no key: it sends them looking for something absent.
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    const entries = SUMMARY.heatmap.legend.entries
    const index = entries.findIndex((entry) => entry.kind === 'aggregated')
    expect(index, 'the fixture has no aggregated cells, so this proves nothing').toBeGreaterThanOrEqual(0)

    // The mark the cells actually carry.
    const cellDash = /<rect class="cell"[^>]*stroke-dasharray="([^"]+)"/.exec(svg)?.[1]
    expect(cellDash).toBe('3 2')

    // The words name that mark, and name no mark that is never drawn.
    expect(entries[index].label).toContain('dashed border')
    expect(svg, 'the copy names a hatch the renderer never draws').not.toMatch(/hatch/i)
    expect(svg, 'a hatch is claimed but no fill pattern is defined').not.toContain('<pattern')

    // And the key shows it: the swatch beside those words carries the same dash,
    // and no other swatch does.
    const swatches = [...svg.matchAll(/<rect class="swatch"[^>]*>/g)].map((m) => m[0])
    expect(swatches).toHaveLength(entries.length)
    expect(swatches[index]).toContain(`stroke-dasharray="${cellDash}"`)
    expect(swatches.filter((swatch) => swatch.includes(`stroke-dasharray="${cellDash}"`))).toHaveLength(1)
  })

  it('the_magnitude_glyph_is_drawn_inside_each_cell_as_a_third_encoding', () => {
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    // The fixture's three layers land on distinct tiers, so distinct glyphs.
    for (const glyph of ['·', '█']) {
      expect(svg, `glyph ${glyph} is not drawn`).toContain(`>${glyph}</text>`)
    }
  })

  it('every_row_carries_the_fidelity_word_beside_its_label_not_only_in_the_header', () => {
    for (const [surface, word] of [
      [SUMMARY, 'exact'],
      [SAMPLED, 'sampled'],
    ] as const) {
      const svg = surfaceToSvg(surface, { palette: 'greyscale' })
      for (const row of surface.heatmap.grid.rows) {
        expect(svg, `row ${row.label} is unlabelled`).toContain(`${row.label} · ${word}`)
      }
    }
  })

  it('the_legend_says_the_scale_is_relative_to_this_map_and_not_an_absolute_threshold', () => {
    const svg = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    expect(svg).toContain('not an absolute threshold')
    expect(SUMMARY.heatmap.legend.scaleNote).toContain('0.5')
  })

  it('no_row_label_is_drawn_over_by_the_plot', () => {
    // A label the cells cover is a label the reader cannot read — and one of
    // them carries the fidelity of the row beside it.
    for (const surface of [SUMMARY, FULL, SAMPLED, ZERO_BASE]) {
      const svg = surfaceToSvg(surface, { palette: 'colour' })
      const firstCellX = Math.min(
        ...[...svg.matchAll(/<rect class="cell"[^>]*\sx="([0-9.]+)"/g)].map((m) => Number(m[1])),
      )
      for (const row of surface.heatmap.grid.rows) {
        const label = `${row.label} · ${surface.fidelityLabel}`
        const right = 20 + label.length * 12 * 0.62
        expect(right, `"${label}" reaches ${right}, the plot starts at ${firstCellX}`).toBeLessThanOrEqual(
          firstCellX,
        )
      }
    }
  })

  it('no_line_of_copy_runs_off_the_right_edge_of_the_image', () => {
    // Text a reader cannot see is text this surface did not display. The run
    // id and the required caveats are the longest lines and must all fit.
    for (const surface of [SUMMARY, FULL, SAMPLED, EMPTY, ZERO_BASE]) {
      const svg = surfaceToSvg(surface, { palette: 'colour' })
      const width = Number(/^<svg[^>]*width="(\d+)"/.exec(svg)?.[1])
      for (const match of svg.matchAll(/<text x="([0-9.]+)"[^>]*font-size="(\d+)"[^>]*>([^<]*)<\/text>/g)) {
        const [, x, size, content] = match
        const right = Number(x) + content.length * Number(size) * 0.62
        expect(right, `"${content}" ends at ${right} of ${width}`).toBeLessThanOrEqual(width)
      }
    }
  })

  it('every_cell_in_the_tensor_view_covers_more_than_one_channel_and_is_marked_as_such', () => {
    // The fixture's tensors have 16, 6, 4 and 8 output channels; manifest v1
    // publishes one number for each whole tensor, so every cell is an
    // aggregate over its channels and none may claim otherwise.
    const surface = buildSurface(manifestFrom('full.v1.json', 'full'))
    const svg = surfaceToSvg(surface, { palette: 'greyscale' })
    expect(svg).toContain('data-aggregated="true"')
    expect(svg).not.toContain('data-aggregated="false"')
    expect(surface.heatmap.grid.rows.some((r) => r.cells.some((c) => c.aggregated === false))).toBe(false)
  })

  it('an_undefined_cell_is_drawn_with_its_own_marker_and_no_magnitude_fill', () => {
    const svg = surfaceToSvg(ZERO_BASE, { palette: 'greyscale' })
    expect(svg).toContain('data-defined="false"')
    const undefinedBar = /data-defined="false"[^>]*data-fill-fraction="([0-9.]+)"/.exec(svg)
    expect(undefinedBar?.[1]).toBe('0')
  })

  it('a_sampled_cell_is_marked_distinctly_from_an_aggregated_one', () => {
    const svg = surfaceToSvg(SAMPLED, { palette: 'greyscale' })
    expect(svg).toContain('data-fidelity="sampled"')
    expect(svg).toContain('data-aggregated="true"')
  })

  it('selection_is_drawn_with_a_stroke_and_a_marker_rather_than_by_colour_alone', () => {
    const selected = surfaceToSvg(SUMMARY, { palette: 'greyscale', selected: { layerIndex: 1, columnIndex: 0 } })
    const unselected = surfaceToSvg(SUMMARY, { palette: 'greyscale' })
    expect(selected).toContain('data-selected="true"')
    expect(selected).toContain('stroke-width')
    // The selected image must differ from the unselected one in more than fill.
    expect(cellFills(selected)).toEqual(cellFills(unselected))
    expect(selected).not.toBe(unselected)
  })
})

describe('QM-0150 a surface that cannot describe real data refuses to render', () => {
  it('a_refused_manifest_produces_a_refusal_image_with_no_cells_at_all', () => {
    const read = readManifest(fixtureText('version-2.json'), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    const svg = refusalToSvg(read.refusal)
    expect(countCells(svg)).toBe(0)
  })

  it('the_refusal_image_names_both_versions_rather_than_showing_a_placeholder_grid', () => {
    const read = readManifest(fixtureText('version-2.json'), { projection: 'summary' })
    if (read.ok) throw new Error('expected a refusal')
    const svg = refusalToSvg(read.refusal)
    expect(svg).toContain('manifest_version')
    expect(svg).toContain('2')
    expect(svg).toContain('1')
    expect(svg).not.toContain('data-fill-fraction')
  })

  it('every_refusal_kind_renders_a_named_state_rather_than_an_empty_image', () => {
    const refusals: Refusal[] = [
      { kind: 'malformed_json', message: 'unexpected end of JSON input' },
      { kind: 'unsupported_version', found: 7, supported: 1, message: 'manifest_version 7 / 1' },
      { kind: 'schema_invalid', errors: [], message: 'missing refusals' },
      { kind: 'wrong_projection', expected: 'summary', found: 'full', message: 'full on the summary route' },
      { kind: 'payload_too_large', bytes: 9, ceilingBytes: 8, message: 'too large' },
      { kind: 'declared_gap', requirement: 'QM-0152', message: 'not wired' },
      { kind: 'transport_failure', retryable: true, message: 'connection refused' },
    ]
    for (const refusal of refusals) {
      const svg = refusalToSvg(refusal)
      expect(countCells(svg), `${refusal.kind} drew cells`).toBe(0)
      expect(svg).toContain(refusal.kind)
      expect(svg.length).toBeGreaterThan(100)
    }
  })

  it('a_retryable_failure_offers_a_retry_and_a_declared_gap_does_not', () => {
    expect(refusalToSvg({ kind: 'transport_failure', retryable: true, message: 'x' })).toContain('Retry')
    expect(refusalToSvg({ kind: 'declared_gap', requirement: 'QM-0152', message: 'x' })).not.toContain(
      'Retry',
    )
  })

  it('an_empty_run_renders_its_explanation_rather_than_a_grid_of_nothing', () => {
    const svg = surfaceToSvg(EMPTY, { palette: 'colour' })
    expect(countCells(svg)).toBe(0)
    expect(svg).toContain('QUANT-003')
  })
})

describe('QM-0150 the 2-D canvas painter', () => {
  /** Records what a real CanvasRenderingContext2D would have been told to do. */
  function recordingContext(): { ops: PaintOp[]; ctx: Record<string, unknown> } {
    const ops: PaintOp[] = []
    let fillStyle = ''
    let strokeStyle = ''
    const ctx = {
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
    return { ops, ctx: ctx as unknown as Record<string, unknown> }
  }

  it('the_painter_fills_two_rectangles_per_cell_the_cell_and_its_redundant_fill_bar', () => {
    const { ops, ctx } = recordingContext()
    paintHeatmap(ctx as never, SUMMARY, { palette: 'colour', width: 300, height: 120 })
    const fills = ops.filter((o) => o.op === 'fillRect')
    expect(fills).toHaveLength(SUMMARY.heatmap.grid.cellCount * 2)
  })

  it('the_painter_never_draws_outside_the_canvas_it_was_given', () => {
    const { ops, ctx } = recordingContext()
    paintHeatmap(ctx as never, FULL, { palette: 'colour', width: 300, height: 120 })
    for (const op of ops) {
      expect(op.x).toBeGreaterThanOrEqual(0)
      expect(op.y).toBeGreaterThanOrEqual(0)
      expect(op.x + op.w).toBeLessThanOrEqual(300 + 1e-9)
      expect(op.y + op.h).toBeLessThanOrEqual(120 + 1e-9)
    }
  })

  it('the_painter_and_the_svg_choose_the_same_colour_for_the_same_cell', () => {
    // One draw plan, two outputs. If they diverge, the evidence artifact stops
    // being evidence about the browser.
    const { ops, ctx } = recordingContext()
    paintHeatmap(ctx as never, SUMMARY, { palette: 'greyscale', width: 300, height: 120 })
    const painted = ops.filter((o) => o.op === 'fillRect').map((o) => o.style)
    for (const fill of cellFills(surfaceToSvg(SUMMARY, { palette: 'greyscale' }))) {
      expect(painted).toContain(fill)
    }
  })

  it('the_painter_clears_before_it_draws_so_a_repaint_cannot_leave_a_stale_cell_behind', () => {
    const { ops, ctx } = recordingContext()
    paintHeatmap(ctx as never, SUMMARY, { palette: 'colour', width: 300, height: 120 })
    expect(ops[0].op).toBe('clearRect')
  })

  it('an_empty_surface_paints_nothing_rather_than_a_placeholder_grid', () => {
    const { ops, ctx } = recordingContext()
    paintHeatmap(ctx as never, EMPTY, { palette: 'colour', width: 300, height: 120 })
    expect(ops.filter((o) => o.op === 'fillRect')).toHaveLength(0)
  })
})
