import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// base relative : le bundle est servi depuis un sous-dossier
// (/option2-svelte/dist/) par le harnais de mesure.
export default defineConfig({
  base: './',
  plugins: [svelte()],
});
