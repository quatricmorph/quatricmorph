import { describe, expect, it } from 'vitest'
import {
  MAGNITUDE_TIERS,
  MAX_HEATMAP_CELLS,
  aggregationFactorFor,
  buildGrid,
  encodeMagnitude,
  greyscaleOf,
  maxColumnsPerRow,
  planRow,
  relativeLuminance,
  uniformBands,
  type Band,
  type RowInput,
} from '../heatmap.js'

// QM-0150 — grid geometry and magnitude encoding. Pure functions; no DOM, no
// canvas, no network. The cell ceiling is the browser-side analogue of the block
// ceiling `GRID-005` already enforces: above it the surface aggregates and says
// so, and it never truncates.

/** `rowCount` rows of `channelCount` unit-wide bands, values ascending by column. */
function uniformRows(rowCount: number, channelCount: number): RowInput[] {
  const rows: RowInput[] = []
  for (let layerIndex = 0; layerIndex < rowCount; layerIndex += 1) {
    rows.push({
      layerIndex,
      label: `layer ${layerIndex}`,
      bands: uniformBands(channelCount, (i) => (i + 1) / channelCount, `layer ${layerIndex}`),
    })
  }
  return rows
}

describe('QM-0150 the cell ceiling', () => {
  it('the_default_ceiling_is_the_two_hundred_and_fifty_thousand_cells_the_task_specifies', () => {
    expect(MAX_HEATMAP_CELLS).toBe(250_000)
  })

  it('the_columns_a_row_may_have_is_the_ceiling_divided_by_the_row_count', () => {
    expect(maxColumnsPerRow(12)).toBe(Math.floor(250_000 / 12))
    expect(maxColumnsPerRow(100)).toBe(2500)
    expect(maxColumnsPerRow(1000)).toBe(250)
  })

  it('exactly_as_many_rows_as_the_ceiling_leaves_one_column_each', () => {
    expect(maxColumnsPerRow(MAX_HEATMAP_CELLS)).toBe(1)
  })

  it('more_rows_than_the_ceiling_is_refused_rather_than_silently_dropping_rows', () => {
    expect(() => maxColumnsPerRow(MAX_HEATMAP_CELLS + 1)).toThrow(/ceiling/)
  })

  it('the_aggregation_factor_is_one_when_the_bands_already_fit', () => {
    expect(aggregationFactorFor(512, 20_833)).toBe(1)
    expect(aggregationFactorFor(1, 1)).toBe(1)
  })

  it('the_aggregation_factor_is_the_ceiling_of_bands_over_columns_when_they_do_not', () => {
    // Hand-computed: ceil(8192 / 2500) = 4; ceil(65536 / 250) = 263.
    expect(aggregationFactorFor(8192, 2500)).toBe(4)
    expect(aggregationFactorFor(65_536, 250)).toBe(263)
  })
})

