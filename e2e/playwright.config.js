import { defineConfig } from '@playwright/test';

// Une seule fenêtre applicative pilotée : exécution strictement séquentielle.
export default defineConfig({
  testDir: './tests',
  workers: 1,
  // Ce budget couvre aussi le `beforeAll` (Playwright y applique le timeout
  // des tests) : il doit laisser passer le démarrage à froid de WebView2 en
  // CI, jusqu'à 90 s d'attente du CDP. Les tests eux-mêmes durent des ms.
  timeout: 180_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
});
