import { describe, expect, it } from 'vitest'
import { buildSurface, forbiddenClaimTermsIn, surfaceStrings } from '../app.js'
import {
  MAX_HEATMAP_CELLS,
  buildGrid,
  cellFidelityOf,
  maxColumnsPerRow,
  planRow,
  uniformBands,
  type Band,
  type Cell,
  type CellFidelity,
  type RowInput,
} from '../heatmap.js'
import { readManifest, type Manifest } from '../manifest-client.js'
import { CELL_RECT_MARKER, SAMPLED_MARK_MARKER, surfaceToSvg, svgTextContent } from '../render.js'
import { fixtureText } from './fixtures.js'

// QM-0153 — the rendering ceiling and labelled degradation.
//
// QM-0150 built the ceiling; this file is about what the surface *says* when it
// hits it, and about the one failure mode that would make the picture
// confidently wrong: truncation. A truncated heat-map looks exactly like an
// honest one — same rows, same colours, same legend — while silently omitting
// the channels the reader came to find. So the coverage assertions below are
// written to fail if a channel ever stops being drawn, at the largest
// dimensions the surface admits.
//
// The second subject is the three-state cell fidelity. `sampled` is the
// engine's coarseness (how the number was obtained) and `aggregated` is the
// renderer's (how it is drawn). Conflating them tells a reader the data is
// coarse when only the display is, or the reverse.

function manifestFrom(name: string, projection: 'summary' | 'full' = 'summary'): Manifest {
  const read = readManifest(fixtureText(name), { projection })
  if (!read.ok) throw new Error(`fixture ${name} did not read: ${read.refusal.message}`)
  return read.value
}

/** `rowCount` rows of `channelCount` unit-wide bands. */
function uniformRows(rowCount: number, channelCount: number): RowInput[] {
  const rows: RowInput[] = []
  for (let layerIndex = 0; layerIndex < rowCount; layerIndex += 1) {
    rows.push({
      layerIndex,
      label: `layer ${layerIndex}`,
      bands: uniformBands(channelCount, (i) => (i % 97) / 97, `layer ${layerIndex}`),
    })
  }
  return rows
}

/**
 * How many times each channel index is covered by a row's cells.
 *
 * The whole point of the exercise: a truncating planner leaves zeroes at the
 * tail, and an overlapping one leaves twos. Either is a wrong picture.
 */
function coverageCounts(cells: readonly Cell[], channelCount: number): { min: number; max: number } {
  const counts = new Uint8Array(channelCount)
  for (const cell of cells) {
    if (cell.channelStart === null || cell.channelEnd === null) {
      throw new Error('a unit-band row lost its channel extents; coverage cannot be checked')
    }
    for (let i = cell.channelStart; i < cell.channelEnd; i += 1) counts[i] += 1
  }
  let min = Number.POSITIVE_INFINITY
  let max = 0
  for (let i = 0; i < channelCount; i += 1) {
    if (counts[i] < min) min = counts[i]
    if (counts[i] > max) max = counts[i]
  }
  return { min, max }
}

