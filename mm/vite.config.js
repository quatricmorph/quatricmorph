import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

const entry = p => fileURLToPath(new URL(p, import.meta.url))

// Multi-page build. Every HTML file that Vite processes has to be named here --
// Rollup will not discover a page just because another page iframes it, and the
// three example pages under examples/ do exactly that (`src="../../index.html"`).
//
// The pages NOT listed are deliberate: ref.html and intro/index.html live in
// public/ instead. They embed zero-md's `<script type="text/markdown">` blocks,
// whose bodies are raw markdown, and Vite's HTML pipeline rewrites <script> tags
// it finds in an entry. Serving them from public/ copies them through byte for
// byte and keeps their URLs (/ref.html, /intro/) unchanged.
export default defineConfig({
  appType: 'mpa',

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

  server: {
    // tools/gpt2_server.py serves the checkpoint-derived matrices under /gpt2/
    // and, when run standalone, mm's static files too. Under `vite dev` the
    // static half comes from Vite, so the data has to be proxied back onto this
    // origin -- examples/gpt2 loads the matrices as same-origin CSV URLs and a
    // cross-origin fetch to :8000 would be blocked.
    proxy: {
      '/gpt2': {
        target: 'http://localhost:8000',
        changeOrigin: false,
      },
    },
  },
})
