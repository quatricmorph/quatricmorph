import { existsSync, globSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, describe, expect, it } from 'vitest'
import { buildSurface } from '../app.js'
import { ENDPOINTS, loadSummary, readManifest, type ManifestTransport, type TransportResult } from '../manifest-client.js'
import { surfaceToSvg } from '../render.js'
import { REPO_ROOT, fixtureText } from './fixtures.js'

// QM-0150 — the program boundary, and the registration that commit 103297d got
// wrong. `QM-0006` added the guard on the workspace side; this is the same
// assertion from the new package's side, so that removing `diagnostics` from
// either configuration file turns a test in the package itself red.

const HERE = dirname(fileURLToPath(import.meta.url))
const PACKAGE_ROOT = resolve(HERE, '..', '..')
const WEB_ROOT = resolve(PACKAGE_ROOT, '..')

const PACKAGE_DIRECTORY_NAME = 'diagnostics'
const EXPECTED_INCLUDE_GLOB = 'diagnostics/src/**/__tests__/**/*.test.ts'

/** Every `.ts` file this package ships, tests included. */
function packageSources(): string[] {
  return globSync('src/**/*.ts', { cwd: PACKAGE_ROOT }).map((relative) => join(PACKAGE_ROOT, relative))
}

/**
 * The files the forbidden-token scans below read.
 *
 * This file is excluded from its own scan: it necessarily contains every token
 * it forbids, as a string literal. The exclusion is asserted to be exactly one
 * file, so it cannot quietly grow into a hiding place.
 */
const SCAN_EXEMPT = 'boundary.test.ts'
function scannedSources(): string[] {
  return packageSources().filter((file) => !file.endsWith(SCAN_EXEMPT))
}

describe('QM-0150 the package is registered in both configuration files', () => {
  it('the_package_directory_name_matches_the_workspace_name_in_its_own_manifest', () => {
    const pkg = JSON.parse(readFileSync(join(PACKAGE_ROOT, 'package.json'), 'utf8')) as { name: string }
    expect(pkg.name).toBe(PACKAGE_DIRECTORY_NAME)
    expect(PACKAGE_ROOT.endsWith(PACKAGE_DIRECTORY_NAME)).toBe(true)
  })

  it('the_web_workspaces_array_names_this_package_and_the_directory_exists', () => {
    const web = JSON.parse(readFileSync(join(WEB_ROOT, 'package.json'), 'utf8')) as {
      workspaces: string[]
    }
    expect(web.workspaces).toContain(PACKAGE_DIRECTORY_NAME)
    expect(existsSync(join(WEB_ROOT, PACKAGE_DIRECTORY_NAME, 'package.json'))).toBe(true)
  })

  it('the_vitest_include_globs_name_this_package_and_the_glob_matches_this_very_file', () => {
    // The 103297d failure was a glob that matched nothing and a suite that
    // exited 0 anyway. If this package is ever unregistered, this assertion
    // is the one that notices.
    const config = readFileSync(join(WEB_ROOT, 'vitest.config.ts'), 'utf8')
    expect(config).toContain(EXPECTED_INCLUDE_GLOB)
    const matched = globSync(EXPECTED_INCLUDE_GLOB, { cwd: WEB_ROOT })
    expect(matched.length).toBeGreaterThan(0)
    expect(matched.some((p) => p.endsWith('boundary.test.ts'))).toBe(true)
  })

  it('the_package_ships_the_entry_points_the_task_expects', () => {
    for (const file of ['index.html', 'src/main.ts', 'src/app.ts', 'src/heatmap.ts', 'src/manifest-client.ts']) {
      expect(existsSync(join(PACKAGE_ROOT, file)), `${file} is missing`).toBe(true)
    }
  })
})

