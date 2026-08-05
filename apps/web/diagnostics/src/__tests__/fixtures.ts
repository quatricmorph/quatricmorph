/**
 * Fixture access for the diagnostics tests.
 *
 * **No test in this package touches the network.** Every manifest a test reads
 * is either a checked-in file under `fixtures/`, or one of `q-report`'s own
 * golden manifests read from the working tree — the latter deliberately, so
 * that a drift between this package's TypeScript types and the Rust producer
 * turns a test red rather than turning into a wrong picture in a browser.
 */

import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))

/** `apps/web/diagnostics/src/__tests__` → repository root is five levels up. */
export const REPO_ROOT = resolve(HERE, '..', '..', '..', '..', '..')
/** `apps/web/diagnostics` — the package's own root, where `index.html` lives. */
export const PACKAGE_ROOT = resolve(HERE, '..', '..')
export const FIXTURE_DIR = join(HERE, 'fixtures')

/** The published schema, read from the repository — never copied into this package. */
export const PUBLISHED_SCHEMA_PATH = join(REPO_ROOT, 'schemas', 'diagnostics', 'manifest.v1.json')

/** `q-report`'s golden manifests — the producer's own bytes. */
export const PRODUCER_GOLDEN_DIR = join(REPO_ROOT, 'crates', 'q-report', 'tests', 'golden')

export function fixtureText(name: string): string {
  return readFileSync(join(FIXTURE_DIR, name), 'utf8')
}

export function fixtureJson<T = unknown>(name: string): T {
  return JSON.parse(fixtureText(name)) as T
}

export function producerGoldenText(name: string): string {
  return readFileSync(join(PRODUCER_GOLDEN_DIR, name), 'utf8')
}

/** A file this package ships, read from the working tree — `index.html`, say. */
export function packageFileText(name: string): string {
  return readFileSync(join(PACKAGE_ROOT, name), 'utf8')
}

/** A structural clone, so a mutating negative-path test cannot poison another. */
export function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}
