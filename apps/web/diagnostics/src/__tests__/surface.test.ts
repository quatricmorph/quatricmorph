import { describe, expect, it } from 'vitest'
import {
  FIDELITY_LABELS,
  FORBIDDEN_CLAIM_TERMS,
  REQUIRED_WORDING,
  buildSurface,
  forbiddenClaimTermsIn,
  layerIndexOf,
  surfaceStrings,
} from '../app.js'
import { readManifest } from '../manifest-client.js'
import type { Manifest, Refusal } from '../manifest-client.js'
import { refusalToSvg, surfaceToSvg, svgTextContent } from '../render.js'
import { fixtureText, packageFileText, producerGoldenText } from './fixtures.js'

/**
 * The words `index.html` shows a reader: markup, styles and scripts removed.
 *
 * The page's own copy is displayed text that no rendering function produces, so
 * nothing else in this suite reads it.
 */
function visibleHtmlText(html: string): string {
  return html
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

// QM-0150 — the view-model. Everything the surface displays is a value on this
// object, so a test can read the surface's own words rather than a screenshot.
//
// Two rules govern this file:
//   * AGENTS.md §4 — every displayed result is labelled exact, sampled or
//     approximate, visibly.
//   * ARCHITECTURE.md §19 — a colour pattern is not a semantic concept. The
//     surface may say where a number is large. It may not say what that means.

function manifestFrom(name: string, projection: 'summary' | 'full' = 'summary'): Manifest {
  const read = readManifest(fixtureText(name), { projection })
  if (!read.ok) throw new Error(`fixture ${name} did not read: ${read.refusal.message}`)
  return read.value
}

const SUMMARY = manifestFrom('summary.v1.json')
const FULL = manifestFrom('full.v1.json', 'full')
const EMPTY = manifestFrom('summary.empty.json')
const SAMPLED = manifestFrom('summary.sampled.json')
const ZERO_BASE = manifestFrom('summary.zero-base.json')
/** A sampled manifest that carries experts — no other fixture does. */
const SAMPLED_EXPERTS = manifestFrom('summary.sampled-experts.json')

/**
 * Every fidelity word this surface puts in front of a reader, as
 * `<where>: <word>` so a failure names the site.
 *
 * Enumerated exhaustively on purpose. The earlier version of the labelling test
 * listed five sites by hand, omitted expert rows and per-cell fidelity, and a
 * reviewer defeated it by hard-coding `fidelityLabel: 'exact'` on every expert
 * row with the whole suite still green. A named list of sites is only as good
 * as the reader's memory, so this walks the surface instead.
 */
function fidelityLabelSites(surface: ReturnType<typeof buildSurface>): string[] {
  const sites: string[] = [
    `surface.fidelity: ${surface.fidelity}`,
    `surface.fidelityLabel: ${surface.fidelityLabel}`,
    `totals: ${surface.totals.fidelityLabel}`,
    `legend.fidelityLabel: ${surface.heatmap.legend.fidelityLabel}`,
    `grid.fidelity: ${surface.heatmap.grid.fidelity}`,
    ...surface.ranking.rows.map((row) => `ranking[${row.rank}]: ${row.fidelityLabel}`),
    ...surface.frontier.rows.map((row) => `frontier[${row.step}]: ${row.fidelityLabel}`),
    ...(surface.experts?.rows ?? []).map(
      (row) => `expert[${row.layerIndex}.${row.expertIndex}]: ${row.fidelityLabel}`,
    ),
  ]
  for (const row of surface.heatmap.grid.rows) {
    for (const cell of row.cells) {
      sites.push(`cell[${String(cell.layerIndex)}.${cell.columnIndex}]: ${cell.fidelity}`)
    }
  }
  return sites
}

/**
 * Every field of the `Surface` value whose type is `Fidelity`, found by walking
 * the object rather than by naming the fields.
 *
 * `fidelityLabelSites` is a hand-written list, and a hand-written list is what
 * failed before. This is the check on the list: if a future field carries a
 * fidelity word and the list above forgets it, the counts disagree and this
 * fails, naming the path.
 */
function fidelityBearingPaths(value: unknown, path = 'surface'): string[] {
  if (typeof value === 'string') {
    return (FIDELITY_LABELS as readonly string[]).includes(value) ? [`${path} = ${value}`] : []
  }
  if (Array.isArray(value)) {
    return value.flatMap((item, index) => fidelityBearingPaths(item, `${path}[${index}]`))
  }
  if (value !== null && typeof value === 'object') {
    return Object.entries(value).flatMap(([key, item]) =>
      fidelityBearingPaths(item, `${path}.${key}`),
    )
  }
  return []
}

describe('QM-0150 a summary manifest renders at the resolution the manifest publishes', () => {
  it('a_summary_manifest_renders_one_row_per_layer', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.heatmap.grid.rows).toHaveLength(3)
    expect(surface.heatmap.grid.rows.map((r) => r.layerIndex)).toEqual([0, 1, 2])
    expect(surface.heatmap.empty).toBeNull()
  })

  it('a_summary_manifest_publishes_one_number_per_layer_so_each_row_is_one_cell', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.heatmap.grid.cellCount).toBe(3)
    expect(surface.heatmap.resolution).toBe('layer')
  })

  it('the_cell_values_are_the_relative_errors_derived_from_the_layer_partials', () => {
    // Hand-computed from the fixture: sqrt(4/400)=0.1, sqrt(25/100)=0.5, sqrt(1/64)=0.125.
    const surface = buildSurface(SUMMARY)
    const values = surface.heatmap.grid.rows.map((r) => r.cells[0].relativeError)
    expect(values[0]).toBeCloseTo(0.1, 12)
    expect(values[1]).toBeCloseTo(0.5, 12)
    expect(values[2]).toBeCloseTo(0.125, 12)
  })

  it('a_layer_level_cell_declares_that_its_channel_extent_is_not_published', () => {
    // The manifest carries no per-channel partials. Claiming a channel extent
    // would be a number this surface made up.
    const surface = buildSurface(SUMMARY)
    const cell = surface.heatmap.grid.rows[0].cells[0]
    expect(cell.channelsPerCell).toBeNull()
    expect(cell.aggregated).toBe(true)
    expect(surface.heatmap.resolutionNote).toContain('manifest v1')
    expect(surface.heatmap.resolutionNote).toContain('per-channel')
  })

  it('a_full_manifest_renders_one_column_per_tensor_with_the_channel_extent_its_shape_declares', () => {
    // Fixture shapes: layer 0 has [6,8] and [4,8] — 6 and 4 output channels,
    // ordered by canonical address; layer 1 has [8,6] — 8 output channels.
    const surface = buildSurface(FULL)
    expect(surface.heatmap.resolution).toBe('tensor')
    const layer0 = surface.heatmap.grid.rows.find((r) => r.layerIndex === 0)
    expect(layer0?.cells.map((c) => c.channelsPerCell)).toEqual([6, 4])
    expect(layer0?.channelCount).toBe(10)
    const layer1 = surface.heatmap.grid.rows.find((r) => r.layerIndex === 1)
    expect(layer1?.cells.map((c) => c.channelsPerCell)).toEqual([8])
  })

  it('a_tensor_outside_the_layer_stack_gets_its_own_row_rather_than_being_dropped', () => {
    // `model.embedding.token_embedding.weight` belongs to no layer. Dropping it
    // is truncation, and truncation is the failure mode that produces a
    // confidently wrong picture.
    const surface = buildSurface(FULL)
    const outside = surface.heatmap.grid.rows.find((r) => r.layerIndex === null)
    expect(outside, 'the non-layer tensor row is missing').toBeDefined()
    expect(outside?.cells[0].sources).toEqual(['model.embedding.token_embedding.weight'])
  })

  it('every_tensor_in_the_manifest_appears_in_exactly_one_cell', () => {
    const surface = buildSurface(FULL)
    const seen = surface.heatmap.grid.rows.flatMap((r) => r.cells.flatMap((c) => c.sources))
    expect(seen.slice().sort()).toEqual((FULL.tensors ?? []).map((t) => t.address).slice().sort())
  })

  it('a_canonical_address_yields_its_layer_index_and_a_non_layer_address_yields_none', () => {
    expect(layerIndexOf('model.layers[10].self_attention.query_projection.weight')).toBe(10)
    expect(layerIndexOf('model.layers[0].mlp.up_projection.weight')).toBe(0)
    expect(layerIndexOf('model.embedding.token_embedding.weight')).toBeNull()
    expect(layerIndexOf('lm_head.weight')).toBeNull()
  })
})

