import { defineConfig } from '@playwright/test';

// Une seule fenêtre applicative pilotée : exécution strictement séquentielle.
export default defineConfig({
  testDir: './tests',
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
});