describe('QM-0150 grid geometry at the sizes the task names', () => {
  it('twelve_layers_of_five_hundred_and_twelve_channels_is_six_thousand_one_hundred_and_forty_four_cells_unaggregated', () => {
    // Hand-computed: 12 * 512 = 6144, which is below the ceiling, so nothing
    // aggregates and every cell covers exactly one channel.
    const grid = buildGrid(uniformRows(12, 512), 'exact')
    expect(grid.rows).toHaveLength(12)
    expect(grid.rows.every((row) => row.cells.length === 512)).toBe(true)
    expect(grid.cellCount).toBe(6144)
    expect(grid.aggregationFactor).toBe(1)
    expect(grid.anyAggregated).toBe(false)
    expect(grid.rows[0].cells.every((cell) => cell.aggregated === false)).toBe(true)
    expect(grid.rows[0].cells.every((cell) => cell.channelsPerCell === 1)).toBe(true)
  })

  it('one_hundred_layers_of_eight_thousand_one_hundred_and_ninety_two_channels_aggregates_and_labels_every_cell', () => {
    // Hand-computed: 100 * 8192 = 819 200 > 250 000. Columns per row = 2500,
    // factor = ceil(8192 / 2500) = 4, columns = ceil(8192 / 4) = 2048,
    // cells = 100 * 2048 = 204 800.
    const grid = buildGrid(uniformRows(100, 8192), 'exact')
    expect(grid.aggregationFactor).toBe(4)
    expect(grid.rows[0].cells).toHaveLength(2048)
    expect(grid.cellCount).toBe(204_800)
    expect(grid.cellCount).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)
    expect(grid.anyAggregated).toBe(true)
    expect(grid.rows[0].cells.every((cell) => cell.aggregated === true)).toBe(true)
    expect(grid.rows[0].cells.every((cell) => cell.channelsPerCell === 4)).toBe(true)
  })

  it('a_thousand_layers_of_sixty_five_thousand_channels_still_stays_inside_the_ceiling', () => {
    // Hand-computed: columns per row = 250, factor = ceil(65 536 / 250) = 263,
    // columns = ceil(65 536 / 263) = 250, cells = 250 000.
    //
    // 65.5 million bands are visited to take 250 000 maxima, which is why this
    // test carries its own timeout. Aggregating by maximum cannot skip a value
    // without risking the one channel the reader came for.
    const grid = buildGrid(uniformRows(1000, 65_536), 'exact')
    expect(grid.aggregationFactor).toBe(263)
    expect(grid.rows[0].cells).toHaveLength(250)
    expect(grid.cellCount).toBe(250_000)
    expect(grid.cellCount).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)
  }, 30_000)

  it('a_layer_with_a_single_channel_renders_as_one_unaggregated_cell_not_a_blank', () => {
    const grid = buildGrid(uniformRows(3, 1), 'exact')
    expect(grid.cellCount).toBe(3)
    expect(grid.rows[0].cells).toHaveLength(1)
    expect(grid.rows[0].cells[0].aggregated).toBe(false)
    expect(grid.anyAggregated).toBe(false)
  })

  it('no_grid_at_any_dimension_exceeds_the_cell_ceiling', () => {
    // Shapes chosen to cross every branch: below the ceiling, far above it,
    // many narrow rows, and few very wide ones. The 1 000 x 65 536 case has its
    // own test above; repeating it here only costs seconds.
    for (const [rows, channels] of [
      [1, 1],
      [12, 512],
      [100, 8192],
      [2048, 2048],
      [20_000, 4],
    ] as const) {
      const grid = buildGrid(uniformRows(rows, channels), 'exact')
      expect(
        grid.cellCount,
        `${rows} rows x ${channels} channels produced ${grid.cellCount} cells`,
      ).toBeLessThanOrEqual(MAX_HEATMAP_CELLS)
    }
  })
})

