import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri uses Chromium on Windows/Linux, WebKit on macOS
// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    hmr: undefined,
    watch: {
      // Don't watch the Rust source — Tauri rebuilds separately
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: 'chrome105',
    minify: 'oxc',
    sourcemap: false,
  },
})
