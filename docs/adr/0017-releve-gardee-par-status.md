# ADR 0017 — Relève gardée par STATUS (le cycle sobre)

**Date** : 2026-08-13 · **Statut** : accepté (GO du Chef Ingénieur sur
les mesures du 3ᵉ terrain, PLAN-SYNCHRO §3)

## Contexte

Mesure sur la boîte réelle du Chef Ingénieur (traces par phase,
2026-08-13) : le cycle de synchronisation **récurrent** coûtait
~38 minutes sur un compte Gmail à ~50 dossiers — INBOX 34 s,
inventaire 660 s, dossiers 1 540 s — parce que chaque dossier payait
`SELECT` + `UID SEARCH ALL` à chaque cycle, même immobile, et que
`SyncEngine::sync` rejouait un `LIST` complet par dossier. Le cycle ne
se reposait jamais ; la latence vécue était la durée du cycle. Bridage
Gmail probable en aggravation (mêmes commandes deux fois plus lentes
d'un cycle à l'autre) : le volume de commandes était lui-même le
problème.

## Décision

1. **Un relevé `STATUS (MESSAGES UIDNEXT UIDVALIDITY)` par dossier et
   par cycle** — celui que la garde d'espace (ADR 0010 §4) payait déjà,
   enrichi — sert AUSSI la décision de relève : `faut_relever`
   (décision pure, mail-core, tenue par table de tests) **saute** le
   dossier quand rien n'a bougé. INBOX est gardée comme les autres.
2. Le repère comparé est le **UIDNEXT vu au relevé précédant la
   dernière relève soldée** (`mailboxes.remote_uidnext`, NULL sur base
   héritée → premier cycle conservateur), pas `last_uid` : un serveur
   ne redescend jamais son UIDNEXT, `last_uid` retombe à la suppression
   du plus récent.
3. **Toute incertitude relève** : dossier jamais relevé, UIDNEXT ou
   UIDVALIDITY tus par le serveur, repère illisible, relevé refusé,
   **actions locales en attente de rejeu** (les sauter les
   abandonnerait).
4. `folders()` est **hoisté hors de `SyncEngine::sync`** : un LIST par
   compte et par cycle, à l'inventaire, avec la liste déjà en main.

## Conséquences

- Cycle au repos : ~51 STATUS + 1 LIST par compte, plus aucun SELECT —
  gate chiffrée : **< 60 s sur le compte Gmail du terrain** (contre
  38 min), lisible dans la trace (`n dossiers (k sautés)`).
- Les changements de **drapeaux seuls** ne réveillent pas un dossier —
  ils n'étaient déjà PAS resynchronisés (`changes_since` absent) : rien
  n'est perdu qui était servi. Le vrai reflet des drapeaux est le
  chantier CONDSTORE (E2b du PLAN-SYNCHRO), où `HIGHESTMODSEQ` rejoindra
  le relevé STATUS.
- Un dossier sauté ne rafraîchit pas `remote_total` : l'avancement
  intégral (ADR 0010 §5) ne bouge pas pour des boîtes immobiles — ce
  qui est exact.