describe('QM-0150 aggregation never truncates', () => {
  it('every_channel_index_is_covered_by_exactly_one_cell_at_every_aggregation_factor', () => {
    for (const [rows, channels] of [
      [12, 512],
      [100, 8192],
      [7, 1000],
      [3, 65_536],
    ] as const) {
      const grid = buildGrid(uniformRows(rows, channels), 'exact')
      for (const row of grid.rows) {
        // Reduced to four assertions per row rather than four per cell: this
        // loop walks a million cells at the larger sizes, and an assertion
        // library is not a hot path.
        let covered = 0
        let cursor = 0
        let contiguous = true
        let extentsPublished = true
        for (const cell of row.cells) {
          if (cell.channelStart === null || cell.channelEnd === null) {
            // Unit bands always publish an extent; a null here is a bug, not
            // the honest "not published" of a summary manifest.
            extentsPublished = false
            break
          }
          if (cell.channelStart !== cursor) contiguous = false
          covered += cell.channelEnd - cell.channelStart
          cursor = cell.channelEnd
        }
        expect(extentsPublished, `${rows}x${channels} row ${row.layerIndex} lost its extents`).toBe(true)
        expect(contiguous, `${rows}x${channels} row ${row.layerIndex} has a gap or an overlap`).toBe(true)
        expect(cursor, `${rows}x${channels} row ${row.layerIndex} stops at ${cursor}`).toBe(channels)
        expect(covered, `${rows}x${channels} row ${row.layerIndex} covered ${covered}`).toBe(channels)
      }
    }
  })

  it('the_last_cell_of_a_row_that_does_not_divide_evenly_is_narrower_not_missing', () => {
    // 10 bands, at most 3 columns: factor = ceil(10 / 3) = 4, columns = 3,
    // widths 4, 4, 2 — hand-counted.
    const row = planRow(
      { layerIndex: 0, label: 'layer 0', bands: uniformBands(10, () => 1, 'x') },
      3,
      'exact',
    )
    expect(row.cells.map((c) => (c.channelEnd ?? 0) - (c.channelStart ?? 0))).toEqual([4, 4, 2])
    expect(row.cells[2].channelEnd).toBe(10)
  })

  it('aggregation_is_by_maximum_so_a_single_bad_channel_is_not_averaged_away', () => {
    // A mean of [0.01, 0.01, 0.01, 0.9] is 0.2325 and looks unremarkable; the
    // maximum is 0.9, which is the finding the reader opened the tool for.
    const bands: Band[] = [
      { channelStart: 0, channelEnd: 1, relativeError: 0.01, source: 'a' },
      { channelStart: 1, channelEnd: 2, relativeError: 0.01, source: 'b' },
      { channelStart: 2, channelEnd: 3, relativeError: 0.01, source: 'c' },
      { channelStart: 3, channelEnd: 4, relativeError: 0.9, source: 'd' },
    ]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 1, 'exact')
    expect(row.cells).toHaveLength(1)
    expect(row.cells[0].relativeError).toBe(0.9)
    expect(row.cells[0].bandsPerCell).toBe(4)
    expect(row.cells[0].sources).toEqual(['a', 'b', 'c', 'd'])
  })

  it('an_undefined_band_does_not_pull_an_aggregate_down_to_zero', () => {
    // null is "not measured", not "measured as zero". Max over
    // [null, 0.4] is 0.4, not 0.
    const bands: Band[] = [
      { channelStart: 0, channelEnd: 1, relativeError: null, source: 'a' },
      { channelStart: 1, channelEnd: 2, relativeError: 0.4, source: 'b' },
    ]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 1, 'exact')
    expect(row.cells[0].relativeError).toBe(0.4)
  })

  it('a_cell_whose_bands_are_all_undefined_stays_undefined_rather_than_becoming_zero', () => {
    const bands: Band[] = [
      { channelStart: 0, channelEnd: 1, relativeError: null, source: 'a' },
      { channelStart: 1, channelEnd: 2, relativeError: null, source: 'b' },
    ]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 1, 'exact')
    expect(row.cells[0].relativeError).toBeNull()
  })

  it('bands_of_unequal_width_keep_their_own_channel_extents_when_merged', () => {
    // Manifest v1 publishes one number per tensor, and tensors have different
    // output-channel counts; a merged cell must report the union, not a guess.
    const bands: Band[] = [
      { channelStart: 0, channelEnd: 4, relativeError: 0.2, source: 'q' },
      { channelStart: 4, channelEnd: 10, relativeError: 0.4, source: 'up' },
    ]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 1, 'exact')
    expect(row.cells[0].channelStart).toBe(0)
    expect(row.cells[0].channelEnd).toBe(10)
    expect(row.cells[0].channelsPerCell).toBe(10)
    expect(row.cells[0].relativeError).toBe(0.4)
  })

  it('a_cell_covering_more_than_one_channel_is_labelled_aggregated_even_at_factor_one', () => {
    // One band, six channels wide, no merging: still an aggregate over six
    // channels, and it must say so.
    const bands: Band[] = [{ channelStart: 0, channelEnd: 6, relativeError: 0.3, source: 'up' }]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 10, 'exact')
    expect(row.aggregationFactor).toBe(1)
    expect(row.cells[0].aggregated).toBe(true)
    expect(row.cells[0].channelsPerCell).toBe(6)
  })

  it('a_cell_whose_channel_extent_is_unknown_is_labelled_aggregated_rather_than_claiming_one_channel', () => {
    // A summary manifest publishes one number per layer and no shape. The
    // honest answer to "how many channels does this cover" is "not published".
    const bands: Band[] = [{ channelStart: null, channelEnd: null, relativeError: 0.3, source: 'layer 0' }]
    const row = planRow({ layerIndex: 0, label: 'layer 0', bands }, 10, 'exact')
    expect(row.cells[0].channelsPerCell).toBeNull()
    expect(row.cells[0].aggregated).toBe(true)
  })

  it('a_row_with_no_bands_produces_no_cells_and_no_invented_ones', () => {
    const row = planRow({ layerIndex: 4, label: 'layer 4', bands: [] }, 10, 'exact')
    expect(row.cells).toEqual([])
    expect(row.channelCount).toBe(0)
  })

  it('an_empty_grid_reports_zero_cells_and_a_null_domain_rather_than_a_fabricated_range', () => {
    const grid = buildGrid([], 'exact')
    expect(grid.rows).toEqual([])
    expect(grid.cellCount).toBe(0)
    expect(grid.domain).toBeNull()
  })

  it('the_fidelity_the_manifest_declares_is_carried_onto_every_cell', () => {
    for (const fidelity of ['exact', 'sampled', 'approximate'] as const) {
      const grid = buildGrid(uniformRows(2, 4), fidelity)
      expect(grid.fidelity).toBe(fidelity)
      expect(grid.rows.every((row) => row.cells.every((cell) => cell.fidelity === fidelity))).toBe(true)
    }
  })

  it('the_domain_is_taken_from_defined_values_only', () => {
    const rows: RowInput[] = [
      {
        layerIndex: 0,
        label: 'layer 0',
        bands: [
          { channelStart: 0, channelEnd: 1, relativeError: null, source: 'a' },
          { channelStart: 1, channelEnd: 2, relativeError: 0.25, source: 'b' },
          { channelStart: 2, channelEnd: 3, relativeError: 0.75, source: 'c' },
        ],
      },
    ]
    const grid = buildGrid(rows, 'exact')
    expect(grid.domain).toEqual({ min: 0.25, max: 0.75 })
    expect(grid.undefinedCellCount).toBe(1)
  })
})

