import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import {
  ANNOTATION_KEYWORDS,
  IMPLEMENTED_KEYWORDS,
  collectKeywords,
  validate,
} from '../schema-validator.js'
import { MANIFEST_SCHEMA_V1 } from '../schema.js'
import { PUBLISHED_SCHEMA_PATH, clone, fixtureJson } from './fixtures.js'

// QM-0150 — the manifest reader validates against
// `schemas/diagnostics/manifest.v1.json` as published. No JSON-Schema library is
// available offline, so this package carries a draft-07 **subset** validator.
//
// A subset validator has one catastrophic failure mode: silently ignoring a
// keyword and passing everything. `every_keyword...` below is the guard against
// exactly that, and it is the most important test in this file.

const VALID_SUMMARY = fixtureJson<Record<string, unknown>>('summary.v1.json')

describe('QM-0150 the published schema is the one this package validates against', () => {
  it('the_schema_is_read_from_the_repository_and_not_copied_into_this_package', () => {
    const onDisk = JSON.parse(readFileSync(PUBLISHED_SCHEMA_PATH, 'utf8')) as Record<string, unknown>
    expect(MANIFEST_SCHEMA_V1).toEqual(onDisk)
    expect(onDisk.$id).toBe('https://quatricmorph.dev/schemas/diagnostics/manifest/v1')
  })

  it('every_keyword_in_the_published_schema_is_one_this_validator_implements_or_knowingly_ignores', () => {
    // A validator that meets an unknown keyword and shrugs will accept a
    // malformed manifest and render it. Enumerate what the schema actually
    // uses and refuse to be surprised.
    const used = [...collectKeywords(MANIFEST_SCHEMA_V1)].sort()
    const known = new Set<string>([...IMPLEMENTED_KEYWORDS, ...ANNOTATION_KEYWORDS])
    const unhandled = used.filter((keyword) => !known.has(keyword))
    expect(
      unhandled,
      `schemas/diagnostics/manifest.v1.json uses keywords this validator does not implement: ${JSON.stringify(unhandled)}`,
    ).toEqual([])
  })

  it('collect_keywords_finds_keywords_nested_under_definitions_and_all_of', () => {
    // Guards the enumerator itself: if it stopped descending, the check above
    // would pass vacuously.
    const used = collectKeywords(MANIFEST_SCHEMA_V1)
    for (const nested of ['exclusiveMinimum', 'maxItems', 'minItems', 'const', 'not', 'if', 'then', 'else']) {
      expect(used.has(nested), `collectKeywords missed ${nested}`).toBe(true)
    }
  })

  it('a_keyword_the_validator_does_not_implement_is_reported_rather_than_ignored', () => {
    const used = collectKeywords({ type: 'object', patternProperties: { '^x': { type: 'string' } } })
    expect(used.has('patternProperties')).toBe(true)
    expect(IMPLEMENTED_KEYWORDS.includes('patternProperties')).toBe(false)
  })
})

describe('QM-0150 draft-07 subset validation accepts what the producer emits', () => {
  it('the_checked_in_summary_fixture_validates_against_the_published_schema', () => {
    expect(validate(MANIFEST_SCHEMA_V1, VALID_SUMMARY)).toEqual([])
  })

  it('the_checked_in_full_projection_fixture_validates_against_the_published_schema', () => {
    expect(validate(MANIFEST_SCHEMA_V1, fixtureJson('full.v1.json'))).toEqual([])
  })

  it('the_schemas_own_example_validates_against_itself', () => {
    // The schema carries an `examples` array. If the example does not satisfy
    // the schema, one of the two is wrong and this package must not guess which.
    const examples = (MANIFEST_SCHEMA_V1 as { examples?: unknown[] }).examples ?? []
    expect(examples.length).toBeGreaterThan(0)
    for (const example of examples) {
      expect(validate(MANIFEST_SCHEMA_V1, example)).toEqual([])
    }
  })
})