describe('QM-0150 every displayed result is labelled exact, sampled or approximate', () => {
  it('the_fidelity_word_from_the_manifest_appears_in_the_surface_itself', () => {
    for (const [manifest, expected] of [
      [SUMMARY, 'exact'],
      [SAMPLED, 'sampled'],
    ] as const) {
      const surface = buildSurface(manifest)
      expect(surface.fidelity).toBe(expected)
      expect(surface.fidelityLabel).toBe(expected)
      expect(surfaceStrings(surface).some((s) => s.includes(expected))).toBe(true)
    }
  })

  it('the_heat_map_panel_carries_the_fidelity_label_not_only_a_tooltip', () => {
    const surface = buildSurface(SAMPLED)
    expect(surface.heatmap.legend.fidelityLabel).toBe('sampled')
    expect(surface.heatmap.legend.entries.some((e) => e.label.includes('sampled'))).toBe(true)
  })

  it('every_ranking_row_carries_its_own_fidelity_label', () => {
    const surface = buildSurface(SAMPLED)
    expect(surface.ranking.rows.length).toBeGreaterThan(0)
    expect(surface.ranking.rows.every((row) => row.fidelityLabel === 'sampled')).toBe(true)
  })

  it('every_frontier_row_carries_its_own_fidelity_label', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.frontier.rows.length).toBeGreaterThan(0)
    expect(surface.frontier.rows.every((row) => row.fidelityLabel === 'exact')).toBe(true)
  })

  it('a_sampled_manifest_carries_experts_in_at_least_one_fixture_so_the_expert_panel_is_covered', () => {
    // Without this, every assertion about expert labelling is vacuous: the
    // other four fixtures all report `experts: []`.
    const surface = buildSurface(SAMPLED_EXPERTS)
    expect(surface.fidelity).toBe('sampled')
    expect(surface.experts?.rows.length).toBeGreaterThan(0)
    expect(surface.ranking.rows.length).toBeGreaterThan(0)
    expect(surface.frontier.rows.length).toBeGreaterThan(0)
    expect(surface.heatmap.grid.cellCount).toBeGreaterThan(0)
  })

  it('a_sampled_manifest_is_never_labelled_exact_anywhere_in_the_surface', () => {
    for (const manifest of [SAMPLED, SAMPLED_EXPERTS]) {
      const surface = buildSurface(manifest)

      // Every site this surface displays a fidelity word at, expert rows and
      // per-cell fidelity included.
      const sites = fidelityLabelSites(surface)
      expect(sites.length, 'no label site was found, so this proves nothing').toBeGreaterThan(6)
      expect(sites.filter((site) => !site.endsWith(': sampled'))).toEqual([])

      // And the check on that list: every `Fidelity`-valued string anywhere in
      // the surface value, found by walking it rather than by naming fields.
      const walked = fidelityBearingPaths(surface)
      expect(walked.filter((path) => !path.endsWith('= sampled'))).toEqual([])
      expect(
        walked.length,
        `the enumerated sites (${sites.length}) and the walked fields (${walked.length}) disagree; ` +
          `a new fidelity-bearing field is not in fidelityLabelSites`,
      ).toBe(sites.length)
    }
  })

  it('the_two_fidelity_words_a_sampled_run_did_not_earn_appear_nowhere_a_reader_can_see', () => {
    for (const manifest of [SAMPLED, SAMPLED_EXPERTS]) {
      const surface = buildSurface(manifest)
      const displayed = [
        ...surfaceStrings(surface),
        surfaceToSvg(surface, { palette: 'colour' }),
        surfaceToSvg(surface, { palette: 'greyscale' }),
      ]
      // Non-vacuity: the word it DID earn is on screen, in both outputs.
      expect(displayed.filter((text) => /\bsampled\b/i.test(text)).length).toBeGreaterThan(2)
      for (const unearned of ['exact', 'approximate']) {
        const pattern = new RegExp(`\\b${unearned}\\b`, 'i')
        for (const text of displayed) {
          expect(
            pattern.test(text),
            `a sampled run displays "${unearned}" in: ${text.slice(0, 200)}`,
          ).toBe(false)
        }
      }
    }
  })

  it('the_three_fidelity_words_are_the_ones_the_data_model_uses_end_to_end', () => {
    expect(FIDELITY_LABELS).toEqual(['exact', 'sampled', 'approximate'])
  })

  it('aggregation_is_labelled_separately_from_fidelity_because_they_mean_different_things', () => {
    // `sampled` is the engine's coarseness; `aggregated` is the renderer's.
    // Conflating them tells the reader the data is coarse when the display is.
    const surface = buildSurface(SAMPLED)
    const cell = surface.heatmap.grid.rows[0].cells[0]
    expect(cell.fidelity).toBe('sampled')
    expect(cell.aggregated).toBe(true)
    expect(surface.heatmap.legend.entries.some((e) => e.kind === 'aggregated')).toBe(true)
    expect(surface.heatmap.legend.entries.some((e) => e.kind === 'fidelity')).toBe(true)
  })
})