describe('QM-0153 no truncation path exists', () => {
  it('every_channel_index_maps_into_exactly_one_cell_at_a_thousand_layers_of_sixty_five_thousand_channels', () => {
    // The extreme the task names: 1 000 x 65 536 = 65 536 000 channels against
    // a 250 000-cell ceiling, an aggregation factor of 263.
    //
    // Rows are planned one at a time rather than through `buildGrid` so that
    // only one row's cells are resident at once; `buildGrid` at this size holds
    // 250 000 cells and 65.5 million band references, and this file must not
    // double that peak beside `heatmap.test.ts`'s own extreme-size test. The
    // column budget fed to `planRow` is the same one `buildGrid` computes.
    const rowCount = 1000
    const channelCount = 65_536
    const maxColumns = maxColumnsPerRow(rowCount)
    expect(maxColumns).toBe(250)

    let totalCells = 0
    let worstMin = Number.POSITIVE_INFINITY
    let worstMax = 0
    let bandsSeen = 0

    for (let layerIndex = 0; layerIndex < rowCount; layerIndex += 1) {
      const row = planRow(
        {
          layerIndex,
          label: `layer ${layerIndex}`,
          bands: uniformBands(channelCount, (i) => (i % 97) / 97, `layer ${layerIndex}`),
        },
        maxColumns,
        'exact',
      )
      const { min, max } = coverageCounts(row.cells, channelCount)
      if (min < worstMin) worstMin = min
      if (max > worstMax) worstMax = max
      totalCells += row.cells.length
      for (const cell of row.cells) bandsSeen += cell.bandsPerCell
    }

    // Every channel of every row is covered once: none dropped, none drawn twice.
    expect(worstMin, 'a channel index is covered by no cell — the tail was truncated').toBe(1)
    expect(worstMax, 'a channel index is covered by more than one cell').toBe(1)
    // And no band was skipped on the way, which holds even where extents do not.
    expect(bandsSeen).toBe(rowCount * channelCount)
    // Hand-computed: ceil(65 536 / 263) = 250 columns, 1 000 rows, 250 000 cells.
    expect(totalCells).toBe(250_000)
    expect(totalCells).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)
  }, 120_000)

  it('every_row_of_a_grid_built_at_the_ceiling_covers_every_one_of_its_channels', () => {
    // The same property through `buildGrid`, so the assembly path is covered
    // and not only `planRow`. Hand-computed: 500 rows leave 500 columns each,
    // factor = ceil(4096 / 500) = 9, columns = ceil(4096 / 9) = 456, cells =
    // 500 * 456 = 228 000.
    const rowCount = 500
    const channelCount = 4096
    const grid = buildGrid(uniformRows(rowCount, channelCount), 'exact')
    expect(grid.rows).toHaveLength(rowCount)
    expect(grid.aggregationFactor).toBe(9)
    expect(grid.cellCount).toBe(228_000)
    expect(grid.cellCount).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)

    let worstMin = Number.POSITIVE_INFINITY
    let worstMax = 0
    for (const row of grid.rows) {
      const { min, max } = coverageCounts(row.cells, channelCount)
      if (min < worstMin) worstMin = min
      if (max > worstMax) worstMax = max
    }
    expect(worstMin, 'a channel is missing from the grid').toBe(1)
    expect(worstMax, 'a channel is drawn twice').toBe(1)
  }, 60_000)

  it('no_band_is_dropped_even_when_the_manifest_publishes_no_channel_extent', () => {
    // The coverage check above reads channel extents. A summary manifest
    // publishes none, so a truncation there would be invisible to it — the
    // band count is the invariant that still holds.
    const bands: Band[] = Array.from({ length: 1000 }, (_, i) => ({
      channelStart: null,
      channelEnd: null,
      relativeError: i / 1000,
      source: `layer 0 part ${i}`,
    }))
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 7, 'exact')
    expect(row.cells.reduce((sum, cell) => sum + cell.bandsPerCell, 0)).toBe(1000)
    expect(row.cells.reduce((sum, cell) => sum + cell.sources.length, 0)).toBe(1000)
    // Every source name survives: nothing was merged away into a cell that
    // cannot say where its number came from.
    expect(new Set(row.cells.flatMap((cell) => cell.sources)).size).toBe(1000)
  })

  it('the_cell_count_stays_inside_the_ceiling_at_dimensions_that_divide_badly', () => {
    // Awkward shapes rather than powers of two: an off-by-one in the column
    // arithmetic shows up here and not at 2^n.
    for (const [rows, channels] of [
      [1, 250_001],
      [3, 99_991],
      [7, 65_537],
      [251, 1009],
      [999, 1001],
    ] as const) {
      const grid = buildGrid(uniformRows(rows, channels), 'exact')
      expect(
        grid.cellCount,
        `${rows} rows x ${channels} channels produced ${grid.cellCount} cells`,
      ).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)
      for (const row of grid.rows) {
        const last = row.cells[row.cells.length - 1]
        expect(last.channelEnd, `${rows}x${channels} stops short of its last channel`).toBe(channels)
      }
    }
  })
})

