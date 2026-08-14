# Plan — DC : le Système, source unique et exhaustive

Commande du Chief Designer (2026-08-14) : clarifier, simplifier et
fiabiliser la documentation de design autour de
[`docs/design/systeme.dc.html`](design/systeme.dc.html). Le Système
redevient **l'unique source normative**, exhaustive de ce qui est
réellement livré — et un mécanisme l'empêche de re-dériver. La refonte
UI v2 est derrière nous : le prototype a rempli son office, la parité
est livrée ; la période où chaque chantier normait par « maquette de
plan + décisions + journal » s'achève en reversant tout au Système.

## 1. L'existant, et pourquoi il ne suffit pas

- **Quatre familles de fichiers portent du design**, à statuts
  différents : le Système (`systeme.dc.html`, 117 Ko — jetons, thèmes,
  typo, kit, six écrans, journal A1–A16) ; le prototype
  (`ui_prototype.html`, **7,3 Mo**, cible normative depuis A6, jamais
  retouché depuis son import du 2026-08-11, commit 9975a12) ; trois
  maquettes de plan (`maquette-synchro`, `maquette-brouillons`,
  `maquette-pieces-jointes`) validées puis restées vivantes ;
  l'inventaire d'icônes ([`assets/icones/`](../assets/icones/README.md),
  le contrat des glyphes).
- **Le journal a décroché.** Dernier commit du Système : e6ccd7d
  (2026-08-13). **A17 est cité au message du commit 27ed056 mais
  n'a jamais été inscrit au fichier** — les trois commits pièces
  jointes, le P0-bis réseau et une partie des brouillons n'ont pas
  touché le Système. « Chaque écart s'inscrit au journal » a cessé de
  fonctionner le 13 août au soir.
- **Les valeurs ont dérivé.** Le `--alert` du thème nuit vaut
  `#ea9a90` dans `systeme.css` (remède A8, commit a378dd6) — le doc ne
  porte pas cette valeur. La section « Kit unifié » affirme un
  inventaire « arrêté à 36 glyphes » ; le contrat réel en compte 39.
  Trois compteurs de glyphes coexistent (section du doc, journal,
  README) et un seul est juste.
- **Le doc dessine des thèmes qui n'existent pas.** La section
  « Thèmes de couleur » en montre **neuf** ; `systeme.css` en
  implémente **sept** (la nature, l'air, le feu, l'eau, les astres, la
  terre, la nuit). « Le vent » et « Tournesol » n'ont jamais été
  livrés — la gate `contraste.mjs` ne les a jamais mesurés.
- **Des surfaces entières manquent.** Écrans dessinés : Onboarding,
  Réception, Conversation, Composition, Avis et progression, Ligne de
  message. Existent à l'écran mais pas comme sections : Réglages deux
  volets (A13 en prose seule), guichet de compte partagé et section
  Comptes (A11), modale de migration (ADR 0012), tout le chantier
  brouillons (dossier, mention en Réception, bloc pointillé), tout le
  chantier pièces jointes (puces à trois états, plafond, transfert),
  la barre d'état synchro et l'état Hors ligne (A16, P0-bis). Les
  écrans dessinés sont périmés : la Composition montre quatre boutons
  (A14 en commande cinq), sans sélecteur « De » (A10), sans puces
  réelles ; la Réception n'a ni mention brouillon ni barre horodatée.
- **Le sommet de la hiérarchie est fictif.** Le prototype, officiellement
  la norme suprême (A6), n'a jamais bougé pendant qu'une dizaine de
  divergences assumées s'accumulaient. Et le doc se contredit : son
  en-tête dit « ce document est la source à transmettre à
  l'implémentation », son journal dit « le prototype l'emporte ».
- **Menus fils qui pendent** : le doc référence un `./support.js` qui
  n'existe pas dans `docs/design/` (bénin — tout le style est inline) ;
  chaque maquette recopie les jetons en dur (« valeurs exactes de
  systeme.css »), troisième copie des couleurs après le doc et le CSS.

## 2. Les décisions (DC-D1 à DC-D6)

- **DC-D1 — clôture d'A6 : le Système seul normatif (A18).** Deux
  normes qui se contredisent en silence sont un défaut — le principe
  acté à A6 joue maintenant dans l'autre sens. `ui_prototype.html` est
  **supprimé** du dépôt (7,3 Mo de player bundlé, non maintenable par
  nature ; l'histoire git le garde — A18 inscrit le renvoi au commit
  9975a12). L'en-tête du Système redevient vrai tel quel : la source,
  c'est lui.