describe('QM-0150 a colour pattern is not a semantic concept', () => {
  it('the_legend_states_what_the_colour_encodes_in_terms_of_the_measured_quantity', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.heatmap.legend.encodes).toContain('relative weight-space error')
    expect(surface.heatmap.legend.encodes).toContain('sum_sq_delta')
  })

  it('the_legend_states_in_the_surface_that_a_colour_is_not_a_finding', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.heatmap.legend.notAClaim).toBe(REQUIRED_WORDING.colour)
    expect(surfaceStrings(surface)).toContain(REQUIRED_WORDING.colour)
  })

  it('the_ranking_panel_carries_the_proxy_for_sensitivity_wording_the_architecture_requires', () => {
    // .plan/DIAGNOSTIC_ARCHITECTURE.md §8: "This layer is important/unimportant"
    // is forbidden; this is the required wording in its place.
    const surface = buildSurface(SUMMARY)
    expect(surface.ranking.caveat).toBe(REQUIRED_WORDING.ranking)
    expect(REQUIRED_WORDING.ranking).toContain('a proxy for sensitivity')
  })

  it('the_frontier_panel_reproduces_the_manifests_own_claim_string_verbatim', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.frontier.claim).toBe(SUMMARY.frontier.claim)
    expect(surface.frontier.claim).toBe('Greedy over error-per-byte; not proven optimal.')
    expect(surfaceStrings(surface)).toContain('Greedy over error-per-byte; not proven optimal.')
  })

  it('the_accuracy_caveat_is_displayed_with_the_wording_the_architecture_requires', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.caveats).toContain(REQUIRED_WORDING.accuracy)
  })

  it('no_string_the_surface_displays_uses_a_forbidden_claim_term', () => {
    for (const manifest of [SUMMARY, FULL, EMPTY, SAMPLED, SAMPLED_EXPERTS, ZERO_BASE]) {
      const surface = buildSurface(manifest)
      for (const text of surfaceStrings(surface)) {
        expect(forbiddenClaimTermsIn(text), `forbidden claim in: ${text}`).toEqual([])
      }
    }
  })

  it('no_word_drawn_into_the_rendered_image_uses_a_forbidden_claim_term', () => {
    // `surfaceStrings()` is the view-model, and the view-model is not the page.
    // The headings, "not measured", and the whole of the refusal state are
    // literals in `render.ts` that no view-model scan can ever reach.
    const drawn: string[] = []
    for (const manifest of [SUMMARY, FULL, EMPTY, SAMPLED, SAMPLED_EXPERTS, ZERO_BASE]) {
      const surface = buildSurface(manifest)
      for (const palette of ['colour', 'greyscale'] as const) {
        for (const line of svgTextContent(surfaceToSvg(surface, { palette }))) {
          drawn.push(line)
          expect(forbiddenClaimTermsIn(line), `forbidden claim drawn in the image: ${line}`).toEqual([])
        }
      }
    }
    // Non-vacuous: these four are `render.ts` literals, and not one of them
    // appears in `surfaceStrings`.
    for (const literal of ['Legend', 'Ranked by relative weight-space error', 'Mixed-precision frontier', 'Declared gaps']) {
      expect(drawn, `the extractor did not read the renderer's own copy`).toContain(literal)
      expect(surfaceStrings(buildSurface(SUMMARY))).not.toContain(literal)
    }
  })

  it('no_word_drawn_into_a_refusal_image_uses_a_forbidden_claim_term', () => {
    const refusals: Refusal[] = [
      { kind: 'malformed_json', message: 'unexpected end of JSON input' },
      { kind: 'unsupported_version', found: 7, supported: 1, message: 'manifest_version 7 / 1' },
      {
        kind: 'schema_invalid',
        errors: [{ path: '/layers/0', keyword: 'required', message: 'aggregate is required' }],
        message: 'the manifest does not match the published schema',
      },
      { kind: 'wrong_projection', expected: 'summary', found: 'full', message: 'full on the summary route' },
      { kind: 'payload_too_large', bytes: 9, ceilingBytes: 8, message: 'too large' },
      { kind: 'declared_gap', requirement: 'QM-0152', message: 'no per-layer route exists yet' },
      { kind: 'transport_failure', retryable: true, message: 'connection refused' },
    ]
    const drawn: string[] = []
    for (const refusal of refusals) {
      for (const line of svgTextContent(refusalToSvg(refusal))) {
        drawn.push(line)
        expect(forbiddenClaimTermsIn(line), `forbidden claim in a refusal: ${line}`).toEqual([])
      }
    }
    expect(drawn).toContain('Nothing was rendered.')
    expect(drawn).toContain('is worse than no picture.')
    expect(drawn).toContain('Retry')
  })

  it('no_word_on_the_page_around_the_image_uses_a_forbidden_claim_term', () => {
    // index.html carries the boundary paragraph, which is the first thing a
    // reader reads and is not produced by any code this suite otherwise runs.
    const visible = visibleHtmlText(packageFileText('index.html'))
    expect(visible, 'the boundary paragraph is not on the page').toContain(
      'a colour here is not a finding',
    )
    expect(forbiddenClaimTermsIn(visible), `forbidden claim on the page: ${visible}`).toEqual([])
  })

  it('the_drawn_text_scans_above_would_catch_a_forbidden_term_that_was_planted_in_them', () => {
    // The scans are only worth their names if the extractors reach the text.
    const planted = buildSurface({
      ...SUMMARY,
      refusals: [{ requirement_id: 'X-1', what: 'this layer is important', why: 'planted' }],
    })
    const drawn = svgTextContent(surfaceToSvg(planted, { palette: 'colour' }))
    expect(drawn.flatMap(forbiddenClaimTermsIn)).toContain('important')

    const refusal = svgTextContent(
      refusalToSvg({ kind: 'transport_failure', retryable: false, message: 'this expert is dead' }),
    )
    expect(refusal.flatMap(forbiddenClaimTermsIn)).toContain('dead')

    expect(forbiddenClaimTermsIn(visibleHtmlText('<p>a red band is a semantic concept</p>'))).toEqual([
      'semantic',
      'concept',
    ])
  })

  it('the_forbidden_claim_checker_is_not_vacuous', () => {
    // If this ever returns [], the test above proves nothing.
    expect(forbiddenClaimTermsIn('this layer is important')).toContain('important')
    expect(forbiddenClaimTermsIn('this expert is dead')).toContain('dead')
    expect(forbiddenClaimTermsIn('the red band is a semantic concept')).toContain('semantic')
    expect(forbiddenClaimTermsIn('the model understands syntax here')).toContain('understand')
  })

  it('the_checker_matches_whole_words_so_ordinary_prose_is_not_flagged', () => {
    expect(forbiddenClaimTermsIn('deadline')).toEqual([])
    expect(forbiddenClaimTermsIn('the concepts directory')).toEqual([])
  })

  it('the_forbidden_terms_include_every_claim_the_diagnostic_architecture_forbids', () => {
    for (const term of ['important', 'unimportant', 'dead', 'semantic', 'hessian']) {
      expect(FORBIDDEN_CLAIM_TERMS).toContain(term)
    }
  })

  it('no_required_wording_trips_the_forbidden_claim_checker', () => {
    for (const wording of Object.values(REQUIRED_WORDING)) {
      expect(forbiddenClaimTermsIn(wording), `required wording is self-inconsistent: ${wording}`).toEqual(
        [],
      )
    }
  })
})

