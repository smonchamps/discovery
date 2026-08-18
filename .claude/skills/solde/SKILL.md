---
name: solde
description: Clôturer un chantier Wind — vérifier terrain validé et CI verte, marquer le PLAN soldé, amender ETAT, consigner la dette, mettre à jour la mémoire persistante.
---

# /solde — la clôture standardisée d'un chantier

L'argument est le plan à solder (`PLAN-XXX`). Un chantier ne se solde
que sur des faits ; vérifier chaque condition avant d'écrire quoi que
ce soit.

## Conditions (toutes, sinon on dit ce qui manque et on s'arrête)

1. **Terrain validé** par le CE, explicitement, sur ses vrais comptes —
   un incrément non validé au terrain n'est pas livré (§2.5).
2. **CI verte** sur le dernier commit poussé (`gh run list`).
3. **Arbre propre** : rien du chantier n'attend un commit.

## Écritures

1. **`docs/PLAN-XXX.md`** : en-tête « **CHANTIER SOLDÉ le AAAA-MM-JJ —
   terrain complet** », avec les commits, la date du GO CE, les
   retouches terrain éventuelles et leur A-n (modèle : PLAN-WADA).
2. **`docs/ETAT.md`** : l'état reflète le chantier livré ;
   budgets re-mesurés si touchés ; pièges appris ajoutés là où ils
   vivent.
3. **Dette** : ce qui est reporté part dans `docs/DETTE.md`, nommé,
   avec la raison du report (§2.6 — un report s'écrit).
4. **ADR** si une décision structurante n'en a pas encore.
5. **Mémoire persistante** : le fichier du plan passe à « soldé », date
   absolue, faits saillants ; l'index `MEMORY.md` suit.
6. Si ces écritures font un commit : `docs:`, sans accents, puis push
   et CI verte.

## Fin de phase

Si le chantier clôt une **phase** du PLAN produit : proposer la revue
de clôture `docs/PHASEn.md` (livré contre le plan, budgets re-mesurés,
enseignements, reports assumés, GO/NO-GO) — c'est une décision CE, ne
pas l'écrire sans son GO.
