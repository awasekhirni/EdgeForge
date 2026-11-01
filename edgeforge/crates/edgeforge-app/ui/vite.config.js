import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Tauri expects a fixed port; if that's not available, it will fall back
// to the next available one.
export default defineConfig({
  plugins: [react()],
  // Vite options tailored for Tauri development; see https://tauri.app/v1/guides/getting-started/setup-vite
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: false,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Produce a single-page app in `dist/` for Tauri to bundle.
    target: 'es2021',
    minify: 'esbuild',
    sourcemap: false,
  },
})
