/**
 * Reading a diagnostics manifest into the surface.
 *
 * Three rules govern this file.
 *
 * 1. **Refuse, do not guess.** An unknown `manifest_version`, a document that
 *    fails the published schema, a projection other than the one asked for —
 *    each is a value describing why nothing will be rendered, never a partial
 *    render. `CAT-002`'s idiom, in TypeScript.
 * 2. **The summary projection is the initial payload.** `ARCHITECTURE.md` §19
 *    forbids sending a whole tensor into the browser, and `manifest.v1.json`
 *    says the per-tensor array is "tens of megabytes … not acceptable to push
 *    into a browser wholesale". The initial view asks for
 *    `/v1/diagnostics/{runId}/summary` and nothing else.
 * 3. **A declared gap is not a failure.** `CESIUM-003`: a 501 carries a
 *    requirement ID, is shown as a boundary, and is never retried.
 */

import { MANIFEST_SCHEMA_V1, SUPPORTED_MANIFEST_VERSION } from './schema.js'
import { validate, type ValidationError } from './schema-validator.js'

export { SUPPORTED_MANIFEST_VERSION }

/**
 * The largest manifest body this surface will read.
 *
 * The browser-side analogue of `assertBlockIsBounded`'s element ceiling
 * (`GRID-005`). The summary projection of a checkpoint with tens of thousands
 * of tensors is well under this; anything above it is the full per-tensor array
 * arriving where it must not.
 */
export const MAX_MANIFEST_PAYLOAD_BYTES = 4 * 1024 * 1024

export type Fidelity = 'exact' | 'sampled' | 'approximate'
export type Projection = 'full' | 'summary'

/**
 * Composable partials for one level of the aggregation hierarchy.
 *
 * The manifest deliberately stores partials rather than finished metrics: sums
 * of squares compose across blocks and RMSE does not. The consumer finishes the
 * arithmetic, which is what `relativeErrorOf` and `rmseOf` below do.
 */
export type ErrorAggregate = {
  count: number
  sum_sq_base: number
  sum_sq_delta: number
  sum_abs_delta: number
  max_abs_delta: number
  bytes_at_base_precision: number
  bytes_at_target_precision: number
}

export type LayerEntry = { layer_index: number; aggregate: ErrorAggregate }
export type ExpertEntry = { layer_index: number; expert_index: number; aggregate: ErrorAggregate }

export type TensorEntry = {
  address: string
  role: string
  dtype: string
  shape: number[]
  aggregate: ErrorAggregate
  outlier_attribution?: { top_0_1_percent_share: number; top_1_percent_share: number }
}

export type RankingEntry = { address: string; relative_error: number; parameter_count: number }
export type FrontierStep = { keep_set: string[]; added_bytes: number; error_removed_fraction: number }
export type Frontier = { method: string; claim: string; steps: FrontierStep[] }
export type RefusalEntry = { requirement_id: string; what: string; why: string }

export type Manifest = {
  manifest_version: number
  projection: Projection
  run: {
    run_id: string
    engine_version: string
    backend: 'cpu' | 'metal'
    started_at: string
    elapsed_seconds: number
    peak_resident_bytes: number
    bytes_read: number
  }
  model: {
    model_id: string
    source_uri: string
    revision_hash: string
    checkpoint_bytes: number
    parameter_count: number
    architecture: string
    resolver_confidence: 'resolved' | 'unknown'
  }
  config: {
    precision: 'int8' | 'int4'
    granularity: { kind: 'per_tensor' | 'per_output_channel' | 'per_group'; group_size?: number }
    zero_point: 'symmetric' | 'asymmetric'
    round: 'nearest_even'
    block_rows: number
    block_columns: number
    resident_ceiling_bytes: number
  }
  totals: ErrorAggregate
  layers: LayerEntry[]
  experts: ExpertEntry[]
  tensors?: TensorEntry[]
  ranking: RankingEntry[]
  frontier: Frontier
  fidelity: Fidelity
  refusals: RefusalEntry[]
}

/**
 * Why nothing will be rendered.
 *
 * Each variant carries enough for the surface to say what happened without
 * inventing a substitute picture.
 */