describe('QM-0150 the program boundary excludes the deferred platform renderer', () => {
  it('exactly_one_file_is_exempt_from_the_forbidden_token_scans', () => {
    expect(packageSources().length - scannedSources().length).toBe(1)
    expect(scannedSources().length).toBeGreaterThan(5)
  })

  it('no_source_in_this_package_imports_cesium_or_three', () => {
    const offenders: string[] = []
    for (const file of scannedSources()) {
      const source = readFileSync(file, 'utf8')
      for (const forbidden of ['cesium', 'three', '@types/three', 'resium']) {
        if (new RegExp(`from\\s+['"\`][^'"\`]*\\b${forbidden}\\b`, 'i').test(source)) {
          offenders.push(`${file}: ${forbidden}`)
        }
      }
    }
    expect(offenders).toEqual([])
  })

  it('this_packages_dependencies_name_no_3d_renderer', () => {
    // Scans the dependency maps and the scripts, not the whole file: the
    // package description names the excluded libraries on purpose, to record
    // the boundary, and prose that states an exclusion must not read as a
    // violation of it.
    const pkg = JSON.parse(readFileSync(join(PACKAGE_ROOT, 'package.json'), 'utf8')) as {
      dependencies?: Record<string, string>
      devDependencies?: Record<string, string>
      peerDependencies?: Record<string, string>
      optionalDependencies?: Record<string, string>
      scripts?: Record<string, string>
    }
    const declared = [
      ...Object.keys(pkg.dependencies ?? {}),
      ...Object.keys(pkg.devDependencies ?? {}),
      ...Object.keys(pkg.peerDependencies ?? {}),
      ...Object.keys(pkg.optionalDependencies ?? {}),
      ...Object.values(pkg.scripts ?? {}),
    ].map((entry) => entry.toLowerCase())

    expect(declared.length).toBeGreaterThan(0)
    for (const forbidden of ['cesium', 'three', 'resium', 'babylon', 'deck.gl']) {
      expect(
        declared.filter((entry) => entry.includes(forbidden)),
        `package.json declares ${forbidden}`,
      ).toEqual([])
    }
  })

  it('no_source_in_this_package_mentions_a_glb_or_a_tileset', () => {
    const offenders: string[] = []
    for (const file of scannedSources()) {
      const source = readFileSync(file, 'utf8')
      for (const forbidden of ['.glb', 'tileset.json', 'b3dm', 'WebGL', 'WebGPU']) {
        if (source.includes(forbidden)) offenders.push(`${file}: ${forbidden}`)
      }
    }
    expect(offenders).toEqual([])
  })

  it('the_schema_is_referenced_at_the_repository_path_and_not_copied_into_this_package', () => {
    // A copy is a second source of truth, and QM-0140 exists because a second
    // source of truth drifts from the Rust producer.
    expect(existsSync(join(REPO_ROOT, 'schemas', 'diagnostics', 'manifest.v1.json'))).toBe(true)
    const copies = globSync('**/manifest.v1.json', { cwd: PACKAGE_ROOT })
    expect(copies).toEqual([])
  })
})

describe('QM-0150 no test in this package touches the network', () => {
  const realFetch = globalThis.fetch

  afterEach(() => {
    globalThis.fetch = realFetch
  })

  it('the_whole_read_to_render_path_runs_with_the_global_fetch_disabled', async () => {
    globalThis.fetch = (() => {
      throw new Error('a diagnostics test attempted a network request')
    }) as typeof fetch

    const transport: ManifestTransport = {
      requestLog: [],
      async fetchSummary(): Promise<TransportResult> {
        return { kind: 'body', text: fixtureText('summary.v1.json') }
      },
      async fetchLayerDetail(): Promise<TransportResult> {
        return { kind: 'declared_gap', requirement: 'QM-0152', message: 'not wired' }
      },
    }

    const read = await loadSummary(transport, 'run-a')
    expect(read.ok).toBe(true)
    if (!read.ok) return
    const svg = surfaceToSvg(buildSurface(read.value), { palette: 'colour' })
    expect(svg.length).toBeGreaterThan(0)
  })

  it('the_only_global_fetch_call_site_in_this_package_is_the_injectable_one_in_the_manifest_client', () => {
    const offenders: string[] = []
    for (const file of scannedSources()) {
      const source = readFileSync(file, 'utf8')
      if (/\bglobalThis\.fetch\b|(?<![.\w])fetch\s*\(/.test(source) && !file.endsWith('manifest-client.ts')) {
        offenders.push(file)
      }
    }
    expect(offenders).toEqual([])
  })

  it('reading_a_manifest_is_a_pure_function_of_the_text_it_is_given', () => {
    globalThis.fetch = (() => {
      throw new Error('a diagnostics test attempted a network request')
    }) as typeof fetch
    expect(readManifest(fixtureText('summary.v1.json'), { projection: 'summary' }).ok).toBe(true)
    expect(ENDPOINTS.summary('r')).toBe('/v1/diagnostics/r/summary')
  })
})
