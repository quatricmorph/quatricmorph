import { defineConfig } from 'vite'
import { resolve } from 'node:path'

// A vanilla Three.js app: no React, no framework plugin. The previous config
// pulled in @vitejs/plugin-react for a starter `App.tsx` that has since been
// removed — it rendered a counter button, not the workspace.
export default defineConfig({
  server: { port: 3000 },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        ref: resolve(__dirname, 'ref.html'),
      },
    },
  },
})
