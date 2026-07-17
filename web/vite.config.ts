import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte({ preprocess: vitePreprocess() })],
  server: {
    host: '127.0.0.1',
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:43473',
        // Forward WebSocket upgrades so dev against a live agent gets push
        // updates on /api/ws instead of degrading to 5s fallback polling.
        ws: true
      }
    }
  }
});
