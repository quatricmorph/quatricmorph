// `vitest/config` re-exports vite's own `defineConfig` and adds the `test` key.
// Importing it here rather than keeping a second vitest.config file is
// deliberate: the `three` -> `three/webgpu` alias below is load-bearing (see the
// note on it), and a separate test config would have to restate it. Two copies
// of that alias is exactly the kind of drift that makes the tests pass against a
// module graph the build never produces.
import { defineConfig } from 'vitest/config'
import { fileURLToPath } from 'node:url'

const entry = p => fileURLToPath(new URL(p, import.meta.url))

// The checkpoint data plane. It is mounted at /api rather than /gpt2 because
// /gpt2/ is now a *page*: the server matches its data prefix before falling
// through to static files, so sharing the prefix made the page unreachable
// (it answered {"error": "unknown route ''"}).
const GPT2_PROXY = {
  '/api': {
    // Override with MM_MODEL_SERVER when the python server is on another port
    // (two checkouts side by side, or a stale one holding :8000).
    target: process.env.MM_MODEL_SERVER || 'http://localhost:8000',
    changeOrigin: false,
  },
}

// Multi-page build. Every HTML file that Vite processes has to be named here --
// Rollup will not discover a page just because another page iframes it, and the
// three checkpoint pages do exactly that. They set the iframe's src from JS
// (`'../index.html?params=' + ...` in src/gpt2page.ts), which is even further
// outside anything the bundler could follow.
//
// The pages are top-level routes (/attngpt2/, /attnqkov/, /gpt2/) rather than
// living under /examples/: they are the product surface, not samples of it.
//
// The pages NOT listed are deliberate: ref.html and intro/index.html live in
// public/ instead. They embed zero-md's `<script type="text/markdown">` blocks,
// whose bodies are raw markdown, and Vite's HTML pipeline rewrites <script> tags
// it finds in an entry. Serving them from public/ copies them through byte for
// byte and keeps their URLs (/ref.html, /intro/) unchanged.
export default defineConfig({
  appType: 'mpa',

  resolve: {
    // The app needs WebGPURenderer, which only the three/webgpu build exports,
    // but the addons (OrbitControls, FontLoader) import bare 'three' inside
    // themselves. Without this the bundle carries two copies of the core, every
    // instanceof across the boundary fails, and the renderer draws nothing --
    // silently, with no error.
    //
    // The regex form matters: a plain string 'three' prefix-matches, so it
    // would rewrite 'three/addons/...' to 'three/webgpu/addons/...' and break
    // every addon import instead.
    alias: [{ find: /^three$/, replacement: 'three/webgpu' }],
  },

  build: {
    rollupOptions: {
      input: {
        main: entry('./index.html'),
        attngpt2: entry('./attngpt2/index.html'),
        attnqkov: entry('./attnqkov/index.html'),
        gpt2: entry('./gpt2/index.html'),
      },
    },
  },

  // tools/gpt2_server.py serves the checkpoint-derived matrices under /api/
  // and, when run standalone, mm's static files too. Under `vite dev` the
  // static half comes from Vite, so the data has to be proxied back onto this
  // origin -- the example pages load the matrices as same-origin CSV URLs and a
  // cross-origin fetch to :8000 would be blocked.
  //
  // `preview` needs the identical proxy: it serves dist/, which has no /api of
  // its own, so without this the built pages come up with "cannot reach the
  // model server" while the dev pages work. (Serving dist/ from the python
  // server instead -- `gpt2_server.py --root dist` -- needs no proxy at all,
  // because then one origin has both halves.)
  server: { proxy: GPT2_PROXY },
  preview: { proxy: GPT2_PROXY },

  // jsdom for everything: gpt2page.ts drives `document` directly, and the
  // modules that only do arithmetic do not care either way. Per-file
  // `@vitest-environment` docblocks would be one more thing to remember.
  test: {
    environment: 'jsdom',
    include: ['test/**/*.test.{js,ts}'],
    setupFiles: ['./test/setup.ts'],
  },
})
