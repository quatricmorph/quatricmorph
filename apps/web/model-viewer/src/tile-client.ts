/**
 * Daemon client for the viewer — Visualization Plane.
 *
 * The endpoints of ARCHITECTURE.md §14.4 and the fact that three of them are
 * not built. A 501 is modelled as a value, not an exception: the viewer must
 * distinguish "this feature does not exist yet" from "the request failed" and
 * show the difference rather than retrying forever.
 */

export type NotImplemented = {
  kind: 'not_implemented'
  requirement: string
  message: string
  documentation?: string
}

export type Fetched<T> = { kind: 'ok'; value: T } | NotImplemented

export type TilesetUri = string

/** Endpoint paths, kept in one place so the daemon and viewer cannot drift. */
export const ENDPOINTS = {
  models: '/v1/models',
  model: (id: string) => `/v1/models/${id}`,
  layers: (id: string) => `/v1/models/${id}/layers`,
  tensor: (id: string) => `/v1/tensors/${id}`,
  value: (id: string, index: number[]) =>
    `/v1/tensors/${id}/value?index=${index.join(',')}`,
  blocks: (id: string, rows: [number, number], columns: [number, number]) =>
    `/v1/tensors/${id}/blocks?rows=${rows[0]}:${rows[1]}&columns=${columns[0]}:${columns[1]}`,
  tileset: (modelId: string) => `/v1/visualizations/${modelId}/tileset.json`,
  query: '/v1/query',
} as const

/** Interpret a daemon response body as either a value or a declared gap. */
export function interpret<T>(status: number, body: unknown): Fetched<T> {
  if (status === 501) {
    const b = body as { requirement?: string; message?: string; documentation?: string }
    return {
      kind: 'not_implemented',
      requirement: b?.requirement ?? 'unknown',
      message: b?.message ?? 'not implemented',
      documentation: b?.documentation,
    }
  }
  if (status >= 400) {
    throw new Error(
      `daemon returned ${status}: ${JSON.stringify(body)}`,
    )
  }
  return { kind: 'ok', value: body as T }
}

/**
 * Whether the viewer can render a model at all.
 *
 * Today: never. `tileset.json` is a 501 (`CESIUM-001`), and there is nothing
 * honest to draw without one.
 */
export function canRenderTileset(tileset: Fetched<TilesetUri>): boolean {
  return tileset.kind === 'ok'
}
