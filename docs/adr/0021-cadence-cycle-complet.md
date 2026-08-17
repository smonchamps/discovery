# ADR 0021 — Cadence du cycle complet (S-D4 tranché)

**Date** : 2026-08-17 · **Statut** : accepté (GO du Chef Ingénieur sur la
mesure terrain du 2026-08-17, PLAN-RETOURS-2 §1)

## Contexte

Retour terrain du Chef Ingénieur : la synchronisation Gmail est « trop
longue ». Mesure par phase sur la boîte réelle (compte Gmail, ~52
dossiers, trace `run_sync`, débogage) :

```
INBOX 3,4s · inventaire 16,4s · 52 dossiers (46 sautés) 31,2s · fils 7,8s · brouillons 8,9s
```

Re-mesuré **en release** (le débogage gonfle les phases CPU ; l'app
release est un sous-système *windows* sans console — trace redirigée
`2> fichier`) :

```
INBOX 5,0s · inventaire 12,6s · 52 dossiers (30 sautés) 109,8s · fils 0,0s · brouillons 7,6s   ≈ 135 s
```

Lecture : la relève gardée (ADR 0017) **fonctionne** — la plupart des
dossiers sont sautés, on ne parcourt pas chaque dossier. Mais le coût est
**~5 s par dossier CHANGÉ** (réseau, bridage Gmail probable — identique
debug/release : 6 dossiers = 31 s, 22 dossiers = 110 s), plus le STATUS
des 52 dossiers à l'inventaire (Gmail n'annonce pas LIST-STATUS → ~52
STATUS séquentiels). Sur Gmail, beaucoup de vues bougent souvent (« Tous
les messages », Important, catégories, libellés) : un cycle complet
**oscille de ~8 s à ~135 s** selon le nombre de dossiers changés. À 135 s
toutes les 5 min, l'app synchronisait **~45 % du temps** — c'est la
cadence, pas le parcours, qui fait le « trop long ».

Or **le veilleur IDLE (ADR 0018) est actif en production** : il tient
INBOX en **temps réel** (`EXISTS` → passe légère ciblée). Le cycle
complet de 5 min datait d'AVANT IDLE, quand il était le seul chemin
d'arrivée du courrier. Depuis, il ne sert plus qu'à ce qu'IDLE ne couvre
PAS — les autres dossiers, les brouillons, le différentiel des
suppressions, les drapeaux. L'ADR 0018 §7 avait laissé la question
ouverte : « la cadence du cycle complet sera re-discutée une fois IDLE
actif (S-D4) ».

## Décision

1. **Le cycle complet passe de 5 min à 30 min.** INBOX ne dépend pas de
   lui pour sa fraîcheur — IDLE la pousse en temps réel.
2. **Une passe légère (STATUS INBOX seul, quelques secondes) tourne
   toutes les 5 min en FILET.** Un veilleur IDLE peut tomber sans s'être
   encore reconnecté — la lecture d'une socket morte « pend » en silence
   (ADR 0018 §Contexte). Le filet garantit qu'INBOX reste fraîche à
   5 min près même dans ce cas. Elle se sabre d'elle-même pendant un
   cycle complet (`enSynchro`) : jamais deux relèves du même INBOX.
3. **« Tous les messages » (All Mail) reste synchronisé.** L'exclure
   aurait allégé chaque cycle mais **cassé la vue Archives** et fait
   disparaître de Wind tout mail archivé depuis un autre appareil (All
   Mail est le seul dépôt d'un message archivé). L'**ADR 0010 (« tout est
   synchronisé ») est préservé**. La cadence, à elle seule, divise la
   charge soutenue par 6 sans aucune perte.

## Conséquences

- **Charge de synchro soutenue ÷6** sur un compte à beaucoup de dossiers.
  Le « trop long » ressenti tombe : le balayage cher se fait 6× moins
  souvent, INBOX reste temps réel.
- **Les changements des AUTRES dossiers faits ailleurs** (réorganisation
  d'un libellé, archivage, brouillon écrit sur un autre appareil) peuvent
  attendre **jusqu'à 30 min** au lieu de 5. Assumé : ce n'est pas le
  courrier entrant (couvert par IDLE + le filet 5 min).
- **Budget à re-mesurer au terrain, en release** (le débogage gonfle les
  phases CPU) — le geste manuel et le réveil de veille forcent toujours
  une relève immédiate.

## Écartée — l'exclusion des vues virtuelles Gmail

Sortir Important / Suivis (Starred) du balayage était sûr (aucun mail
unique, non montrés dans la nav de Wind) mais, **une fois la cadence à
30 min**, le gain devient marginal (ces vues ne sont balayées que 6× moins
souvent de toute façon), pour un coût de code réel : le type `Folder` du
cœur ne porte pas la notion de « vue Gmail », il faudrait l'étendre,
propager la détection des drapeaux `\Important`/`\Flagged` dans
l'adaptateur IMAP, et toucher la logique de portée voisine de l'ADR 0010.
Reporté (§2.6) — à rouvrir si une mesure terrain montre que la cadence
seule ne suffit pas.
