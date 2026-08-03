/**
 * `model-viewer` entry point — Visualization Plane.
 *
 * Renders nothing. It reports what is and is not available, which is the only
 * honest thing this app can do until `tileset.json` generation exists
 * (`CESIUM-001`).
 */
export { Lod, decideLoad, lodForDistance, geometricErrorForLod } from './lod-policy.js'
export type { CameraState, Interaction, LoadDecision } from './lod-policy.js'
export { ENDPOINTS, interpret, canRenderTileset } from './tile-client.js'
export type { Fetched, NotImplemented } from './tile-client.js'
