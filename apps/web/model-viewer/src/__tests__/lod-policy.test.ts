import { describe, expect, it } from 'vitest'
import {
  decideLoad,
  geometricErrorForLod,
  Lod,
  lodForDistance,
  type Interaction,
} from '../lod-policy.js'

describe('CESIUM-002 LOD loading policy', () => {
  it('maps camera distance onto the six-level ladder', () => {
    expect(lodForDistance(100_000)).toBe(Lod.Model)
    expect(lodForDistance(2000)).toBe(Lod.Subsystem)
    expect(lodForDistance(500)).toBe(Lod.Layer)
    expect(lodForDistance(100)).toBe(Lod.Tensor)
    expect(lodForDistance(20)).toBe(Lod.Block)
    expect(lodForDistance(1)).toBe(Lod.ScalarRegion)
  })

  it('never reads exact values from camera movement alone', () => {
    // ARCHITECTURE.md §18 AC-006 / AC-007: zooming out must not load exact
    // values, and zooming in must read only the byte ranges it needs.
    const camera = { distance: 0.5, screenSpaceErrorTolerance: 16 }
    for (const interaction of ['idle', 'navigating', 'hovering'] as Interaction[]) {
      const d = decideLoad(camera, interaction)
      expect(d.fetchExactValues).toBe(false)
    }
  })

  it('reads exact values only on an explicit selection', () => {
    const d = decideLoad({ distance: 4096, screenSpaceErrorTolerance: 16 }, 'selected')
    expect(d.fetchExactValues).toBe(true)
    expect(d.lod).toBe(Lod.ScalarRegion)
    // ...and distance does not matter: selection wins over camera position.
    expect(decideLoad({ distance: 1e9, screenSpaceErrorTolerance: 16 }, 'selected').fetchExactValues).toBe(true)
  })

  it('does not prefetch while the camera is still moving', () => {
    const camera = { distance: 500, screenSpaceErrorTolerance: 16 }
    expect(decideLoad(camera, 'navigating').prefetchChildren).toBe(false)
    expect(decideLoad(camera, 'idle').prefetchChildren).toBe(true)
  })

  it('does not prefetch below the finest level', () => {
    const d = decideLoad({ distance: 0.1, screenSpaceErrorTolerance: 16 }, 'idle')
    expect(d.lod).toBe(Lod.ScalarRegion)
    expect(d.prefetchChildren).toBe(false)
  })

  it('geometric error decreases monotonically, matching q-tileset', () => {
    let prev = Infinity
    for (const lod of [Lod.Model, Lod.Subsystem, Lod.Layer, Lod.Tensor, Lod.Block, Lod.ScalarRegion]) {
      const e = geometricErrorForLod(lod)
      expect(e).toBeLessThan(prev)
      prev = e
    }
    expect(geometricErrorForLod(Lod.Model)).toBe(1024)
    expect(geometricErrorForLod(Lod.ScalarRegion)).toBe(32)
  })
})