describe('QM-0153 cell fidelity is three states, not two', () => {
  const exactCell = (): Cell =>
    planRow(
      { layerIndex: 0, label: 'layer 0', bands: uniformBands(4, () => 0.5, 'x') },
      10,
      'exact',
    ).cells[0]

  it('a_cell_of_one_exactly_measured_channel_is_exact', () => {
    const fidelity: CellFidelity = cellFidelityOf(exactCell())
    expect(fidelity).toEqual({ kind: 'exact' })
  })

  it('a_merged_cell_reports_aggregated_with_the_channels_it_covers', () => {
    const row = planRow(
      { layerIndex: 0, label: 'layer 0', bands: uniformBands(12, () => 0.5, 'x') },
      3,
      'exact',
    )
    expect(cellFidelityOf(row.cells[0])).toEqual({ kind: 'aggregated', channelsPerCell: 4 })
  })

  it('an_aggregated_cell_with_no_published_extent_says_so_rather_than_claiming_one_channel', () => {
    const bands: Band[] = [
      { channelStart: null, channelEnd: null, relativeError: 0.3, source: 'layer 0' },
    ]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 10, 'exact')
    expect(cellFidelityOf(row.cells[0])).toEqual({ kind: 'aggregated', channelsPerCell: null })
  })

  it('a_cell_carrying_a_sampled_engine_value_is_sampled_and_not_merely_aggregated', () => {
    // The two mean different things. `sampled` is how the number was obtained;
    // `aggregated` is how it is drawn. A sampled number merged across four
    // channels is still, first, a sampled number.
    const row = planRow(
      { layerIndex: 0, label: 'layer 0', bands: uniformBands(12, () => 0.5, 'x') },
      3,
      'sampled',
    )
    expect(cellFidelityOf(row.cells[0]).kind).toBe('sampled')
    // and the aggregation is not lost by the classification
    expect(row.cells[0].aggregated).toBe(true)
    expect(row.cells[0].channelsPerCell).toBe(4)
  })

  it('an_approximate_engine_value_is_never_described_to_the_reader_with_the_word_sampled', () => {
    // `CellFidelity` groups every non-exact engine label under one marker, but
    // the words on the page come from the manifest's own field. Printing
    // "sampled" over an approximate run would be a different claim about how
    // the number was obtained.
    const surface = buildSurface({ ...manifestFrom('summary.v1.json'), fidelity: 'approximate' })
    const svg = surfaceToSvg(surface, { palette: 'greyscale' })
    const words = [...surfaceStrings(surface), ...svgTextContent(svg)].join('\n')
    expect(words).toContain('approximate')
    expect(words, 'an approximate run is labelled sampled').not.toMatch(/\bsampled\b/)
    expect(svg).toContain('data-fidelity="approximate"')
    expect(svg).not.toContain('data-fidelity="sampled"')
  })

  it('an_exact_cell_is_never_marked_as_sampled', () => {
    const svg = surfaceToSvg(buildSurface(manifestFrom('summary.v1.json')), { palette: 'greyscale' })
    expect(svg).toContain('data-cell-fidelity="aggregated"')
    expect(svg).not.toContain('data-cell-fidelity="sampled"')
  })
})

