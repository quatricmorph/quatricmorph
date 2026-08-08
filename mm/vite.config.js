import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

const entry = p => fileURLToPath(new URL(p, import.meta.url))

const GPT2_PROXY = {
  '/gpt2': {
    target: 'http://localhost:8000',
    changeOrigin: false,
  },
}

// Multi-page build. Every HTML file that Vite processes has to be named here --
// Rollup will not discover a page just because another page iframes it, and the
// three example pages under examples/ do exactly that. They now set the iframe's
// src from JS (`'../../index.html?params=' + ...` in src/gpt2page.js), which is
// even further outside anything the bundler could follow.
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
        attngpt2: entry('./examples/attngpt2/index.html'),
        attnqkov: entry('./examples/attnqkov/index.html'),
        gpt2: entry('./examples/gpt2/index.html'),
      },
    },
  },

  // tools/gpt2_server.py serves the checkpoint-derived matrices under /gpt2/
  // and, when run standalone, mm's static files too. Under `vite dev` the
  // static half comes from Vite, so the data has to be proxied back onto this
  // origin -- the example pages load the matrices as same-origin CSV URLs and a
  // cross-origin fetch to :8000 would be blocked.
  //
  // `preview` needs the identical proxy: it serves dist/, which has no /gpt2 of
  // its own, so without this the built pages come up with "cannot reach the
  // model server" while the dev pages work. (Serving dist/ from the python
  // server instead -- `gpt2_server.py --root dist` -- needs no proxy at all,
  // because then one origin has both halves.)
  server: { proxy: GPT2_PROXY },
  preview: { proxy: GPT2_PROXY },
})
