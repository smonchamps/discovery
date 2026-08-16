# Annotations CE — vers une UI v3

Séance d'annotation du prototype `prototype-classique.html` (mode Classique,
Système v2 « Wada »), panneau navigateur intégré. Chaque entrée est un verdict
CE consigné tel quel ; la synthèse et la proposition v3 viendront après la
séance.

## 2026-08-16

### 1. Navigation latérale (`.nav`, rail 248 px)
**Verdict CE : conserver la v2 telle quelle.**
Bloc concerné : le rail complet — boîtes (Réception/Envoyés/Brouillons/
Archives/Indésirables/Corbeille, pastille de non-lus), section Bibliothèque
(Fichiers, Contacts, Collections, Parcours, Extraits, Modèles), Étiquettes,
section Boîtes (Toutes les boîtes, Travail…). Aucun changement en v3.

### 2. En-tête (`header.entete`, 52 px)
**Verdict CE : remplacer l'en-tête v2 par celui de la maquette Classique.**
Composition retenue (telle que dans `prototype-classique.html`) :
- marque « Wind » 18 px + trait hitofude (SVG accent, décalé de 3 px sous la
  ligne de base, sans tuile-enveloppe — A30) ;
- champ de recherche central (max 520 px, placeholder « Chercher un message,
  une personne, un fichier ») ;
- à droite : bouton accent « Écrire » et bouton « Réglages ».

### 3. Bandeau d'en-tête de liste (`.listeTete`)
**Verdict CE : ajouter ce bandeau en v3, SANS le bouton « Tout marquer lu ».**
On garde le titre de la boîte (h1 16 px, « Boîte de réception »…) en tête du
volet liste ; le bouton mini « Tout marquer lu » est écarté.

### 4. Liste des messages (`.liste` / `.ligne`)
**Verdict CE : la liste des emails v3 suit le format de la maquette.**
Dessin des pistes (A29.3/A30) : lignes continues séparées au filet, 14 px,
grille avatar (28 px, cliquable = sélection lot) · nom + heure tabulaire ·
objet · aperçu une ligne. Non-lu en 700, survol en teinte légère, ligne
choisie en teinte + liseré gauche accent. Rang de puces sous l'aperçu quand
il existe : étiquette (accent), « Brouillon : » en alerte dans l'aperçu,
« Remonté ce matin », note de ligne en italique.

### 5. Filtres du volet liste (`.filtres` — Tous / Non lus / Brouillons)
**Verdict CE : conserver le bloc v2 tel quel.**
Les onglets de filtre de la maquette (Tous / Non lus / Brouillons en pied de
liste) ne remplacent pas l'existant : la v3 garde le dispositif de filtrage
de la v2.

### 6. Volet de lecture (`.voletLecture` / `.lecture`)
**Verdict CE : ce layout remplace celui de la v2 — sous réserve des
exceptions listées ci-dessous (en cours de dictée).**
Composition retenue :
- titre du fil (h1 24 px) ;
- sous-titre en puces : n messages, n fichiers, étiquettes (accent) ;
  bouton nu « Tout déplier » à droite ;
- messages en cartes (`.carteMsg`) : les anciens repliés sur une ligne
  (avatar · nom · résumé · quand), le dernier déplié en carte pleine
  (en-tête avec adresse et destinataire, corps 68ch max, section
  « Fichiers joints » en puces) ;
- ~~note privée du fil sur fond jaune (« jamais transmise »)~~ → retirée,
  voir exception a.

**Exceptions CE :**
- **a. Note privée de fil (`.noteFil`) : non implémentée.** Le bloc jaune
  « note privée, jamais transmise » ne fait pas partie de la v3.
- **b. Bouton « Plus » de la barre d'actions (`.barreActions`) : écarté à ce
  stade.** La barre d'actions du fil garde ses boutons directs (Répondre,
  Répondre à tous, Transférer, Supprimer…) sans le menu « ⋯ Plus ».