describe('QM-0153 the marks a reader sees without hovering', () => {
  const SAMPLED = buildSurface(manifestFrom('summary.sampled.json'))
  const EXACT = buildSurface(manifestFrom('summary.v1.json'))

  function marks(svg: string): string[] {
    return [...svg.matchAll(new RegExp(`${SAMPLED_MARK_MARKER}[^>]*>`, 'g'))].map((m) => m[0])
  }

  function cells(svg: string): string[] {
    return [...svg.matchAll(new RegExp(`${CELL_RECT_MARKER}[^>]*>`, 'g'))].map((m) => m[0])
  }

  it('a_sampled_cell_carries_a_corner_wedge_and_an_exact_one_does_not', () => {
    const sampled = surfaceToSvg(SAMPLED, { palette: 'greyscale' })
    const exact = surfaceToSvg(EXACT, { palette: 'greyscale' })
    // One wedge per cell, plus the one drawn on the legend swatch that names it.
    expect(marks(sampled)).toHaveLength(SAMPLED.heatmap.grid.cellCount + 1)
    expect(marks(exact)).toHaveLength(0)
  })

  it('the_sampled_wedge_is_a_different_mark_from_the_aggregation_dash', () => {
    // Both are present on the same cells in this fixture — a summary manifest
    // publishes no channel extent, so its cells are aggregates, and its run was
    // sampled. Two facts, two marks; one mark for both would say neither.
    const svg = surfaceToSvg(SAMPLED, { palette: 'greyscale' })
    for (const cell of cells(svg)) {
      expect(cell).toContain('data-aggregated="true"')
      expect(cell).toContain('stroke-dasharray="3 2"')
      expect(cell).toContain('data-cell-fidelity="sampled"')
    }
    expect(marks(svg).length).toBeGreaterThan(0)
    // The wedge is a shape, not a dash: it cannot be mistaken for the border.
    expect(marks(svg)[0]).not.toContain('stroke-dasharray')
  })

  it('the_sampled_wedge_survives_greyscale_because_it_is_a_shape_and_not_a_colour', () => {
    // The mark is drawn identically under both palettes — it carries no hue to
    // lose — and its ink is one of two values chosen to stay legible against
    // the lightest and the darkest fill a cell can have.
    const grey = surfaceToSvg(SAMPLED, { palette: 'greyscale' })
    const colour = surfaceToSvg(SAMPLED, { palette: 'colour' })
    expect(marks(grey)).toEqual(marks(colour))
    expect(marks(grey).length).toBeGreaterThan(0)
    for (const mark of marks(grey)) {
      const fill = /fill="([^"]+)"/.exec(mark)?.[1]
      expect(['#111827', '#ffffff'], `${fill} is a hue the greyscale palette would lose`).toContain(fill)
    }
  })

  it('the_key_carries_the_same_wedge_beside_the_words_that_name_it', () => {
    const svg = surfaceToSvg(SAMPLED, { palette: 'greyscale' })
    const entries = SAMPLED.heatmap.legend.entries
    const sampled = entries.find((entry) => entry.kind === 'engine-coarse')
    expect(sampled, 'the sampled fixture produced no sampled legend entry').toBeDefined()
    expect(sampled?.label).toContain('wedge')
    expect(svgTextContent(svg).some((line) => line.includes(sampled?.label ?? ' '))).toBe(true)
    // The mark drawn on the swatch is the mark drawn on the cells.
    expect(svg).toContain('<path class="sampled-mark" data-in="legend"')
  })

  it('the_sampled_and_aggregated_legend_entries_are_two_entries_and_not_one', () => {
    const kinds = SAMPLED.heatmap.legend.entries.map((entry) => entry.kind)
    expect(kinds).toContain('engine-coarse')
    expect(kinds).toContain('aggregated')
    expect(kinds.filter((kind) => kind === 'engine-coarse')).toHaveLength(1)
  })

  it('the_legend_separates_the_engines_coarseness_from_the_renderers', () => {
    const note = SAMPLED.heatmap.legend.fidelityNote
    expect(note).toContain('sampled')
    expect(note.toLowerCase()).toContain('drawn')
    // Wrapped into several <text> lines in the image, so it is read back joined.
    const drawn = svgTextContent(surfaceToSvg(SAMPLED, { palette: 'colour' })).join(' ')
    expect(drawn).toContain('describes how they were obtained')
  })

  it('no_new_legend_copy_uses_the_forbidden_claim_vocabulary', () => {
    for (const surface of [SAMPLED, EXACT]) {
      for (const line of surfaceStrings(surface)) {
        expect(forbiddenClaimTermsIn(line), `"${line}" makes a claim this surface may not make`).toEqual([])
      }
    }
  })

  it('every_legend_line_still_fits_the_page_after_the_new_entry', () => {
    for (const surface of [SAMPLED, EXACT]) {
      const svg = surfaceToSvg(surface, { palette: 'colour' })
      const width = Number(/^<svg[^>]*width="(\d+)"/.exec(svg)?.[1])
      for (const match of svg.matchAll(/<text x="([0-9.]+)"[^>]*font-size="(\d+)"[^>]*>([^<]*)<\/text>/g)) {
        const [, x, size, content] = match
        const right = Number(x) + content.length * Number(size) * 0.62
        expect(right, `"${content}" ends at ${right} of ${width}`).toBeLessThanOrEqual(width)
      }
    }
  })
})

