# Revue de clôture — la refonte UI v2

Exigée par la gate de bascule (PLAN-UI-V2 §P5). Close le 2026-08-15
avec l'exécution de B2 : **v2 est la seule interface, v1 n'existe
plus** que dans l'historique git.

## Livré vs plan

| Phase | Plan | Livré | Écart |
|---|---|---|---|
| P1 | gate perf sur base réelle | ✓ + deux dettes cœur préexistantes trouvées et corrigées (pagination OFFSET 252→14,6 ms ; orphelins 428→3,3 ms à l'ouverture) | néant — gains au-delà du plan |
| P2 | écran 02 (nav, liste, volet) | ✓ pixel-exact, actions réelles, aperçus matérialisés | aperçu/puces corrigés au terrain le jour même |
| P3 | écran 03 (conversation) | ✓ | hauteur d'iframe bornée (sandbox opaque) ; « À » approximé — dits à la livraison |
| P4 | composition, réglages, onboarding | ✓ | « De » = adresse seule ; toast « Message envoyé. » = remise ; sélecteur De ajouté sur verdict (A10) |
| P5 | dus de bascule, décisions | ✓ les cinq dus + D1/D3 câblées, D2 coupée signée | D5 coupée PUIS rouverte au terrain bêta (bouton câblé par PLAN-SYNCHRO E3) — l'hypothèse « la synchro auto suffit » était fausse |
| R1 | v2 autonome | ✓ témoin de bout en bout (envoi parti seul, copie tirée) | — |
| R2 | parcours v1 portés | ✓ 9 portés, graines exactes ; régression #4 re-couverte | 3 abandons motivés en tête de spec |
| B1 | bascule réversible | ✓ 2026-08-12 | — |
| B2 | retrait | ✓ 2026-08-15 | fenêtre close à J-3 sur décision CE (motif : interférences v1 dans les tests ; v2 stable — 0.1.3→0.1.4 auto-update, zéro défaut critique acté) |

## Budgets — mesurés, pas déclarés

Sur la base réelle (2 942 conversations, 256k messages), cycle de
synchro actif, 2 passes à chaud (gate R1) : démarrage 578–649 ms ✓ ·
première page 94–99 ms ✓ · page p95 9,2–9,7 ms ✓ · thème p95 0,4 ms ✓ ·
RAM 184–187 Mo ✓ · ouverture p50 ~14 ms ✓, p95 52–55 ms — dépassement
de 2–5 ms porté par les corps > 1 Mo, acté en dette **D-1**.

## Écarts assumés — tous au journal du Système

A6 (le prototype devient cible normative) jusqu'à **A13** (Réglages en
deux volets) : barres de défilement (A7), accessibilité — jetons
corrigés, clavier partout, gate de contraste (A8), objet 16 px (A9),
sélecteur De (A10), section Comptes (A11), flèche de Transférer en
miroir (A12). Depuis, PLAN-DC (A18) a fait du Système le SEUL document
normatif — le prototype qui a guidé cette refonte est retiré, sa
mission accomplie.

## Enseignements

1. **Le terrain corrige le jour même, et il a toujours raison** — y
   compris contre le prototype (A9, A10, A12) et contre nos décisions
   (D5 rouverte) ; un « coupé » signé n'est pas un « coupé » définitif.
2. **Les bancs mentent si on ne les blinde pas** : dist embarqué à la
   compilation de main.rs, zombies qui verrouillent l'exe, cache HTTP
   WebView2 — trois pièges MESURÉS, centralisés dans `rebuild-v2.mjs`.
3. **`| tail` avale les échecs** (deux fois) : le code de sortie se
   vérifie nu, jamais derrière un filtre.
4. **La v1 était la colonne vertébrale d'exécution**, pas un écran — le
   relevé de R1 (cinq flux qu'elle seule déclenchait) a évité de livrer
   un client qui ne relève plus le courrier.
5. **Une passe d'audit transverse rapporte** : l'accessibilité a trouvé
   17 paires de contraste sous seuil dans les jetons mêmes du prototype.

## Reports — tenus au registre et à l'Annexe A

Multi-fenêtre, composition riche, pièces jointes à l'envoi, Cc/Cci :
reportés, affordances inertes comme au prototype (Annexe A signée).
Dette ouverte : D-1 (gros corps), D-2 (LRU), D-4 (piège de focus).
Thèmes « Le vent »/« Tournesol » (moitié restante de D6) : après
bascule — c'est maintenant ; à instruire si réclamés.

## GO / NO-GO

**GO — signé par le Chef Ingénieur le 2026-08-15** (« Go B2
maintenant »), fenêtre d'observation close à J-3 sous son arbitrage,
motif consigné au plan de retrait. La refonte est terminée : PLAN-UI-V2
et PLAN-RETRAIT-V1 sont soldés.
