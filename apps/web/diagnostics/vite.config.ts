import { defineConfig } from 'vite'
import { resolve } from 'node:path'

// The schema is imported from `schemas/diagnostics/manifest.v1.json` at the
// repository root rather than copied into this package. A copy is a second
// source of truth, and a second source of truth drifts — which is the exact
// failure `QM-0140` exists to prevent. `fs.allow` lets the dev server read it;
// `vite build` inlines it through the normal JSON import.
const REPO_ROOT = resolve(import.meta.dirname, '..', '..', '..')

export default defineConfig({
  server: {
    fs: {
      allow: [resolve(import.meta.dirname), resolve(REPO_ROOT, 'schemas')],
    },
  },
})
