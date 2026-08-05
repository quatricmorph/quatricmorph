import { describe, expect, it } from 'vitest'
import {
  ENDPOINTS,
  HttpTransport,
  MAX_MANIFEST_PAYLOAD_BYTES,
  SUPPORTED_MANIFEST_VERSION,
  loadLayerDetail,
  loadSummary,
  readManifest,
  relativeErrorOf,
  rmseOf,
  type ManifestTransport,
  type TransportResult,
} from '../manifest-client.js'
import { clone, fixtureJson, fixtureText, producerGoldenText } from './fixtures.js'

// QM-0150 — the manifest client. Nothing here reaches the network; every
// transport in this file is a fake fed from a checked-in fixture, and the
// request log is the assertion that the initial view fetched the summary
// projection and nothing else (ARCHITECTURE.md §19, "do not send the entire
// tensor into the browser").

/** A transport that records what was asked for and answers from a fixture. */
class RecordingTransport implements ManifestTransport {
  readonly requestLog: string[] = []

  constructor(
    private readonly answers: {
      summary?: TransportResult
      detail?: TransportResult
    },
  ) {}

  async fetchSummary(runId: string): Promise<TransportResult> {
    this.requestLog.push(ENDPOINTS.summary(runId))
    return this.answers.summary ?? { kind: 'failure', message: 'no summary configured' }
  }

  async fetchLayerDetail(runId: string, layerIndex: number): Promise<TransportResult> {
    this.requestLog.push(ENDPOINTS.layerDetail(runId, layerIndex))
    return this.answers.detail ?? { kind: 'failure', message: 'no detail configured' }
  }
}

const body = (name: string): TransportResult => ({ kind: 'body', text: fixtureText(name) })

describe('QM-0150 manifest endpoints', () => {
  it('the_summary_and_full_routes_are_the_ones_api_contracts_defines', () => {
    // `.plan/API_CONTRACTS.md` / QM-0143: GET /v1/diagnostics/{runId}/summary
    // and GET /v1/diagnostics/{runId}.
    expect(ENDPOINTS.summary('r1')).toBe('/v1/diagnostics/r1/summary')
    expect(ENDPOINTS.full('r1')).toBe('/v1/diagnostics/r1')
  })

  it('the_supported_manifest_version_is_the_one_the_published_schema_pins', () => {
    expect(SUPPORTED_MANIFEST_VERSION).toBe(1)
  })
})

describe('QM-0150 reading a manifest', () => {
  it('a_valid_summary_manifest_is_read_and_its_layers_are_carried_through', () => {
    const read = readManifest(fixtureText('summary.v1.json'), { projection: 'summary' })
    expect(read.ok).toBe(true)
    if (!read.ok) return
    expect(read.value.manifest_version).toBe(1)
    expect(read.value.projection).toBe('summary')
    expect(read.value.layers.map((l) => l.layer_index)).toEqual([0, 1, 2])
    expect(read.value.fidelity).toBe('exact')
  })

  it('a_valid_full_manifest_is_read_and_carries_its_tensor_array', () => {
    const read = readManifest(fixtureText('full.v1.json'), { projection: 'full' })
    expect(read.ok).toBe(true)
    if (!read.ok) return
    expect(read.value.tensors?.length).toBe(4)
  })

  it('a_manifest_from_the_rust_producer_is_read_without_a_type_drift', () => {
    // The goldens are `q-report`'s own bytes. A field this package's types get
    // wrong shows up here rather than as a wrong picture in a browser.
    const read = readManifest(producerGoldenText('manifest.v1.summary.json'), { projection: 'summary' })
    expect(read.ok, JSON.stringify(read.ok ? null : read.refusal)).toBe(true)
  })
})