- **DC-D2 — le Système s'amende dans le même commit que l'UI.** Tout
  commit qui change un pixel ou un geste amende le Système — section
  d'écran ou journal — **dans le même commit**. Le trou A17 prouve que
  « après coup » ne tient pas. La règle vaut pour les valeurs (jetons,
  géométries) comme pour les surfaces (sections d'écran).
- **DC-D3 — un seul compteur de glyphes.**
  [`assets/icones/README.md`](../assets/icones/README.md) est le
  contrat ; le corps du Système **renvoie** à l'inventaire et ne compte
  plus. Les nombres du journal restent — ce sont des faits datés, pas
  des états courants.
- **DC-D4 — les maquettes meurent à la validation.** Une maquette de
  plan reste l'outil d'exploration **avant** le GO ; au GO du CE, la
  variante retenue est reversée au Système (section d'écran, aux
  jetons) et la maquette est **supprimée** — sa validation reste datée
  au journal et au plan. Jamais deux dessins vivants du même écran.
  Les maquettes futures peuvent continuer de recopier les jetons en
  dur : elles sont jetables par contrat.
- **DC-D5 — le doc dit le livré, rien que le livré.** Les valeurs
  dessinées proviennent de `systeme.css` et des composants réels —
  jamais l'inverse, jamais d'invention. « Le vent » et « Tournesol »
  sont retirés de la section Thèmes (l'écart inscrit au journal — s'ils
  reviennent un jour, ce sera par un chantier qui les livre ET les
  mesure au banc de contraste). Le décor fictionnel des écrans
  (contrat Vantis, planning semaine 33) reste : c'est le seed Clarity,
  il est dans la base de démonstration — le décor n'est pas la norme.
- **DC-D6 — une gate de cohérence garde les valeurs.** Un script au
  rang de `contraste.mjs` compare la table des thèmes du Système aux
  `:root` de `systeme.css` (7 thèmes × jetons, valeur pour valeur) et
  vérifie que le corps du doc ne porte aucun compteur de glyphes
  propre. Les règles de process (DC-D2, DC-D4) attrapent les oublis
  d'écrans ; la gate attrape les oublis de jetons. Pour que le script
  ne dépende pas de la prose, les cellules de la table des thèmes
  portent des attributs `data-theme` / `data-jeton` (E3).

## 3. La cible — table des matières du Système après le plan

Le socle (Principes, Couleurs, Thèmes à 7, Typographie, Troncature,
Formes/élévation/signature, Kit unifié, Icônes en renvoi) est révisé ;
les écrans suivent le parcours réel :

1. **Onboarding et guichet de compte** — l'écran 01 existant + le
   guichet partagé (A11) : routage par domaine, géométrie compacte
   40 px dans Réglages.
2. **Boîte de réception** — révisée : mention « Brouillon — » sur les
   fils (variante B, B-D3), dossier Brouillons au rail (clic =
   reprise, B-D1).
3. **Barre d'état et synchronisation** — section neuve : les cinq
   états (à jour · il y a N · cycle courant avec % et barre fine 2 px ·
   échec avec Réessayer · **Hors ligne**, P0-bis), le bouton de relève
   (S-D1), la règle des trois régions rappelée (A4).
4. **Conversation en lecture** — révisée : bloc de brouillon en
   pointillé en dernière position, « Répondre à tous » (A14).
5. **Composition** — révisée : cinq boutons, sélecteur « De » (A10),
   puces de pièces réelles (nom + taille + retrait), poids total,
   refus au plafond (PJ-D3), transfert à trois états (rapatriement /
   arrivée / échec avec Réessayer et renoncement), reprise avec puces.
6. **Réglages** — section neuve : surimpression 800 × 640, deux
   volets, six groupes (A13), rangée d'interrupteur, rangée Langue
   (A15).
7. **Migration** — section neuve : la modale visible et interruptible
   (ADR 0012).
8. **Avis et progression** — révisée : la source « brouillons »
   retirée de la fente (PLAN-BROUILLONS E2).
9. **Ligne de message** — inchangée (repli 104 px documenté, A2/A6).
10. **Journal des amendements** — complété (§4).

## 4. Travaux — le document

1. **Le journal rattrapé, quatre amendements :**
   - **A17** *(daté 2026-08-14, inscrit avec retard — le retard est
     dit)* : police 37 → 39 glyphes (`hourglass_empty`, `warning`),
     tel que réclamé par le commit 27ed056 — le numéro était pris, la
     ligne manquait.
   - **A18** : clôture d'A6 (DC-D1) + les règles de tenue (DC-D2,
     DC-D3, DC-D4) — datées, motivées : commande du Chief Designer.
   - **A19** — *rattrapage groupé des chantiers muets* : brouillons à
     l'écran (B-D1, B-D3, variante B, `--alert` nuit `#ea9a90` —
     2026-08-13), pièces jointes à l'écran (puces, plafond, transfert —
     2026-08-14), réseau en direct (P0-bis — 2026-08-14), chaque fait
     daté de son commit.
   - **A20** — *l'exhaustivité* : inscrit à la livraison d'E2 (les
     sections neuves et révisées, les maquettes supprimées, les deux
     thèmes fantômes retirés).
