import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { buildSurface } from '../app.js'
import { readManifest, type Manifest } from '../manifest-client.js'
import { refusalToSvg, surfaceToSvg } from '../render.js'
import { fixtureText } from './fixtures.js'

// QM-0150 — the visual evidence.
//
// The task's completion evidence asks for screenshots in colour and greyscale.
// No browser is available in this environment, so the artifact is an SVG
// produced by `render.ts` — the same draw plan the 2-D canvas painter consumes,
// asserted elsewhere in this suite to pick the same colours. It is committed
// under `artifacts/`, and this file asserts the committed bytes still match
// what the renderer produces, so the evidence cannot go stale while the code
// moves under it.
//
// Regenerate after a deliberate rendering change:
//
//   cd apps/web && QM_WRITE_ARTIFACTS=1 npx vitest run diagnostics/src/__tests__/artifacts.test.ts

const HERE = dirname(fileURLToPath(import.meta.url))
const ARTIFACT_DIR = resolve(HERE, '..', '..', 'artifacts')
const WRITE = process.env.QM_WRITE_ARTIFACTS === '1'

function manifestFrom(name: string, projection: 'summary' | 'full' = 'summary'): Manifest {
  const read = readManifest(fixtureText(name), { projection })
  if (!read.ok) throw new Error(`fixture ${name} did not read: ${read.refusal.message}`)
  return read.value
}

function refusalFrom(name: string): string {
  const read = readManifest(fixtureText(name), { projection: 'summary' })
  if (read.ok) throw new Error(`fixture ${name} was expected to be refused`)
  return refusalToSvg(read.refusal)
}

/** Name → the SVG it must contain. Every entry is committed under `artifacts/`. */
const ARTIFACTS: Record<string, () => string> = {
  // The default view: layer rows at the resolution a summary manifest publishes.
  'summary-colour.svg': () => surfaceToSvg(buildSurface(manifestFrom('summary.v1.json')), { palette: 'colour' }),
  'summary-greyscale.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('summary.v1.json')), { palette: 'greyscale' }),
  // Drill-down: tensor columns with the channel extents their shapes declare.
  'tensor-colour.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('full.v1.json', 'full')), { palette: 'colour' }),
  'tensor-greyscale.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('full.v1.json', 'full')), { palette: 'greyscale' }),
  // The same manifest under a cell ceiling small enough to force aggregation.
  // Both palettes: QM-0153's evidence is that the degraded state is legible in
  // colour *and* in greyscale, and one image cannot show that.
  'aggregated-greyscale.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('full.v1.json', 'full'), { cellCeiling: 3 }), {
      palette: 'greyscale',
    }),
  'aggregated-colour.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('full.v1.json', 'full'), { cellCeiling: 3 }), {
      palette: 'colour',
    }),
  // Nothing was measured: an explanation, not an empty grid.
  'empty-colour.svg': () => surfaceToSvg(buildSurface(manifestFrom('summary.empty.json')), { palette: 'colour' }),
  // A manifest this build will not read: refused, not partially rendered.
  'refused-version-2.svg': () => refusalFrom('version-2.json'),
  // A sampled run: never dressed up as an exact one. Both layers measure the
  // same value, so this is also the single-valued map — one legend entry that
  // says so, rather than six identical tiers all reading "fill 100%".
  'sampled-greyscale.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('summary.sampled.json')), { palette: 'greyscale' }),
  // A sampled run with an expert panel — the one panel no artifact used to
  // show, and the one a reviewer defeated the labelling test through.
  'sampled-experts-colour.svg': () =>
    surfaceToSvg(buildSurface(manifestFrom('summary.sampled-experts.json')), { palette: 'colour' }),
}

describe('QM-0150 the committed visual artifacts are what the renderer produces now', () => {
  for (const [name, render] of Object.entries(ARTIFACTS)) {
    it(`the_committed_${name.replace(/[-.]/g, '_')}_matches_the_current_renderer`, () => {
      const path = join(ARTIFACT_DIR, name)
      const rendered = render()
      if (WRITE) {
        mkdirSync(ARTIFACT_DIR, { recursive: true })
        writeFileSync(path, rendered)
      }
      expect(existsSync(path), `${path} is not committed`).toBe(true)
      expect(readFileSync(path, 'utf8')).toBe(rendered)
    })
  }

  it('every_artifact_is_an_svg_document_a_reviewer_can_open', () => {
    for (const name of Object.keys(ARTIFACTS)) {
      const svg = readFileSync(join(ARTIFACT_DIR, name), 'utf8')
      expect(svg.startsWith('<svg'), `${name} is not an SVG`).toBe(true)
      expect(svg.trimEnd().endsWith('</svg>'), `${name} is truncated`).toBe(true)
    }
  })

  it('a_greyscale_artifact_and_its_colour_twin_differ_only_in_their_fills', () => {
    const colour = readFileSync(join(ARTIFACT_DIR, 'summary-colour.svg'), 'utf8')
    const grey = readFileSync(join(ARTIFACT_DIR, 'summary-greyscale.svg'), 'utf8')
    expect(colour).not.toBe(grey)
    const strip = (svg: string) => svg.replace(/fill="#[0-9a-f]{6}"/g, 'fill="#"')
    expect(strip(colour)).toBe(strip(grey))
  })

  it('the_refusal_artifact_contains_no_heat_map_cell', () => {
    const svg = readFileSync(join(ARTIFACT_DIR, 'refused-version-2.svg'), 'utf8')
    expect(svg).not.toContain('data-fill-fraction')
  })
})