describe('QM-0150 a manifest that does not validate is refused, not rendered', () => {
  it('an_unknown_manifest_version_is_refused_naming_both_versions', () => {
    const read = readManifest(fixtureText('version-2.json'), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('unsupported_version')
    if (read.refusal.kind !== 'unsupported_version') return
    expect(read.refusal.found).toBe(2)
    expect(read.refusal.supported).toBe(1)
    // Both versions must appear in the message a reader sees.
    expect(read.refusal.message).toContain('2')
    expect(read.refusal.message).toContain('1')
  })

  it('the_version_is_checked_before_the_schema_so_the_refusal_names_versions_not_a_const_mismatch', () => {
    const read = readManifest(fixtureText('version-2.json'), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).not.toBe('schema_invalid')
  })

  it('a_manifest_failing_schema_validation_is_refused_not_rendered', () => {
    const broken = clone(fixtureJson<Record<string, unknown>>('summary.v1.json'))
    delete broken.refusals
    const read = readManifest(JSON.stringify(broken), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('schema_invalid')
    if (read.refusal.kind !== 'schema_invalid') return
    expect(read.refusal.errors.length).toBeGreaterThan(0)
    expect(read.refusal.message).toContain('refusals')
  })

  it('a_manifest_with_an_unknown_field_is_refused_rather_than_read_past', () => {
    const broken = clone(fixtureJson<Record<string, unknown>>('summary.v1.json'))
    broken.per_channel_error = [0.1, 0.2]
    const read = readManifest(JSON.stringify(broken), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('schema_invalid')
    expect(read.refusal.message).toContain('per_channel_error')
  })

  it('malformed_json_is_refused_with_a_named_parse_failure', () => {
    const read = readManifest('{"manifest_version": 1,', { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('malformed_json')
  })

  it('a_json_document_that_is_not_an_object_is_refused', () => {
    for (const text of ['[]', '"summary"', '42', 'null']) {
      const read = readManifest(text, { projection: 'summary' })
      expect(read.ok, `parsed ${text} as a manifest`).toBe(false)
    }
  })

  it('an_empty_body_is_refused_rather_than_read_as_an_empty_diagnosis', () => {
    const read = readManifest('', { projection: 'summary' })
    expect(read.ok).toBe(false)
  })

  it('a_full_projection_arriving_on_the_summary_route_is_refused_as_the_wrong_projection', () => {
    // The whole point of the summary route. A full manifest here means the
    // per-tensor array reached the browser.
    const read = readManifest(fixtureText('full.v1.json'), { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('wrong_projection')
  })

  it('a_payload_above_the_browser_ceiling_is_refused_before_it_is_parsed', () => {
    const huge = 'x'.repeat(MAX_MANIFEST_PAYLOAD_BYTES + 1)
    const read = readManifest(huge, { projection: 'summary' })
    expect(read.ok).toBe(false)
    if (read.ok) return
    expect(read.refusal.kind).toBe('payload_too_large')
    if (read.refusal.kind !== 'payload_too_large') return
    expect(read.refusal.ceilingBytes).toBe(MAX_MANIFEST_PAYLOAD_BYTES)
    expect(read.refusal.message).toContain('§19')
  })

  it('the_browser_payload_ceiling_is_documented_as_a_number_not_a_feeling', () => {
    expect(MAX_MANIFEST_PAYLOAD_BYTES).toBe(4 * 1024 * 1024)
  })
})

describe('QM-0150 relative error is derived, and an undefined one is not a zero', () => {
  it('relative_error_is_the_root_of_delta_over_base', () => {
    // Hand-computed: sqrt(4 / 400) = 0.1, sqrt(25 / 100) = 0.5, sqrt(1 / 64) = 0.125.
    const manifest = fixtureJson<{ layers: { aggregate: Parameters<typeof relativeErrorOf>[0] }[] }>(
      'summary.v1.json',
    )
    expect(relativeErrorOf(manifest.layers[0].aggregate)).toBeCloseTo(0.1, 12)
    expect(relativeErrorOf(manifest.layers[1].aggregate)).toBeCloseTo(0.5, 12)
    expect(relativeErrorOf(manifest.layers[2].aggregate)).toBeCloseTo(0.125, 12)
  })

  it('rmse_is_the_root_of_delta_over_count', () => {
    // Hand-computed: sqrt(4 / 1000) = 0.06324555320336758.
    const manifest = fixtureJson<{ layers: { aggregate: Parameters<typeof rmseOf>[0] }[] }>(
      'summary.v1.json',
    )
    expect(rmseOf(manifest.layers[0].aggregate)).toBeCloseTo(0.06324555320336758, 12)
  })

  it('a_zero_denominator_yields_no_relative_error_rather_than_a_relative_error_of_zero', () => {
    // The failure that destroys trust in a diagnostic: "nothing was measured"
    // rendered as "measured, and it was perfect".
    const manifest = fixtureJson<{ layers: { aggregate: Parameters<typeof relativeErrorOf>[0] }[] }>(
      'summary.zero-base.json',
    )
    expect(relativeErrorOf(manifest.layers[1].aggregate)).toBeNull()
    expect(rmseOf(manifest.layers[1].aggregate)).toBeNull()
  })
})

describe('QM-0150 the initial view fetches the summary projection and nothing else', () => {
  it('loading_the_initial_view_issues_exactly_one_request_and_it_is_the_summary_route', () => {
    const transport = new RecordingTransport({ summary: body('summary.v1.json') })
    return loadSummary(transport, 'run-a').then((read) => {
      expect(read.ok).toBe(true)
      expect(transport.requestLog).toEqual(['/v1/diagnostics/run-a/summary'])
    })
  })

  it('loading_the_initial_view_never_requests_the_full_per_tensor_manifest', () => {
    const transport = new RecordingTransport({ summary: body('summary.v1.json') })
    return loadSummary(transport, 'run-a').then(() => {
      expect(transport.requestLog).not.toContain(ENDPOINTS.full('run-a'))
      expect(transport.requestLog.some((url) => url === '/v1/diagnostics/run-a')).toBe(false)
    })
  })

  it('per_tensor_detail_is_requested_only_on_an_explicit_layer_selection_and_only_for_that_layer', () => {
    const transport = new RecordingTransport({
      summary: body('summary.v1.json'),
      detail: body('full.v1.json'),
    })
    return loadSummary(transport, 'run-a')
      .then(() => {
        expect(transport.requestLog).toHaveLength(1)
        return loadLayerDetail(transport, 'run-a', 1)
      })
      .then(() => {
        expect(transport.requestLog).toEqual([
          '/v1/diagnostics/run-a/summary',
          '/v1/diagnostics/run-a/layers/1',
        ])
      })
  })

  it('a_declared_gap_from_the_transport_is_surfaced_with_its_requirement_id_and_is_not_retried', () => {
    let calls = 0
    const transport: ManifestTransport = {
      requestLog: [],
      async fetchSummary(): Promise<TransportResult> {
        calls += 1
        return { kind: 'declared_gap', requirement: 'CESIUM-001', message: 'not built' }
      },
      async fetchLayerDetail(): Promise<TransportResult> {
        return { kind: 'failure', message: 'unused' }
      },
    }
    return loadSummary(transport, 'run-a').then((read) => {
      expect(read.ok).toBe(false)
      if (read.ok) return
      expect(read.refusal.kind).toBe('declared_gap')
      if (read.refusal.kind !== 'declared_gap') return
      expect(read.refusal.requirement).toBe('CESIUM-001')
      expect(calls).toBe(1)
    })
  })

  it('a_transport_failure_is_a_named_retryable_error_distinct_from_a_declared_gap', () => {
    const transport = new RecordingTransport({
      summary: { kind: 'failure', message: 'connection refused' },
    })
    return loadSummary(transport, 'run-a').then((read) => {
      expect(read.ok).toBe(false)
      if (read.ok) return
      expect(read.refusal.kind).toBe('transport_failure')
      if (read.refusal.kind !== 'transport_failure') return
      expect(read.refusal.retryable).toBe(true)
      expect(read.refusal.message).toContain('connection refused')
    })
  })

  it('a_declared_gap_is_not_marked_retryable', () => {
    const transport = new RecordingTransport({
      summary: { kind: 'declared_gap', requirement: 'QM-0152', message: 'not wired' },
    })
    return loadSummary(transport, 'run-a').then((read) => {
      expect(read.ok).toBe(false)
      if (read.ok) return
      expect('retryable' in read.refusal && read.refusal.retryable).toBeFalsy()
    })
  })
})

describe('QM-0150 the HTTP transport', () => {
  it('the_summary_request_goes_to_the_summary_route_and_carries_no_body', () => {
    const seen: { url: string; init?: RequestInit }[] = []
    const transport = new HttpTransport('http://127.0.0.1:9', async (url, init) => {
      seen.push({ url: String(url), init })
      return new Response(fixtureText('summary.v1.json'), { status: 200 })
    })
    return transport.fetchSummary('run-a').then((result) => {
      expect(result.kind).toBe('body')
      expect(seen).toHaveLength(1)
      expect(seen[0].url).toBe('http://127.0.0.1:9/v1/diagnostics/run-a/summary')
      expect(transport.requestLog).toEqual(['/v1/diagnostics/run-a/summary'])
    })
  })

  it('a_501_is_read_as_a_declared_gap_carrying_its_requirement_id', () => {
    // The CESIUM-003 discipline: a 501 is a boundary, not a failure to retry.
    const transport = new HttpTransport('http://127.0.0.1:9', async () =>
      new Response(JSON.stringify({ error: 'not_implemented', requirement: 'REP-001', message: 'x' }), {
        status: 501,
      }),
    )
    return transport.fetchSummary('run-a').then((result) => {
      expect(result.kind).toBe('declared_gap')
      if (result.kind !== 'declared_gap') return
      expect(result.requirement).toBe('REP-001')
    })
  })

  it('a_404_is_a_named_failure_naming_the_run', () => {
    const transport = new HttpTransport('http://127.0.0.1:9', async () =>
      new Response('{}', { status: 404 }),
    )
    return transport.fetchSummary('missing-run').then((result) => {
      expect(result.kind).toBe('failure')
      if (result.kind !== 'failure') return
      expect(result.message).toContain('missing-run')
      expect(result.message).toContain('404')
    })
  })

  it('a_thrown_network_error_is_a_failure_value_rather_than_an_escaping_exception', () => {
    const transport = new HttpTransport('http://127.0.0.1:9', async () => {
      throw new TypeError('fetch failed')
    })
    return transport.fetchSummary('run-a').then((result) => {
      expect(result.kind).toBe('failure')
      if (result.kind !== 'failure') return
      expect(result.message).toContain('fetch failed')
    })
  })

  it('per_layer_detail_over_http_is_a_declared_gap_rather_than_a_full_manifest_download', () => {
    // `.plan/API_CONTRACTS.md` defines no per-layer detail route. Falling back
    // to GET /v1/diagnostics/{runId} would pull the whole per-tensor array into
    // the browser, which ARCHITECTURE.md §19 forbids. QM-0152 wires this.
    let fetches = 0
    const transport = new HttpTransport('http://127.0.0.1:9', async () => {
      fetches += 1
      return new Response('{}', { status: 200 })
    })
    return transport.fetchLayerDetail('run-a', 3).then((result) => {
      expect(result.kind).toBe('declared_gap')
      if (result.kind !== 'declared_gap') return
      expect(result.requirement).toBe('QM-0152')
      expect(result.message).toContain('§19')
      expect(fetches, 'the declared gap must not have issued a request').toBe(0)
    })
  })
})