2. **Les valeurs corrigées** : `#ea9a90` dans la table des thèmes
   (avec sa note — la mention « Brouillon — » est du texte sur trois
   fonds de rangée) ; « arrêté à 36 glyphes » remplacé par le renvoi au
   contrat (DC-D3) ; « Le vent » et « Tournesol » retirés (DC-D5) ;
   la référence morte à `support.js` retirée ; l'en-tête et le
   préambule du journal réécrits post-A18.
3. **Les sections d'écran** (§3) : dessinées aux jetons, dans la
   grammaire du doc (mêmes gabarits de section, largeur 1440,
   annotations en marge), à l'état **validé et livré**. Sources : les
   maquettes pour les variantes retenues, `systeme.css` et les
   composants Svelte pour les valeurs, les plans pour les décisions
   (`R-D*`, `B-D*`, `PJ-D*`, `S-D*`).
4. **Les maquettes supprimées** à la fin d'E2 (DC-D4) — leurs
   validations restent datées aux plans et au journal.

## 5. Travaux — la gate de cohérence

- **`e2e/coherence-systeme.mjs`**, au rang de `contraste.mjs` :
  1. parse la table des thèmes du Système par ses attributs
     `data-theme` / `data-jeton` (posés en E3) et les `:root` de
     `apps/desktop/ui-v2/src/systeme.css` ; toute paire (thème, jeton)
     doit être **identique valeur pour valeur**, dans les deux sens —
     un jeton du CSS absent du doc est un échec autant qu'une valeur
     fausse ;
  2. vérifie l'absence de tout motif « N glyphes » hors du journal, et
     la présence du renvoi à `assets/icones/README.md` ;
  3. échoue en nommant l'écart (thème, jeton, valeur doc, valeur CSS) —
     le remède est toujours le même : amender le Système dans le
     commit fautif (DC-D2).
- La gate ne lit **pas** les sections d'écran (prose et dessins libres) :
  les écrans sont gardés par le process, pas par le script — un parseur
  de dessins serait une fausse promesse.

## 6. e2e et gates

- Rien ne change à l'écran : **aucun test Rust ni e2e existant ne
  bouge**. La suite complète reste la gate de non-régression de chaque
  étape (piège connu : ne pas committer pendant la suite — échange de
  conf du banc).
- E3 ajoute `coherence-systeme.mjs` à la suite ; il doit être **rouge
  sur mutation** (preuve à la livraison : une valeur altérée à la main
  dans le doc fait échouer la gate, puis est restaurée).
- `contraste.mjs` inchangé — les paires du doc sont celles du CSS,
  déjà au banc.

## 7. Ordre de livraison

- **E1 — gouvernance et rattrapage** *(un commit docs)* : A17, A18,
  A19 inscrits ; `ui_prototype.html` supprimé ; en-tête et préambule
  du journal réécrits ; `support.js` retiré ; `#ea9a90` ; compteur →
  renvoi ; thèmes fantômes retirés de la section (le retrait complet
  avec re-dessin de la grille attend E2 si besoin de mise en page).
  Après E1, plus aucune contradiction entre normes — le Système est
  juste, pas encore complet.
- **E2 — l'exhaustivité** *(le gros morceau, découpable par section)* :
  les sections de la cible (§3) dessinées ou révisées ; les maquettes
  supprimées ; A20 inscrit. La section **pièces jointes se dessine
  après le verdict terrain du CE** (envoi réel, transfert en ligne,
  reflet Gmail — attendu incessamment) pour ne pas dessiner deux
  fois ; si le verdict tarde, elle se dessine telle que livrée et
  s'amendera au verdict (DC-D2).
- **E3 — la gate** : attributs `data-theme` / `data-jeton` posés sur la
  table des thèmes, `e2e/coherence-systeme.mjs` écrit et branché à la
  suite, preuve « rouge sur mutation » rejouée. À partir d'E3, la
  dérive des valeurs est un échec de build, plus une découverte
  d'audit.

Hors périmètre (dit pour ne pas y glisser) : la renumérotation ou la
réécriture d'amendements existants (l'historique ne se réécrit pas —
A5) ; les spikes (`spikes/`), historiques et non normatifs, restent où
ils sont ; les ADR restent la norme de l'architecture — le Système
norme l'écran ; la traduction du Système (il est en français, comme le
catalogue de référence — A15) ; tout changement de forme du doc
(il reste un HTML autonome à valeurs inline — c'est la gate qui
fiabilise, pas un changement de format).
