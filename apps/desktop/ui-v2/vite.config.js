import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// base relative : le bundle est embarqué par Tauri (frontendDist), pas
// servi depuis une racine de domaine.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  // `esnext` : le seul navigateur est le WebView2 embarqué (Chromium
  // courant) — l'await de module de main.js (restauration de la langue
  // avant montage) passe sans transformation.
  build: { outDir: 'dist', emptyOutDir: true, target: 'esnext' },
});
