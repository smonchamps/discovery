// Tests du helper d'allocation de port CDP (PLAN-ISOLATION-E2E, E1).
// Hors de `tests/` à dessein : ce dossier appartient à Playwright
// (une seule fenêtre pilotée, workers: 1) — ces tests-ci sont du pur
// node:test, joués par `node --test` avant la suite.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { allouerPortCdp } from './port-cdp.mjs';

const ecouter = (port) =>
  new Promise((resolve, reject) => {
    const serveur = createServer();
    serveur.once('error', reject);
    serveur.listen(port, '127.0.0.1', () => resolve(serveur));
  });

const fermer = (serveur) => new Promise((resolve) => serveur.close(resolve));

test('le port rendu est libre : on peut y écouter aussitôt', async () => {
  const port = await allouerPortCdp();
  assert.ok(Number.isInteger(port) && port > 0 && port < 65536, `port invalide : ${port}`);
  const serveur = await ecouter(port);
  await fermer(serveur);
});

test('un port tenu par un autre processus n\'est jamais rendu', async () => {
  // On tient un port, puis on alloue plusieurs fois : l'OS ne doit
  // jamais rendre le port tenu — c'est toute la promesse face aux
  // suites e2e concurrentes.
  const tenu = await ecouter(0);
  const portTenu = tenu.address().port;
  try {
    for (let n = 0; n < 20; n++) {
      assert.notEqual(await allouerPortCdp(), portTenu);
    }
  } finally {
    await fermer(tenu);
  }
});
