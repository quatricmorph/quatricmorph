import { afterAll, describe, expect, it } from 'vitest'
import { existsSync, globSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// QM-0006 — regression guard for the class of breakage introduced by commit
// 103297d, which rewrote every *reference* from `matrix-workspace` to
// `quatricmorph-workspace` across 57 files but never renamed the directory.
//
// Nothing failed. `npx vitest run` reported `Test Files 3 passed (3) /
// Tests 27 passed (27)` at exit 0 while silently collecting none of the nine
// test files under the workspace, and `npm run build --workspace
// quatricmorph-workspace` exited 1 with "No workspaces found".
//
// A configuration that points at a path which does not exist must fail loudly.
// These tests assert two consistency properties between the checked-in
// configuration and the filesystem:
//
//   1. every entry in apps/web/package.json "workspaces" resolves to a
//      directory that contains a package.json;
//   2. every `include` glob in apps/web/vitest.config.ts matches at least one
//      file on disk.
//
// No network access. Every path is read from the working tree or from a
// hand-built fixture tree under the OS temp directory.

// This file lives at <apps/web>/<workspace>/src/util/__tests__/ — four levels
// below apps/web. The depth is asserted below rather than trusted.
const HERE = dirname(fileURLToPath(import.meta.url))
const WEB_ROOT = resolve(HERE, '..', '..', '..', '..')

/** Hand-written expectation. Not read from any file this test validates. */
const EXPECTED_WORKSPACES = ['quatricmorph-workspace', 'model-viewer', 'query-interface']
const EXPECTED_INCLUDE_GLOB_COUNT = 4
const RENAMED_AWAY_FROM = 'matrix-workspace'

interface WebPackageJson {
  name: string
  workspaces: string[]
}

function readWebPackageJson(): WebPackageJson {
  return JSON.parse(readFileSync(join(WEB_ROOT, 'package.json'), 'utf8')) as WebPackageJson
}

/**
 * Pull the `include` globs out of vitest.config.ts as text.
 *
 * Deliberately not `import()`: this test also has to run from inside the
 * workspace package, where apps/web/vitest.config.ts sits outside Vite's
 * server root and may not be importable. Reading bytes has no such limit.
 */
function extractIncludeGlobs(configSource: string): string[] {
  const block = /include\s*:\s*\[([\s\S]*?)\]/.exec(configSource)
  if (block === null) {
    throw new Error('vitest.config.ts has no `include: [...]` array; this test can no longer validate it')
  }
  return [...block[1].matchAll(/['"`]([^'"`]+)['"`]/g)].map((m) => m[1])
}

/** Workspace entries that do not resolve to a directory holding a package.json. */
function unresolvedWorkspaces(root: string, entries: readonly string[]): string[] {
  return entries.filter((entry) => !existsSync(join(root, entry, 'package.json')))
}

/** Include globs that match no file on disk. */
function globsMatchingNothing(root: string, globs: readonly string[]): string[] {
  return globs.filter((glob) => globSync(glob, { cwd: root }).length === 0)
}

const fixtureRoots: string[] = []

function makeFixtureTree(files: readonly string[], bareDirs: readonly string[] = []): string {
  const root = mkdtempSync(join(tmpdir(), 'qm0006-'))
  fixtureRoots.push(root)
  for (const dir of bareDirs) {
    mkdirSync(join(root, dir), { recursive: true })
  }
  for (const file of files) {
    const full = join(root, file)
    mkdirSync(dirname(full), { recursive: true })
    writeFileSync(full, '// fixture\n')
  }
  return root
}

afterAll(() => {
  for (const root of fixtureRoots) {
    rmSync(root, { recursive: true, force: true })
  }
})

describe('QM-0006 web workspace paths', () => {
  it('apps_web_is_four_directory_levels_above_this_test_file', () => {
    // If this fails, every path below is measured from the wrong directory and
    // the rest of this file proves nothing.
    const pkg = readWebPackageJson()
    expect(pkg.name).toBe('quatricmorph-web')
  })

  it('every_workspace_path_resolves_on_disk', () => {
    const { workspaces } = readWebPackageJson()
    const offenders = unresolvedWorkspaces(WEB_ROOT, workspaces)
    expect(
      offenders,
      `apps/web/package.json declares workspaces that do not exist on disk: ${JSON.stringify(offenders)}`,
    ).toEqual([])
  })

  it('every_vitest_include_glob_matches_at_least_one_file', () => {
    const globs = extractIncludeGlobs(readFileSync(join(WEB_ROOT, 'vitest.config.ts'), 'utf8'))
    const offenders = globsMatchingNothing(WEB_ROOT, globs)
    expect(
      offenders,
      `apps/web/vitest.config.ts include globs that match nothing: ${JSON.stringify(offenders)}`,
    ).toEqual([])
  })

  it('the_workspaces_array_names_exactly_the_three_web_applications', () => {
    expect(readWebPackageJson().workspaces).toEqual(EXPECTED_WORKSPACES)
  })

  it('the_vitest_config_still_declares_the_include_globs_this_test_reads', () => {
    // Guards the extraction itself: if vitest.config.ts is refactored so the
    // regex above stops matching, the glob check would pass vacuously over an
    // empty list. Fail here instead.
    const globs = extractIncludeGlobs(readFileSync(join(WEB_ROOT, 'vitest.config.ts'), 'utf8'))
    expect(globs).toHaveLength(EXPECTED_INCLUDE_GLOB_COUNT)
    for (const glob of globs) {
      expect(glob.endsWith('.test.ts')).toBe(true)
    }
  })

  it('the_pre_rename_workspace_directory_is_gone', () => {
    expect(
      existsSync(join(WEB_ROOT, RENAMED_AWAY_FROM)),
      `apps/web/${RENAMED_AWAY_FROM} still exists; the rename begun by 103297d is incomplete`,
    ).toBe(false)
  })

  it('the_workspace_package_name_matches_its_directory_name', () => {
    // 103297d's sed ran twice over a name that already carried the prefix,
    // leaving "quatricmorph-quatricmorph-workspace".
    const manifest = join(WEB_ROOT, 'quatricmorph-workspace', 'package.json')
    expect(existsSync(manifest), `${manifest} does not exist`).toBe(true)
    const pkg = JSON.parse(readFileSync(manifest, 'utf8')) as { name: string }
    expect(pkg.name).toBe('quatricmorph-workspace')
  })
})

describe('QM-0006 workspace path checks report their offenders', () => {
  it('a_workspace_entry_pointing_at_a_missing_directory_is_reported_by_name', () => {
    const root = makeFixtureTree(['present/package.json'])
    expect(unresolvedWorkspaces(root, ['present', 'does-not-exist-workspace'])).toEqual([
      'does-not-exist-workspace',
    ])
  })

  it('a_workspace_entry_without_a_package_json_is_reported_by_name', () => {
    // The exact shape of the bug: the directory name is right but nothing is
    // there to install or build.
    const root = makeFixtureTree(['present/package.json'], ['empty-dir'])
    expect(unresolvedWorkspaces(root, ['present', 'empty-dir'])).toEqual(['empty-dir'])
  })

  it('every_workspace_entry_resolving_reports_no_offenders', () => {
    const root = makeFixtureTree(['a/package.json', 'b/package.json'])
    expect(unresolvedWorkspaces(root, ['a', 'b'])).toEqual([])
  })

  it('a_vitest_include_glob_matching_nothing_is_reported_by_name', () => {
    // Hand-built tree: exactly two .test.ts files, both under real/src.
    const root = makeFixtureTree([
      'real/src/one.test.ts',
      'real/src/nested/__tests__/two.test.ts',
    ])
    const globs = ['real/src/**/*.test.ts', 'no-such-dir/**/*.test.ts']
    expect(globsMatchingNothing(root, globs)).toEqual(['no-such-dir/**/*.test.ts'])
  })

  it('the_glob_matcher_finds_every_file_in_a_hand_built_fixture_tree', () => {
    // Expected values counted by hand from the literal list above, not
    // produced by the matcher.
    const root = makeFixtureTree([
      'real/src/one.test.ts',
      'real/src/nested/__tests__/two.test.ts',
      'real/src/nested/not-a-test.ts',
    ])
    expect(globSync('real/src/**/*.test.ts', { cwd: root }).sort()).toEqual([
      join('real', 'src', 'nested', '__tests__', 'two.test.ts'),
      join('real', 'src', 'one.test.ts'),
    ])
    expect(globsMatchingNothing(root, ['real/src/**/*.test.ts'])).toEqual([])
  })

  it('an_include_array_the_extractor_cannot_find_raises_rather_than_passing_vacuously', () => {
    expect(() => extractIncludeGlobs('export default { test: { environment: "node" } }')).toThrow(
      /no `include: \[\.\.\.\]` array/,
    )
  })

  it('the_extractor_reads_the_globs_verbatim_from_config_text', () => {
    const source = [
      'export default defineConfig({',
      '  test: {',
      "    include: ['a/**/*.test.ts', \"b/**/*.test.ts\"],",
      '  },',
      '})',
    ].join('\n')
    expect(extractIncludeGlobs(source)).toEqual(['a/**/*.test.ts', 'b/**/*.test.ts'])
  })
})