describe('QM-0150 draft-07 subset validation refuses what the schema forbids', () => {
  it('a_missing_required_field_is_reported_with_its_name', () => {
    const broken = clone(VALID_SUMMARY)
    delete broken.refusals
    const errors = validate(MANIFEST_SCHEMA_V1, broken)
    expect(errors.length).toBeGreaterThan(0)
    expect(errors.some((e) => e.keyword === 'required' && e.message.includes('refusals'))).toBe(true)
  })

  it('an_unknown_top_level_field_is_reported_because_additional_properties_are_forbidden', () => {
    const broken = clone(VALID_SUMMARY)
    broken.per_channel = [1, 2, 3]
    const errors = validate(MANIFEST_SCHEMA_V1, broken)
    expect(errors.some((e) => e.keyword === 'additionalProperties' && e.message.includes('per_channel'))).toBe(
      true,
    )
  })

  it('a_manifest_version_other_than_one_fails_the_const_keyword', () => {
    const broken = clone(VALID_SUMMARY)
    broken.manifest_version = 2
    const errors = validate(MANIFEST_SCHEMA_V1, broken)
    expect(errors.some((e) => e.keyword === 'const' && e.path === '/manifest_version')).toBe(true)
  })

  it('a_value_outside_an_enum_is_reported_with_its_path', () => {
    const broken = clone(VALID_SUMMARY) as { run: { backend: string } }
    broken.run.backend = 'cuda'
    const errors = validate(MANIFEST_SCHEMA_V1, broken)
    expect(errors.some((e) => e.keyword === 'enum' && e.path === '/run/backend')).toBe(true)
  })

  it('a_negative_number_below_a_minimum_is_reported', () => {
    const broken = clone(VALID_SUMMARY) as { totals: { sum_sq_base: number } }
    broken.totals.sum_sq_base = -1
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'minimum')).toBe(true)
  })

  it('a_zero_peak_resident_byte_count_fails_exclusive_minimum', () => {
    const broken = clone(VALID_SUMMARY) as { run: { peak_resident_bytes: number } }
    broken.run.peak_resident_bytes = 0
    expect(
      validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'exclusiveMinimum'),
    ).toBe(true)
  })

  it('an_empty_string_where_min_length_one_is_required_is_reported', () => {
    const broken = clone(VALID_SUMMARY) as { model: { revision_hash: string } }
    broken.model.revision_hash = ''
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'minLength')).toBe(true)
  })

  it('a_string_where_an_integer_is_required_is_reported', () => {
    const broken = clone(VALID_SUMMARY) as { model: { parameter_count: unknown } }
    broken.model.parameter_count = '3000'
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'type')).toBe(true)
  })

  it('a_fractional_value_where_an_integer_is_required_is_reported', () => {
    const broken = clone(VALID_SUMMARY) as { model: { parameter_count: number } }
    broken.model.parameter_count = 3000.5
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'type')).toBe(true)
  })

  it('a_summary_projection_carrying_a_tensors_array_is_refused_by_the_all_of_branch', () => {
    // The schema's `allOf`: `full` requires `tensors`, anything else forbids it.
    // A summary that smuggles the per-tensor array in is the exact payload
    // ARCHITECTURE.md §19 forbids pushing into a browser.
    const broken = clone(VALID_SUMMARY)
    broken.tensors = []
    expect(validate(MANIFEST_SCHEMA_V1, broken).length).toBeGreaterThan(0)
  })

  it('a_full_projection_without_a_tensors_array_is_refused_by_the_all_of_branch', () => {
    const broken = clone(fixtureJson<Record<string, unknown>>('full.v1.json'))
    delete broken.tensors
    expect(validate(MANIFEST_SCHEMA_V1, broken).length).toBeGreaterThan(0)
  })

  it('a_rank_four_tensor_shape_is_refused_by_max_items', () => {
    const broken = clone(fixtureJson<Record<string, unknown>>('full.v1.json')) as {
      tensors: { shape: number[] }[]
    }
    broken.tensors[0].shape = [2, 2, 2, 2]
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'maxItems')).toBe(true)
  })

  it('an_empty_frontier_keep_set_is_refused_by_min_items', () => {
    const broken = clone(VALID_SUMMARY) as {
      frontier: { steps: { keep_set: string[] }[] }
    }
    broken.frontier.steps[0].keep_set = []
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'minItems')).toBe(true)
  })

  it('an_error_removed_fraction_above_one_is_refused_by_maximum', () => {
    const broken = clone(VALID_SUMMARY) as {
      frontier: { steps: { error_removed_fraction: number }[] }
    }
    broken.frontier.steps[0].error_removed_fraction = 1.5
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'maximum')).toBe(true)
  })

  it('a_per_group_granularity_without_a_group_size_is_refused_by_the_if_then_branch', () => {
    const broken = clone(VALID_SUMMARY) as { config: { granularity: Record<string, unknown> } }
    broken.config.granularity = { kind: 'per_group' }
    expect(validate(MANIFEST_SCHEMA_V1, broken).length).toBeGreaterThan(0)
  })

  it('a_per_tensor_granularity_carrying_a_group_size_is_refused_by_the_else_branch', () => {
    const broken = clone(VALID_SUMMARY) as { config: { granularity: Record<string, unknown> } }
    broken.config.granularity = { kind: 'per_tensor', group_size: 128 }
    expect(validate(MANIFEST_SCHEMA_V1, broken).length).toBeGreaterThan(0)
  })

  it('a_null_where_an_object_is_required_is_reported_rather_than_treated_as_absent', () => {
    const broken = clone(VALID_SUMMARY)
    broken.frontier = null
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'type')).toBe(true)
  })

  it('an_array_where_an_object_is_required_is_reported', () => {
    const broken = clone(VALID_SUMMARY)
    broken.model = []
    expect(validate(MANIFEST_SCHEMA_V1, broken).some((e) => e.keyword === 'type')).toBe(true)
  })

  it('a_bad_entry_deep_inside_an_array_is_reported_with_its_index_in_the_path', () => {
    const broken = clone(VALID_SUMMARY) as { layers: { aggregate: { count: unknown } }[] }
    broken.layers[2].aggregate.count = -4
    const errors = validate(MANIFEST_SCHEMA_V1, broken)
    expect(errors.some((e) => e.path === '/layers/2/aggregate/count')).toBe(true)
  })

  it('every_defect_in_a_manifest_with_several_is_reported_not_only_the_first', () => {
    const broken = clone(VALID_SUMMARY) as Record<string, unknown> & {
      run: { backend: string }
      model: { revision_hash: string }
    }
    broken.run.backend = 'cuda'
    broken.model.revision_hash = ''
    delete broken.refusals
    const paths = new Set(validate(MANIFEST_SCHEMA_V1, broken).map((e) => e.path))
    expect(paths.has('/run/backend')).toBe(true)
    expect(paths.has('/model/revision_hash')).toBe(true)
    expect(paths.has('')).toBe(true)
  })
})
