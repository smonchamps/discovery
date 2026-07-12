# Revue de conception — clôture de la Phase 0 (2026-07-12)

Conformément au plan ([PLAN.md](PLAN.md) §2), la Phase 0 se clôt par cette
revue : chaque problème dur a été résolu par un spike mesuré sur le compte
Gmail réel, les décisions issues de l'exploration sont gelées, et les
hypothèses restantes sont nommées avec leur échéance.

## 1. Les quatre problèmes durs : verdicts

| Spike | Question | Verdict | Mesure clé |
|---|---|---|---|
| oauth-gmail *(retiré → [mail-auth](../crates/mail-auth/src/lib.rs))* | Authentifier sans jamais voir un mot de passe ? | ✅ Oui | PKCE loopback + Credential Manager, reconnexion silencieuse |
| sync-engine *(retiré → [mail-core](../crates/mail-core/src/sync.rs))* | Synchro incrémentale avec liste offline instantanée ? | ✅ Oui | Lecture offline ~180 µs (budget < 1 s tenu ×5000) |
| [html-render](../spikes/html-render/README.md) | Afficher l'HTML réel sûrement sans le casser ? | ✅ Sécurité acquise | 3 couches, 10 tests ; fidélité 0/20/0 → chantier CSS compris |
| [web-bridge](../spikes/web-bridge/README.md) | Que coûte le pont web (« un seul cerveau ») ? | ✅ Viable | 0,36 ms serveur, insensible au volume (5 → 501 msgs) |

## 2. Décisions gelées (issues de la grille set-based, PLAN.md §2.4)

1. **Stockage local : SQLite** (FTS5 en Phase 3) — la lecture est
   sub-milliseconde et insensible au volume testé.
2. **Détection des changements IMAP : CONDSTORE** (Gmail l'expose, pas
   QRESYNC), diff UID complet en repli, QRESYNC en optimisation opportuniste.
3. **Parsing MIME : `mail-parser` (Stalwart)** — RFC 2047, corps, conversion
   texte→HTML : rien à réécrire.
4. **Auth Gmail : OAuth2 PKCE loopback**, tokens dans le coffre de l'OS,
   vérification systématique des **scopes accordés** (consentement granulaire).
5. **Architecture « un seul cerveau »** : le même moteur sert le desktop en
   processus (~180 µs) et le web via un `sync-server` hébergé (Phase 4) ;
   le desktop ne passe **jamais** par HTTP.
6. **Sécurité du rendu** : défense en profondeur non négociable —
   assainissement + blocage des images distantes + sandbox/CSP ; données de
   mail dans le DOM via `textContent` uniquement.

## 3. Hypothèses restantes (nommées, avec échéance)

- **Shell desktop Tauri 2** : hypothèse de travail non spikée — à valider au
  tout premier gate de la Phase 1 (démarrage < 1 s, RAM < 200 Mo).
- **Fidélité CSS** : parser et réinjecter les blocs `<style>` scopés
  (`lightningcss`) — chantier Phase 1, approche Gmail.
- **Microsoft 365** : IMAP+OAuth vs Graph API — à trancher en Phase 3.
- **Ordre de synchro initiale** : du plus récent au plus ancien (enseignement
  sync-engine) — à implémenter en Phase 1.

## 4. Risques ouverts (PLAN.md §7)

- **Audit CASA Google** (scopes restreints) : toujours sur le chemin critique
  du lancement public — dossier à ouvrir dès que possible.
- Mode Testing : refresh tokens 7 jours, 100 testeurs max — contrainte de
  développement acceptée.

## 5. Sort des spikes

Amendement au plan initial (« suppression à la clôture ») : chaque spike est
supprimé **lorsque son équivalent de production existe** dans `mail-core`,
pas avant — leur code sert de référence d'implémentation à la Phase 1.
Les verdicts, eux, vivent dans les README et dans ce document.

Retirés le 2026-07-12, squelette marchant validé sur compte réel :
`oauth-gmail` (→ `crates/mail-auth`) et `sync-engine` (→ `crates/mail-core` +
`crates/mail-imap`). Leurs README restent lisibles dans l'historique git
(commits `6e23aaa` et `85bfa2e`). Restent en place : `html-render` (jusqu'à
l'écran de lecture) et `web-bridge` (jusqu'au sync-server de Phase 4).

## 6. Décision

**GO Phase 1 — squelette marchant** (PLAN.md §4) : un compte Gmail, synchro
des enveloppes vers SQLite dans `mail-core` (avec tests, contre un serveur
IMAP simulé), shell Tauri 2, liste virtualisée et lecture d'un message.
CI complète dès le premier jour ; gate de sortie : budgets de performance
tenus sur 50 000 messages réels.
