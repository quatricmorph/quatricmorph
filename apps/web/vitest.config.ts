import { defineConfig } from 'vitest/config'

// One runner for every web app. Each app keeps its own tests beside its source;
// this config makes `npm test` at apps/web cover all of them, which is what CI
// and STATUS.md report against.
export default defineConfig({
  test: {
    environment: 'node',
    include: [
      'quatricmorph-workspace/src/**/*.test.ts',
      'quatricmorph-workspace/src/**/__tests__/**/*.test.ts',
      'model-viewer/src/**/__tests__/**/*.test.ts',
      'query-interface/src/**/__tests__/**/*.test.ts',
    ],
  },
})