export type Refusal =
  | { kind: 'malformed_json'; message: string }
  | { kind: 'unsupported_version'; found: number; supported: number; message: string }
  | { kind: 'schema_invalid'; errors: ValidationError[]; message: string }
  | { kind: 'wrong_projection'; expected: Projection; found: Projection; message: string }
  | { kind: 'payload_too_large'; bytes: number; ceilingBytes: number; message: string }
  | { kind: 'declared_gap'; requirement: string; message: string }
  | { kind: 'transport_failure'; retryable: boolean; message: string }

export type Read<T> = { ok: true; value: T } | { ok: false; refusal: Refusal }

/**
 * Daemon routes.
 *
 * `summary` and `full` are the routes `.plan/API_CONTRACTS.md` records for
 * `QM-0143`. `layerDetail` is **proposed, not defined**: no per-layer route
 * exists yet, and `HttpTransport.fetchLayerDetail` refuses rather than falling
 * back to `full`, which would pull the whole per-tensor array into the browser.
 * The string exists so a request log has a stable key to assert against.
 */
export const ENDPOINTS = {
  summary: (runId: string) => `/v1/diagnostics/${runId}/summary`,
  full: (runId: string) => `/v1/diagnostics/${runId}`,
  layerDetail: (runId: string, layerIndex: number) => `/v1/diagnostics/${runId}/layers/${layerIndex}`,
} as const

/** sqrt(sum_sq_delta / sum_sq_base), or `null` when there is nothing to divide by. */
export function relativeErrorOf(aggregate: ErrorAggregate): number | null {
  if (!(aggregate.sum_sq_base > 0)) return null
  return Math.sqrt(aggregate.sum_sq_delta / aggregate.sum_sq_base)
}

/** sqrt(sum_sq_delta / count), or `null` when nothing was reduced. */
export function rmseOf(aggregate: ErrorAggregate): number | null {
  if (!(aggregate.count > 0)) return null
  return Math.sqrt(aggregate.sum_sq_delta / aggregate.count)
}

/**
 * Read a manifest body.
 *
 * Order matters. The size ceiling comes first so an oversized body is never
 * parsed; the version check comes before schema validation so that a future
 * manifest is refused by *version*, naming both, rather than by a `const`
 * mismatch a reader cannot act on.
 */
export function readManifest(text: string, expect: { projection: Projection }): Read<Manifest> {
  const bytes = byteLength(text)
  if (bytes > MAX_MANIFEST_PAYLOAD_BYTES) {
    return refuse({
      kind: 'payload_too_large',
      bytes,
      ceilingBytes: MAX_MANIFEST_PAYLOAD_BYTES,
      message:
        `manifest body is ${bytes} bytes, above this surface's ${MAX_MANIFEST_PAYLOAD_BYTES}-byte ceiling. ` +
        `Load the summary projection: pushing a whole per-tensor manifest into the browser is what ` +
        `ARCHITECTURE.md §19 forbids.`,
    })
  }

  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch (error) {
    return refuse({
      kind: 'malformed_json',
      message: `the manifest body is not JSON: ${error instanceof Error ? error.message : String(error)}`,
    })
  }

  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return refuse({
      kind: 'malformed_json',
      message: `the manifest body is a ${parsed === null ? 'null' : Array.isArray(parsed) ? 'array' : typeof parsed}, not a JSON object`,
    })
  }

  const document = parsed as Record<string, unknown>
  const version = document.manifest_version
  if (typeof version !== 'number' || version !== SUPPORTED_MANIFEST_VERSION) {
    const found = typeof version === 'number' ? version : Number.NaN
    return refuse({
      kind: 'unsupported_version',
      found,
      supported: SUPPORTED_MANIFEST_VERSION,
      message:
        `manifest_version ${typeof version === 'number' ? version : JSON.stringify(version)} is not the ` +
        `version this build reads (${SUPPORTED_MANIFEST_VERSION}). Nothing is rendered: a reader that ` +
        `guesses at an unknown layout produces a plausible wrong answer.`,
    })
  }

  const errors = validate(MANIFEST_SCHEMA_V1, document)
  if (errors.length > 0) {
    return refuse({
      kind: 'schema_invalid',
      errors,
      message:
        `the manifest does not satisfy schemas/diagnostics/manifest.v1.json ` +
        `(${errors.length} problem${errors.length === 1 ? '' : 's'}): ` +
        errors
          .slice(0, 4)
          .map((e) => e.message)
          .join('; '),
    })
  }

  const manifest = document as unknown as Manifest
  if (manifest.projection !== expect.projection) {
    return refuse({
      kind: 'wrong_projection',
      expected: expect.projection,
      found: manifest.projection,
      message:
        `expected the ${expect.projection} projection but the body is ${manifest.projection}. ` +
        `A full manifest arriving where a summary was asked for means the per-tensor array reached ` +
        `the browser (ARCHITECTURE.md §19).`,
    })
  }

  return { ok: true, value: manifest }
}

