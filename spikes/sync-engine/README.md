# Spike Phase 0 — Moteur de synchronisation (enveloppes → SQLite)

**Question à trancher :** peut-on synchroniser INBOX de façon incrémentale vers
SQLite, avec une liste utilisable **instantanément et hors-ligne**, dans les
budgets du plan (liste < 1 s, `docs/PLAN.md` §1) ?

**Statut : jetable.** Ce code valide des décisions ; il sera supprimé une fois
la Phase 0 conclue.

## Ce que le spike démontre

1. **Offline-first** : au lancement, la liste est servie depuis SQLite *avant*
   toute connexion réseau (chronométré — c'est la mesure clé).
2. **Synchro initiale** par lots de 1 000 (`FETCH` de séquences, enveloppes +
   INTERNALDATE uniquement — jamais les corps).
3. **Synchro incrémentale** : nouveaux messages via `UID FETCH last+1:*`,
   suppressions par diff avec `UID SEARCH ALL`, resynchronisation complète si
   `UIDVALIDITY` change.
4. **Décodage RFC 2047 via `mail-parser`** (enseignement du spike oauth-gmail) :
   les sujets s'affichent enfin lisiblement.
5. Le spike affiche les **capabilities** du serveur (CONDSTORE, QRESYNC…) pour
   éclairer le choix du mécanisme de détection de changements en production.

## Limites assumées (réponses attendues en Phase 1, pas ici)

- Une seule boîte (INBOX), pas de flags lu/non-lu — la réponse de production
  est CONDSTORE/QRESYNC selon les capabilities observées.
- Suppression par diff complet des UIDs : acceptable jusqu'à ~100k messages,
  à remplacer par QRESYNC si disponible.

## Prérequis et lancement

Le spike `oauth-gmail` doit avoir été lancé une fois (refresh token présent
dans le Credential Manager). Puis, dans le même terminal PowerShell :

```powershell
$env:GOOGLE_CLIENT_ID = "…"
$env:GOOGLE_CLIENT_SECRET = "…"
cargo run -p spike-sync-engine --release
```

`--release` compte : les mesures de performance n'ont pas de sens en debug.
Lancez-le **deux fois** : la première fait la synchro initiale, la seconde
montre la liste offline instantanée puis la synchro incrémentale.

La base est écrite dans `target/spike-sync.db` (surchargeable via `SPIKE_DB_PATH`).

## Verdict (validé le 2026-07-11, INBOX réelle de 496 messages, build --release)

**Question tranchée : oui**, largement dans les budgets.

| Mesure | Résultat | Budget du plan |
|---|---|---|
| Lecture offline (liste + comptage) | ~180 µs | < 1 s — tenu ×5000 |
| Synchro initiale (496 enveloppes) | 1,03 s (~480 env./s) | — |
| Synchro incrémentale (aucun changement) | 130 ms | — |
| Base SQLite | 100 Ko (~206 o/enveloppe) | — |

Extrapolations pour le gate Phase 1 (50 000 messages) :
- base ≈ 10 Mo — négligeable ;
- synchro initiale ≈ 1 min 45 — acceptable car progressive : les insertions
  par lots de 1 000 rendent la liste utilisable pendant la synchro ;
- lecture offline attendue sub-milliseconde grâce à l'index date (à re-mesurer
  au gate avec un vrai volume).

Enseignements :

1. **L'architecture « enveloppes d'abord + SQLite » tient la promesse produit.**
   La vitesse perçue vient du cache local ; le réseau devient un raffinement.
2. **Le protocole UID suffit au spike, pas à la production.** Le diff complet
   des UIDs pour détecter les suppressions croît linéairement. Capabilities
   observées chez Gmail (2026-07-12) : **CONDSTORE présent, QRESYNC absent**
   → décision Phase 1 : détection des changements via CONDSTORE
   (HIGHESTMODSEQ), diff UID complet en repli pour les serveurs sans
   extension, QRESYNC en optimisation opportuniste là où il existe.
3. **Inverser l'ordre de synchro initiale en Phase 1** : du plus récent au plus
   ancien, pour que les premiers lots soient ceux que l'utilisateur regarde.
