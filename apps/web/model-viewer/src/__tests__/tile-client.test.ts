import { describe, expect, it } from 'vitest'
import { canRenderTileset, ENDPOINTS, interpret } from '../tile-client.js'

describe('CESIUM-003 daemon client', () => {
  it('builds the endpoints of ARCHITECTURE.md §14', () => {
    expect(ENDPOINTS.value('abc', [100, 42])).toBe('/v1/tensors/abc/value?index=100,42')
    expect(ENDPOINTS.blocks('abc', [0, 256], [0, 256])).toBe(
      '/v1/tensors/abc/blocks?rows=0:256&columns=0:256',
    )
    expect(ENDPOINTS.tileset('m')).toBe('/v1/visualizations/m/tileset.json')
  })

  it('treats a 501 as a declared gap, not a failure to retry', () => {
    const r = interpret<string>(501, {
      requirement: 'CESIUM-001',
      message: 'tileset.json is not generated',
      documentation: 'ARCHITECTURE.md §9, §10',
    })
    expect(r.kind).toBe('not_implemented')
    if (r.kind === 'not_implemented') {
      expect(r.requirement).toBe('CESIUM-001')
      expect(r.documentation).toContain('ARCHITECTURE.md')
    }
  })

  it('still throws on real errors', () => {
    expect(() => interpret(500, { error: 'internal' })).toThrow(/500/)
    expect(() => interpret(404, { error: 'not_found' })).toThrow(/404/)
  })

  it('reports that nothing can be rendered while the tileset is a 501', () => {
    const tileset = interpret<string>(501, { requirement: 'CESIUM-001' })
    expect(canRenderTileset(tileset)).toBe(false)
    expect(canRenderTileset(interpret<string>(200, '/tiles/tileset.json'))).toBe(true)
  })
})