describe('QM-0150 the ranked list and the frontier agree with the manifest', () => {
  it('the_ranked_list_reproduces_the_manifests_own_order_without_re_sorting_it', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.ranking.rows.map((r) => r.address)).toEqual(SUMMARY.ranking.map((r) => r.address))
    expect(surface.ranking.rows.map((r) => r.relativeError)).toEqual(
      SUMMARY.ranking.map((r) => r.relative_error),
    )
  })

  it('each_ranked_row_carries_the_parameter_count_the_manifest_gives_it', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.ranking.rows.map((r) => r.parameterCount)).toEqual([4096, 2048, 1024])
  })

  it('each_ranked_row_names_the_layer_its_address_belongs_to', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.ranking.rows.map((r) => r.layerIndex)).toEqual([1, 2, 0])
  })

  it('the_frontier_table_reproduces_the_manifests_steps_with_their_costs', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.frontier.rows.map((r) => r.addedBytes)).toEqual([6144, 9216])
    expect(surface.frontier.rows.map((r) => r.errorRemovedFraction)).toEqual([0.5, 0.75])
    expect(surface.frontier.rows.map((r) => r.keepSetSize)).toEqual([1, 2])
  })

  it('the_heat_map_and_the_ranked_list_are_derived_from_the_same_manifest_and_do_not_disagree', () => {
    // The tensor the ranking puts first must belong to the layer the heat-map
    // paints darkest. Two panels that disagree are worse than one panel.
    const surface = buildSurface(SUMMARY)
    const worstRanked = surface.ranking.rows[0].layerIndex
    const worstPainted = surface.heatmap.grid.rows
      .slice()
      .sort((a, b) => (b.cells[0].relativeError ?? -1) - (a.cells[0].relativeError ?? -1))[0].layerIndex
    expect(worstPainted).toBe(worstRanked)
  })
})

