import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// base relative : le bundle est embarqué par Tauri (frontendDist), pas
// servi depuis une racine de domaine.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  build: { outDir: 'dist', emptyOutDir: true },
});
