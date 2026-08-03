import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

const __dirname = dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  // three@0.185+ exports three/addons/* natively — do not alias to examples/jsm
  // (a custom alias can break dep prebundling and leak bad module URLs)
  optimizeDeps: {
    include: [
      'three',
      'three/addons/controls/OrbitControls.js',
      'three/addons/loaders/FontLoader.js',
      'lil-gui',
    ],
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        ref: resolve(__dirname, 'ref.html'),
        intro: resolve(__dirname, 'intro/index.html'),
        attngpt2: resolve(__dirname, 'examples/attngpt2/index.html'),
        attnqkov: resolve(__dirname, 'examples/attnqkov/index.html'),
      },
    },
  },
})