describe('QM-0150 refusals are visible with their requirement IDs', () => {
  it('every_refusal_in_the_manifest_is_shown_with_its_requirement_id', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.refusals.rows.map((r) => r.requirementId)).toEqual(['EVAL-001', 'GRID-007'])
    expect(surface.refusals.rows.map((r) => r.what)).toEqual([
      'accuracy estimate',
      'rank-4 tensor model.layers[1].router.gate_projection.weight',
    ])
  })

  it('the_reason_for_each_refusal_is_displayed_not_only_its_id', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.refusals.rows[0].why).toContain('Accuracy impact is not measured')
    expect(surfaceStrings(surface).some((s) => s.includes('ADR-010'))).toBe(true)
  })

  it('an_empty_refusals_array_is_shown_as_the_claim_it_is_rather_than_as_an_absent_panel', () => {
    // `refusals: []` asserts that nothing was refused. That is a claim, and it
    // is displayed as one.
    const manifest = { ...SUMMARY, refusals: [] }
    const surface = buildSurface(manifest)
    expect(surface.refusals.rows).toEqual([])
    expect(surface.refusals.empty).not.toBeNull()
    expect(surface.refusals.empty?.explanation).toContain('nothing was refused')
  })
})

describe('QM-0150 empty and undefined data refuse to look like measurements', () => {
  it('an_empty_ranking_renders_an_explanatory_state_and_not_a_blank_panel', () => {
    const surface = buildSurface(EMPTY)
    expect(surface.ranking.rows).toEqual([])
    expect(surface.ranking.empty).not.toBeNull()
    expect(surface.ranking.empty?.explanation.length).toBeGreaterThan(0)
  })

  it('an_empty_layer_list_renders_an_explanatory_state_rather_than_an_empty_grid', () => {
    const surface = buildSurface(EMPTY)
    expect(surface.heatmap.grid.cellCount).toBe(0)
    expect(surface.heatmap.empty).not.toBeNull()
    expect(surface.heatmap.empty?.explanation).toContain('no layer')
  })

  it('an_empty_run_points_the_reader_at_the_refusal_that_explains_why_it_is_empty', () => {
    const surface = buildSurface(EMPTY)
    expect(surface.heatmap.empty?.requirementIds).toContain('QUANT-003')
  })

  it('an_empty_frontier_renders_an_explanatory_state_rather_than_an_empty_table', () => {
    const surface = buildSurface(EMPTY)
    expect(surface.frontier.rows).toEqual([])
    expect(surface.frontier.empty).not.toBeNull()
  })

  it('a_layer_with_nothing_measured_is_shown_as_not_measured_and_never_as_zero_error', () => {
    const surface = buildSurface(ZERO_BASE)
    const cells = surface.heatmap.grid.rows.map((r) => r.cells[0])
    expect(cells[0].relativeError).toBeCloseTo(0.5, 12)
    expect(cells[1].relativeError).toBeNull()
    expect(surface.heatmap.grid.undefinedCellCount).toBe(1)
    expect(surface.heatmap.legend.entries.some((e) => e.kind === 'undefined')).toBe(true)
  })

  it('a_single_valued_map_prints_one_legend_entry_that_says_so_not_six_identical_ones', () => {
    // The sampled fixture measures 0.2 on both layers. The legend used to print
    // six tiers all reading "fill 100%", with six identical swatches: a key
    // whose entries cannot be told apart cannot be used to decode the map, and
    // a spread-less map painted at the darkest tier reads as maximally bad.
    const surface = buildSurface(SAMPLED)
    expect(surface.heatmap.grid.domain).toEqual({ min: 0.2, max: 0.2 })
    expect(surface.heatmap.legend.entries.filter((e) => e.kind === 'magnitude')).toEqual([])
    const uniform = surface.heatmap.legend.entries.filter((e) => e.kind === 'uniform')
    expect(uniform).toHaveLength(1)
    expect(uniform[0].label).toContain('0.2')
    expect(surface.heatmap.legend.scaleNote).toContain('same value')
  })

  it('a_map_with_a_real_spread_still_prints_the_six_tier_key', () => {
    // The single-valued case must not swallow the ordinary one.
    const surface = buildSurface(SUMMARY)
    expect(surface.heatmap.legend.entries.filter((e) => e.kind === 'magnitude')).toHaveLength(6)
    expect(surface.heatmap.legend.entries.filter((e) => e.kind === 'uniform')).toEqual([])
  })

  it('a_map_with_nothing_measurable_prints_no_magnitude_key_rather_than_one_for_an_invented_range', () => {
    // `grid.domain` is null here. The legend used to substitute 0..1 and print
    // a six-tier ramp for a map that has no cells at all.
    const surface = buildSurface(EMPTY)
    expect(surface.heatmap.grid.domain).toBeNull()
    expect(surface.heatmap.legend.entries.filter((e) => e.kind === 'magnitude')).toEqual([])
    expect(surface.heatmap.legend.entries.filter((e) => e.kind === 'uniform')).toEqual([])
    expect(surface.heatmap.legend.scaleNote).toContain('no scale')
  })

  it('no_two_legend_entries_are_drawn_the_same_way_as_each_other', () => {
    // Labels differ trivially; what a reader matches against the map is the
    // swatch and the glyph. Two entries that look identical are two entries
    // that cannot be used.
    for (const manifest of [SUMMARY, FULL, EMPTY, SAMPLED, SAMPLED_EXPERTS, ZERO_BASE]) {
      const entries = buildSurface(manifest).heatmap.legend.entries
      const marks = entries.map((e) => `${e.colour}|${e.greyscale}|${e.glyph}`)
      expect(new Set(marks).size, `indistinguishable legend entries: ${marks.join(' , ')}`).toBe(
        marks.length,
      )
    }
  })

  it('a_manifest_with_no_experts_omits_the_expert_panel_rather_than_showing_an_empty_one', () => {
    expect(buildSurface(SUMMARY).experts).toBeNull()
  })

  it('a_manifest_with_experts_shows_the_expert_panel', () => {
    const withExperts: Manifest = {
      ...SUMMARY,
      experts: [{ layer_index: 0, expert_index: 3, aggregate: SUMMARY.layers[0].aggregate }],
    }
    const surface = buildSurface(withExperts)
    expect(surface.experts?.rows).toHaveLength(1)
    expect(surface.experts?.rows[0].expertIndex).toBe(3)
    expect(surface.experts?.rows[0].fidelityLabel).toBe('exact')
  })
})