export type TransportResult =
  | { kind: 'body'; text: string }
  | { kind: 'declared_gap'; requirement: string; message: string }
  | { kind: 'failure'; message: string }

/** Where manifest bodies come from. Injected, so no test needs a network. */
export interface ManifestTransport {
  readonly requestLog: readonly string[]
  fetchSummary(runId: string): Promise<TransportResult>
  fetchLayerDetail(runId: string, layerIndex: number): Promise<TransportResult>
}

/** The initial view. One request, to the summary route. */
export async function loadSummary(
  transport: ManifestTransport,
  runId: string,
): Promise<Read<Manifest>> {
  return interpret(await transport.fetchSummary(runId), 'summary')
}

/** Per-layer detail. Called only from an explicit user action, never on load. */
export async function loadLayerDetail(
  transport: ManifestTransport,
  runId: string,
  layerIndex: number,
): Promise<Read<Manifest>> {
  return interpret(await transport.fetchLayerDetail(runId, layerIndex), 'full')
}

function interpret(result: TransportResult, projection: Projection): Read<Manifest> {
  switch (result.kind) {
    case 'body':
      return readManifest(result.text, { projection })
    case 'declared_gap':
      return refuse({
        kind: 'declared_gap',
        requirement: result.requirement,
        message: result.message,
      })
    case 'failure':
      return refuse({ kind: 'transport_failure', retryable: true, message: result.message })
  }
}

/** The HTTP binding. `fetch` is a constructor argument so tests inject one. */
export class HttpTransport implements ManifestTransport {
  readonly requestLog: string[] = []

  constructor(
    private readonly baseUrl: string,
    private readonly fetchImpl: typeof fetch = (...args: Parameters<typeof fetch>) =>
      globalThis.fetch(...args),
  ) {}

  async fetchSummary(runId: string): Promise<TransportResult> {
    const route = ENDPOINTS.summary(runId)
    this.requestLog.push(route)
    let response: Response
    try {
      response = await this.fetchImpl(`${this.baseUrl}${route}`)
    } catch (error) {
      return {
        kind: 'failure',
        message: `could not reach the daemon at ${this.baseUrl}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      }
    }
    if (response.status === 501) {
      const body = (await response.json().catch(() => ({}))) as {
        requirement?: string
        message?: string
      }
      return {
        kind: 'declared_gap',
        requirement: body.requirement ?? 'unknown',
        message: body.message ?? 'the daemon declares this capability unbuilt',
      }
    }
    if (!response.ok) {
      return {
        kind: 'failure',
        message: `the daemon answered ${response.status} for run ${runId}`,
      }
    }
    return { kind: 'body', text: await response.text() }
  }

  /**
   * **Not wired** (`QM-0152`).
   *
   * `.plan/API_CONTRACTS.md` defines no per-layer detail route. The available
   * alternative — `GET /v1/diagnostics/{runId}` — returns the whole per-tensor
   * array, which is exactly the payload ARCHITECTURE.md §19 forbids pushing
   * into a browser. So this issues no request at all rather than issuing the
   * wrong one; `QM-0152` adds the route and the binding together.
   */
  async fetchLayerDetail(runId: string, layerIndex: number): Promise<TransportResult> {
    return {
      kind: 'declared_gap',
      requirement: 'QM-0152',
      message:
        `no per-layer detail route is defined for run ${runId} layer ${layerIndex}. Falling back to ` +
        `${ENDPOINTS.full(runId)} would download the whole per-tensor array, which ARCHITECTURE.md §19 ` +
        `forbids, so nothing was requested.`,
    }
  }
}

function refuse(refusal: Refusal): Read<never> {
  return { ok: false, refusal }
}

function byteLength(text: string): number {
  return new TextEncoder().encode(text).length
}
