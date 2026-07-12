# Spike Phase 0 — Pont web

**Question à trancher :** un navigateur ne peut pas parler IMAP (pas de socket
TCP brut). Le plan (§3) répond par « un seul cerveau » : le même moteur Rust
sert le desktop en local et le web via HTTP. Que coûte réellement ce pont ?

**Statut : jetable.** Ce code valide des décisions ; il sera supprimé une fois
la Phase 0 conclue.

## Ce que le spike démontre

1. Le moteur validé par le spike `sync-engine` (SQLite + synchro IMAP
   incrémentale), exposé derrière un serveur HTTP local :
   - `GET /` — une page web minimale (la « liste de messages » du futur client web) ;
   - `GET /api/messages` — les 50 dernières enveloppes en JSON, chronométrées ;
   - `POST /api/sync` — synchro incrémentale à la demande, avec reconnexion
     IMAP silencieuse si la session a expiré.
2. Les latences mesurées des deux côtés (serveur et navigateur) — à comparer
   aux ~180 µs de l'accès en processus mesuré par le spike sync-engine :
   la différence EST le coût du pont.
3. La règle de sécurité côté page : les données du mail entrent dans le DOM
   par `textContent`, jamais par `innerHTML`.

## Lancer

```powershell
# Mode hors-ligne (5 messages de démonstration, aucun credential)
cargo run -p spike-web-bridge -- --offline

# Mode réel (mêmes prérequis que les autres spikes)
$env:GOOGLE_CLIENT_ID = "…"
$env:GOOGLE_CLIENT_SECRET = "…"
cargo run -p spike-web-bridge --release
```

Puis ouvrez <http://127.0.0.1:8990>, notez la ligne de statut (latences) et
testez le bouton « Synchroniser ». `Ctrl+C` pour arrêter.

## Ce que le spike NE couvre PAS (chantiers de la Phase 4, notés au plan §7)

- **Authentification et multi-tenant** : ici, le serveur est mono-utilisateur
  sur 127.0.0.1. En production, c'est un service hébergé qui détient les
  tokens OAuth des utilisateurs — sessions, TLS, chiffrement au repos et
  pentest sont le gate de la Phase 4.
- **Asynchronisme** : `tiny_http` séquentiel suffit à un utilisateur ;
  la production passera sur axum/tokio.
- **Push temps réel** : le rafraîchissement est manuel ici ; la production
  utilisera SSE/WebSocket + IMAP IDLE côté serveur.

## Verdict (validé le 2026-07-12 — mode --offline puis compte réel, 501 messages)

**Question tranchée : oui**, le pont web est viable et son coût est mesuré.

| Mesure | Démo (5 msgs) | Réel (501 msgs) |
|---|---|---|
| `/api/messages` côté serveur (SQLite + JSON) | ~0,35 ms | **0,36 ms** |
| `/api/messages` côté navigateur | ~2,6 ms (établi) | 19 ms (1er chargement) |

Le temps serveur est **insensible au volume** (5 → 501 messages : identique),
conformément à ce que le spike sync-engine avait mesuré sur SQLite.

**Le coût du pont sur localhost est de l'ordre de 2-3 ms** — contre ~180 µs
pour l'accès en processus du desktop (spike sync-engine). Imperceptible en
local ; en production s'y ajoutera l'aller-retour réseau réel (20-80 ms
typiques), ce qui confirme deux choix du plan : le desktop reste en accès
direct (jamais via HTTP), et le client web (Phase 4) devra soigner
l'optimisme UI et le push serveur.

L'architecture « un seul cerveau » est validée : les modules `db`/`sync`/
`gmail` sont ceux du spike sync-engine, inchangés — seule la couche de
service diffère (IPC desktop vs HTTP web).

_Mode réel à confirmer : synchro au démarrage + bouton « Synchroniser »
(synchro incrémentale ~130 ms attendue + reconnexion IMAP silencieuse)._