describe('QM-0150 drilling down', () => {
  it('the_default_drill_level_is_the_model_and_names_no_layer', () => {
    const surface = buildSurface(SUMMARY)
    expect(surface.drill.level).toBe('model')
    expect(surface.drill.layerIndex).toBeNull()
    expect(surface.drill.path).toEqual(['model'])
  })

  it('selecting_a_layer_narrows_the_grid_to_that_layers_tensors', () => {
    const surface = buildSurface(FULL, { layerIndex: 0 })
    expect(surface.drill.level).toBe('layer')
    expect(surface.drill.layerIndex).toBe(0)
    expect(surface.drill.path).toEqual(['model', 'layer 0'])
    expect(surface.heatmap.grid.rows).toHaveLength(1)
    expect(surface.heatmap.grid.rows[0].cells).toHaveLength(2)
  })

  it('a_layer_index_outside_the_manifest_is_a_named_error_not_an_empty_grid', () => {
    expect(() => buildSurface(FULL, { layerIndex: 99 })).toThrow(/99/)
    expect(() => buildSurface(FULL, { layerIndex: -1 })).toThrow(/-1/)
  })

  it('selecting_a_layer_on_a_summary_manifest_is_a_named_error_because_the_detail_is_not_there', () => {
    expect(() => buildSurface(SUMMARY, { layerIndex: 0 })).toThrow(/summary/)
  })
})

describe('QM-0150 the surface reads the producers own manifest', () => {
  it('q_reports_golden_summary_manifest_renders_without_a_type_drift', () => {
    const read = readManifest(producerGoldenText('manifest.v1.summary.json'), { projection: 'summary' })
    expect(read.ok).toBe(true)
    if (!read.ok) return
    const surface = buildSurface(read.value)
    expect(surface.heatmap.grid.rows.length).toBe(read.value.layers.length)
    expect(surface.ranking.rows.length).toBe(read.value.ranking.length)
    expect(FIDELITY_LABELS).toContain(surface.fidelityLabel)
  })

  it('q_reports_golden_full_manifest_renders_at_tensor_resolution', () => {
    const read = readManifest(producerGoldenText('manifest.v1.json'), { projection: 'full' })
    expect(read.ok).toBe(true)
    if (!read.ok) return
    const surface = buildSurface(read.value)
    expect(surface.heatmap.resolution).toBe('tensor')
    const painted = surface.heatmap.grid.rows.flatMap((r) => r.cells.flatMap((c) => c.sources))
    expect(painted.slice().sort()).toEqual((read.value.tensors ?? []).map((t) => t.address).slice().sort())
  })
})
