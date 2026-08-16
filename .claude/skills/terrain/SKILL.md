---
name: terrain
description: Traiter un constat terrain du Chef Ingénieur — reproduire, remonter à la racine, corriger le jour même avec test, amendement du Système et gate complète. Voie rapide, mêmes gardes que /chantier.
---

# /terrain — le constat terrain se corrige le jour même

L'argument est le constat, tel que vu au terrain. C'est la boucle
genchi genbutsu de PASSATION §2.5 : les retours du terrain se corrigent
**le jour même** (modèle : le WAL, ADR 0011 ; les traits d'accent,
A37/A38).

## Déroulé

1. **Reproduire** — comprendre le mécanisme exact avant de toucher au
   code. Si la reproduction exige la machine du CE, demander la mesure
   ou la manipulation précise et attendre. Jamais de correction sur
   hypothèse.
2. **Remonter à la racine** — un symptôme corrigé en surface revient
   ailleurs (leçon 9ebd7b2 → 5698641 : le blur du raccourci ne couvrait
   que e/Suppr ; la racine était le focus laissé par le clic). Si la
   racine est profonde ou le périmètre s'élargit, **basculer en
   `/chantier`** — la voie rapide ne dispense pas de conception.
3. **TDD** : un test qui échoue sur le constat (e2e si c'est un parcours,
   Rust si c'est le cœur), puis la correction. Le test s'asserte sur le
   fait observé au terrain, pas sur l'implémentation.
4. **DC-D2** : si l'UI est touchée, le Système s'amende au même commit —
   phrase au journal (A-n), règle mise à jour là où elle vit.
5. **Gate complète** : `/gate`. Puis commit (`fix:`, sans accents, le
   mécanisme et le remède au corps du message), push, `gh run watch`
   jusqu'à CI verte.
6. **Clôture** : proposer au CE de re-jouer le geste au terrain pour
   confirmer — en fournissant **systématiquement les commandes
   PowerShell nécessaires à ce test terrain** (lancement de l'app,
   build, préparation des comptes, mesures), prêtes à copier, une par
   bloc. Mettre à jour la mémoire si le constat clôt ou rouvre un
   chantier.
