# Revue de clôture — Phase 1 « je lis mes mails » (2026-07-12)

Le squelette marchant du plan ([PLAN.md](PLAN.md) §4) est livré, validé sur
compte réel et mesuré contre ses budgets. Discovery est une application :
fenêtre, compte Gmail, synchronisation, liste, lecture sûre, offline-first.

## 1. Livré, contre le plan

| Exigence du plan | État | Preuve |
|---|---|---|
| Un compte Gmail via OAuth PKCE | ✅ | `mail-auth`, coffre OS, reconnexion silencieuse, scopes vérifiés |
| Synchro enveloppes → SQLite, TDD serveur simulé | ✅ | `mail-core` (moteur + `FakeServer`), plus récent d'abord, CONDSTORE prêt |
| Shell Tauri : liste + lecture | ✅ | Liste virtualisée, volet de lecture 3 couches (ammonia + pixel neutre + sandbox/CSP) |
| CI complète dès le jour 1 | ✅ | fmt, clippy `-D warnings`, tests, cargo audit — verte sur `main` |
| **Gate : budgets tenus sur 50 000 messages** | ✅ | RAM résidente **84,5 Mo** (delta volume : 0), démarrage **348 ms**, défilement fluide validé à la main, base 5,4 Mo |

Au-delà du plan, exigé par l'usage réel : images embarquées (`cid:`)
inlinées en `data:` URIs, bouton « Afficher les images » par message
(CSP qui suit le choix), 65 tests.

## 2. Enseignements consignés

1. **Un `srcdoc` hérite de la CSP de son hôte** (prouvé par mesure,
   `naturalWidth` 0→1) : le shell doit ouvrir `img-src`/`style-src`, le
   document du message reste la couche restrictive. Documenté dans
   `mail_render::email_document`.
2. **La RAM qui fait foi est le working set privé**, pas les octets
   committés (Chromium : 250-375 Mo de commit pour 85 Mo résidents).
   Méthodologie corrigée dans l'ADR 0002.
3. La virtualisation isole totalement la mémoire du volume : ~40 nœuds
   DOM, que la boîte fasse 500 ou 50 000 messages.

## 3. Reporté, volontairement

- **Fidélité CSS** (blocs `<style>` via lightningcss) — chantier connu
  depuis Phase 0, verdict 0/20/0 inchangé : lisible partout, dégradé.
- **CONDSTORE réel** dans l'adaptateur (le moteur est prêt, le repli
  différentiel fonctionne) ; **IDLE/push** et synchro périodique.
- **Raccourcis clavier** — au périmètre de la Phase 2 (plan §4).
- **Dossier CASA Google** — action côté produit-owner, toujours sur le
  chemin critique du lancement public (limite actuelle : mode Testing).

## 4. Décision

**GO Phase 2 — « je travaille dans mes mails »** (PLAN.md §4) : actions
lu/non-lu, archiver, supprimer, déplacer — optimistes en UI, portées par
une **file d'actions offline rejouable** journalisée dans SQLite et
rejouée en tête de synchronisation. Composer/répondre/envoyer suivront.
**Gate de sortie : zéro perte d'action prouvée par tests de coupure**
(réseau, crash) **et parcours E2E verts.**
