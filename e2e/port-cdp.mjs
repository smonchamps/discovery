// Allocation d'un port CDP libre (PLAN-ISOLATION-E2E).
//
// Le port 9222 codé en dur était le SEUL état partagé entre deux suites
// e2e jouées en même temps depuis deux worktrees (constat 2026-08-15 :
// applications mortes au démarrage, échecs croisés — `connectOverCDP`
// reconnaît « sa » fenêtre au seul critère `tauri.localhost`, vrai pour
// n'importe quelle fenêtre Wind). Remède à la racine : plus de port
// partagé du tout — l'OS choisit un port libre à chaque lancement.
//
// Fenêtre TOCTOU assumée : entre la fermeture de la sonde et le bind de
// WebView2, un tiers peut prendre le port. L'échec est alors bruyant
// (CDP injoignable, journal recraché) et la relance choisit un autre
// port — c'est un flake théorique, pas un état stable.
import { createServer } from 'node:net';

export function allouerPortCdp() {
  return new Promise((resolve, reject) => {
    const sonde = createServer();
    sonde.once('error', reject);
    sonde.listen(0, '127.0.0.1', () => {
      const { port } = sonde.address();
      sonde.close((erreur) => (erreur ? reject(erreur) : resolve(port)));
    });
  });
}