describe('QM-0153 the legend states the aggregation rule', () => {
  // The full manifest under a ceiling of three cells: three rows, one column
  // each, so layer 0's two tensors merge into one cell — factor 2.
  const MERGED = buildSurface(manifestFrom('full.v1.json', 'full'), { cellCeiling: 3 })

  it('the_aggregation_factor_is_stated_in_the_legend_and_drawn_in_the_image', () => {
    expect(MERGED.heatmap.grid.aggregationFactor).toBe(2)
    expect(MERGED.heatmap.legend.aggregationNote).toContain('2')
    const entry = MERGED.heatmap.legend.entries.find((e) => e.kind === 'aggregated')
    expect(entry?.label).toContain('factor 2')
    expect(surfaceToSvg(MERGED, { palette: 'colour' })).toContain('factor 2')
  })

  it('the_legend_says_the_merge_is_by_maximum_and_says_why_it_is_not_a_mean', () => {
    // A mean would hide the one catastrophic channel inside a healthy group,
    // which is the finding the tool exists to surface. The rule is a product
    // decision, so it is stated where the reader is looking.
    const note = MERGED.heatmap.legend.aggregationNote
    expect(note).toContain('maximum')
    expect(note).toMatch(/\bmean\b/)
    expect(svgTextContent(surfaceToSvg(MERGED, { palette: 'colour' })).join(' ')).toContain('by maximum')
  })

  it('a_merged_cell_takes_the_worst_of_its_columns_and_not_their_average', () => {
    const worst = Math.max(
      ...MERGED.heatmap.grid.rows.flatMap((row) => row.cells.map((cell) => cell.relativeError ?? 0)),
    )
    const unmerged = buildSurface(manifestFrom('full.v1.json', 'full'))
    const worstUnmerged = Math.max(
      ...unmerged.heatmap.grid.rows.flatMap((row) => row.cells.map((cell) => cell.relativeError ?? 0)),
    )
    // Merging changes how many cells are drawn; it never changes the largest
    // number on the map, which a mean would.
    expect(worst).toBe(worstUnmerged)
  })
})

describe('QM-0153 an aggregation factor of one is the unaggregated case', () => {
  it('a_row_that_needed_no_merging_is_identical_to_the_same_row_planned_with_room_to_spare', () => {
    const bands = () => uniformBands(64, (i) => i / 64, 'layer 0')
    const tight = planRow({ layerIndex: 0, label: 'layer 0', bands: bands() }, 64, 'exact')
    const roomy = planRow({ layerIndex: 0, label: 'layer 0', bands: bands() }, 100_000, 'exact')
    expect(tight).toEqual(roomy)
    expect(tight.aggregationFactor).toBe(1)
  })

  it('unit_wide_bands_at_factor_one_carry_no_aggregation_mark_and_no_aggregated_legend_entry', () => {
    // AC6, read against the behaviour QM-0150 fixed: a cell that covers more
    // than one channel is an aggregate whatever the merge factor, so the case
    // that must carry no mark is the one where the renderer merged nothing
    // *and* each band is a single channel.
    const grid = buildGrid(uniformRows(4, 8), 'exact')
    expect(grid.aggregationFactor).toBe(1)
    expect(grid.anyAggregated).toBe(false)
    for (const row of grid.rows) {
      for (const cell of row.cells) {
        expect(cell.aggregated).toBe(false)
        expect(cellFidelityOf(cell)).toEqual({ kind: 'exact' })
      }
    }
  })

  it('a_single_channel_layer_is_drawn_as_a_cell_rather_than_special_cased_into_a_blank', () => {
    const grid = buildGrid(uniformRows(1, 1), 'exact')
    expect(grid.cellCount).toBe(1)
    expect(grid.rows[0].cells[0].aggregated).toBe(false)
    expect(cellFidelityOf(grid.rows[0].cells[0])).toEqual({ kind: 'exact' })
  })
})