describe('QM-0150 magnitude survives greyscale', () => {
  const domain = { min: 0, max: 1 }

  it('there_are_six_ordered_magnitude_tiers', () => {
    expect(MAGNITUDE_TIERS).toBe(6)
  })

  it('the_colour_ramp_is_strictly_monotonic_in_luminance_so_it_survives_greyscale', () => {
    const luminances = Array.from({ length: MAGNITUDE_TIERS }, (_, tier) =>
      relativeLuminance(encodeMagnitude(tier / (MAGNITUDE_TIERS - 1), domain).colour),
    )
    for (let i = 1; i < luminances.length; i += 1) {
      expect(
        luminances[i],
        `tier ${i} is not darker than tier ${i - 1}: ${luminances[i]} vs ${luminances[i - 1]}`,
      ).toBeLessThan(luminances[i - 1])
    }
  })

  it('consecutive_tiers_differ_in_luminance_by_enough_to_be_told_apart_in_print', () => {
    const luminances = Array.from({ length: MAGNITUDE_TIERS }, (_, tier) =>
      relativeLuminance(encodeMagnitude(tier / (MAGNITUDE_TIERS - 1), domain).colour),
    )
    for (let i = 1; i < luminances.length; i += 1) {
      expect(luminances[i - 1] - luminances[i]).toBeGreaterThanOrEqual(0.1)
    }
  })

  it('the_greyscale_rendering_preserves_the_order_of_the_colour_rendering', () => {
    const values = [0, 0.2, 0.4, 0.6, 0.8, 1]
    const colour = values.map((v) => relativeLuminance(encodeMagnitude(v, domain).colour))
    const grey = values.map((v) => relativeLuminance(encodeMagnitude(v, domain).greyscale))
    const order = (xs: number[]) => xs.map((_, i) => i).sort((a, b) => xs[a] - xs[b])
    expect(order(grey)).toEqual(order(colour))
  })

  it('magnitude_is_recoverable_from_the_fill_fraction_alone_without_any_colour', () => {
    // The redundant channel. If a reader sees no colour at all — greyscale
    // print, or a colour-vision difference — the ranking must still be there.
    const values = [0.05, 0.5, 0.95, 0.2]
    const byFill = values
      .map((v, i) => ({ i, fill: encodeMagnitude(v, domain).fillFraction }))
      .sort((a, b) => a.fill - b.fill)
      .map((e) => e.i)
    const byValue = values.map((_, i) => i).sort((a, b) => values[a] - values[b])
    expect(byFill).toEqual(byValue)
  })

  it('the_fill_fraction_is_monotonic_and_bounded_to_the_unit_interval', () => {
    let previous = -1
    for (let i = 0; i <= 20; i += 1) {
      const { fillFraction } = encodeMagnitude(i / 20, domain)
      expect(fillFraction).toBeGreaterThanOrEqual(0)
      expect(fillFraction).toBeLessThanOrEqual(1)
      expect(fillFraction).toBeGreaterThanOrEqual(previous)
      previous = fillFraction
    }
  })

  it('the_glyph_is_a_third_ordered_channel_so_magnitude_survives_a_monochrome_terminal', () => {
    const glyphs = Array.from({ length: MAGNITUDE_TIERS }, (_, tier) =>
      encodeMagnitude(tier / (MAGNITUDE_TIERS - 1), domain).glyph,
    )
    expect(new Set(glyphs).size).toBe(MAGNITUDE_TIERS)
  })

  it('a_degenerate_domain_where_every_value_is_equal_does_not_divide_by_zero', () => {
    const flat = encodeMagnitude(0.3, { min: 0.3, max: 0.3 })
    expect(Number.isFinite(flat.fillFraction)).toBe(true)
    expect(Number.isNaN(flat.fillFraction)).toBe(false)
    expect(flat.defined).toBe(true)
  })

  it('a_single_valued_map_is_not_painted_as_though_every_cell_were_the_worst', () => {
    // `span === 0` used to fall to `normalised = 1`: every cell at the darkest
    // tier, and a six-entry legend every line of which read "fill 100%". A map
    // with no spread ranks nothing, and must not read as maximally bad.
    const flat = encodeMagnitude(0.2, { min: 0.2, max: 0.2 })
    const worst = encodeMagnitude(1, { min: 0, max: 1 })
    expect(flat.uniform).toBe(true)
    expect(flat.tier).toBeNull()
    expect(flat.normalised, 'a value with no range has no position in one').toBeNull()
    expect(flat.fillFraction, 'a bar drawn to any height claims a rank there is not').toBe(0)
    expect(flat.colour).not.toBe(worst.colour)
    expect(flat.glyph).not.toBe(worst.glyph)
    expect(relativeLuminance(flat.greyscale)).toBeGreaterThan(relativeLuminance(worst.greyscale))
  })

  it('a_domain_where_every_value_is_zero_is_single_valued_rather_than_the_lowest_of_a_range', () => {
    // There is no range here for a value to be the lowest of. The previous
    // encoding said "tier 1 of 6", which is a rank taken from one sample.
    const flat = encodeMagnitude(0, { min: 0, max: 0 })
    expect(flat.uniform).toBe(true)
    expect(flat.tier).toBeNull()
    expect(flat.colour).toBe(encodeMagnitude(0.2, { min: 0.2, max: 0.2 }).colour)
  })

  it('a_single_valued_cell_is_not_confusable_with_an_unmeasured_one', () => {
    // "Every cell measured the same" and "nothing was measured" are different
    // statements, and must not be drawn the same way.
    const flat = encodeMagnitude(0.2, { min: 0.2, max: 0.2 })
    const missing = encodeMagnitude(null, { min: 0.2, max: 0.2 })
    expect(flat.defined).toBe(true)
    expect(missing.defined).toBe(false)
    expect(flat.colour).not.toBe(missing.colour)
    expect(flat.glyph).not.toBe(missing.glyph)
    expect(
      Math.abs(relativeLuminance(flat.greyscale) - relativeLuminance(missing.greyscale)),
      'the two are indistinguishable in greyscale',
    ).toBeGreaterThan(0.05)
  })

  it('the_single_valued_fill_is_distinguishable_in_greyscale_from_every_magnitude_tier', () => {
    // Otherwise a printed map cannot be told from a ranked one.
    const flat = encodeMagnitude(0.2, { min: 0.2, max: 0.2 })
    for (let tier = 0; tier < MAGNITUDE_TIERS; tier += 1) {
      const ramp = encodeMagnitude(tier / (MAGNITUDE_TIERS - 1), { min: 0, max: 1 })
      expect(
        Math.abs(relativeLuminance(flat.greyscale) - relativeLuminance(ramp.greyscale)),
        `the single-valued fill matches tier ${tier + 1} in greyscale`,
      ).toBeGreaterThan(0.05)
    }
  })

  it('a_value_inside_a_real_range_is_still_ranked_so_the_single_valued_case_is_not_over_eager', () => {
    const ranked = encodeMagnitude(0.5, { min: 0, max: 1 })
    expect(ranked.uniform).toBe(false)
    expect(ranked.tier).not.toBeNull()
    expect(ranked.normalised).toBeCloseTo(0.5, 12)
  })

  it('an_undefined_value_is_not_encoded_as_the_lowest_magnitude', () => {
    // "Not measured" must never look like "measured, and it was the best".
    const undefinedCell = encodeMagnitude(null, domain)
    const lowest = encodeMagnitude(0, domain)
    expect(undefinedCell.defined).toBe(false)
    expect(undefinedCell.tier).toBeNull()
    expect(undefinedCell.colour).not.toBe(lowest.colour)
    expect(undefinedCell.glyph).not.toBe(lowest.glyph)
  })

  it('an_undefined_value_has_no_fill_and_no_normalised_position', () => {
    const undefinedCell = encodeMagnitude(null, domain)
    expect(undefinedCell.normalised).toBeNull()
    expect(undefinedCell.fillFraction).toBe(0)
  })

  it('a_null_domain_leaves_every_value_unencodable_rather_than_inventing_a_range', () => {
    expect(encodeMagnitude(0.5, null).defined).toBe(false)
  })

  it('the_greyscale_helper_returns_an_equal_channel_colour', () => {
    const grey = greyscaleOf('#e31a1c')
    expect(/^#([0-9a-f]{2})\1\1$/.test(grey), `${grey} is not a grey`).toBe(true)
  })

  it('relative_luminance_matches_the_published_endpoints_of_the_srgb_range', () => {
    // Hand-checked against the WCAG definition: white is 1, black is 0.
    expect(relativeLuminance('#ffffff')).toBeCloseTo(1, 6)
    expect(relativeLuminance('#000000')).toBeCloseTo(0, 6)
  })
})
