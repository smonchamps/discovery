# Plan — L'interface en plusieurs langues

Commande (2026-08-12) : support multilingue de l'interface. Le
prototype est muet sur la langue — il parle français ; le Système
complète (A6), l'amendement **A15** portera la spécification. Le
français du prototype devient le catalogue `fr`, mot pour mot : le
prototype RESTE la cible exacte du français.

## 1. État des lieux — où vit le français aujourd'hui

| Surface | Constat | Sort |
|---|---|---|
| Les 12 composants Svelte (~3 200 lignes) | de l'ordre de 150 chaînes uniques en dur : libellés, aria-labels, placeholders, toasts, états vides, confirmations, libellés de boîtes (`Nav.svelte`) | catalogue, **E1** |
| `lib/quand.js` | mois, jours et formes de dates du prototype en dur (« Hier », « Lundi », « 1ᵉʳ août ») | tables par langue, **E1** |
| `index.html` | `lang="fr"` figé | dynamique, **E1** |
| Réponse pré-remplie | « Bonjour {prénom}, » (`Composition.svelte`) | **E2** (L-4) |
| `mail-core/notify.rs` | « {n} nouveaux messages » — le seul texte du cœur montré à l'OS | **E2** |
| Erreurs `mail-core` | françaises (thiserror), traversent en `String` et s'affichent interpolées (« Transfert impossible : {err} ») | enveloppes traduites à **E2**, détail verbatim (L-3) |
| `mail-core/compose.rs` | préfixes `Re:`/`Fwd:` déjà neutres | RIEN à faire |
| e2e | les sept specs s'ancrent sur les libellés français | le fr reste canonique (L-6) |

## 2. Le contrat (A15, à inscrire au journal à la livraison)

- La langue est une préférence d'**application** (pas par compte),
  persistée en base dans `prefs` (clé `lang`) — le patron des bulles
  (`notif_pref`). Pas localStorage : le thème peut y vivre parce que
  seul le web le lit ; la langue traverse la frontière, le shell la
  lit pour composer les notifications.
- Défaut au premier lancement : la langue du système si couverte,
  sinon `fr`. L'UI pose la clé au premier lancement
  (`navigator.language`) ; le shell lit `prefs.lang`, défaut `fr`
  tant que la clé n'existe pas.
- **Application immédiate**, sans redémarrage — le geste du thème.
- Le réglage vit dans **Réglages > Affichage** : une rangée
  « Langue » — pas de groupe neuf pour une rangée (règle des groupes
  réels, A13).
- `<html lang>` suit la langue (accessibilité, lecteurs d'écran).
- Les formes de dates anglaises par **transposition** de la grammaire
  du prototype — l'heure reste sur 24 h dans les deux langues : on
  livre une langue, pas une locale régionale.

| Contexte | fr (prototype) | en (A15) |
|---|---|---|
| Aujourd'hui, liste | `09:12` | `09:12` |
| Veille | `Hier` | `Yesterday` |
| 2 à 6 jours | `Lundi` | `Monday` |
| Dans l'année | `5 août` · `1ᵉʳ août` | `Aug 5` · `Aug 1` |
| Au-delà | `5 août 2024` | `Aug 5, 2024` |
| Lecture, forme longue | `Aujourd'hui, 09:12` | `Today, 09:12` |

- Les raccourcis (D3) ne bougent pas d'une langue à l'autre — la
  table des Réglages est une référence, pas une traduction.

## 3. L'architecture — un module maison, zéro dépendance

- `lib/texte.js`, à la manière de `theme.js` et `quand.js` :
  catalogues plats `fr.js`/`en.js` (clé → chaîne, gabarits
  `{param}`), fonction `t(cle, params)`, langue courante en `$state`
  — Svelte 5 re-rend au changement, la bascule est immédiate. Pas de
  svelte-i18n ni d'ICU : l'UI n'a AUCUNE dépendance d'exécution et
  n'en gagne pas pour un dictionnaire plat (ADR **0016** à poser).
- Pluriels : le strict besoin du dépôt — une forme singulier/pluriel
  par clé (« {n} éléments ») ; pas de moteur CLDR pour deux langues
  qui n'en ont pas besoin.
- Clé absente du catalogue actif → repli sur `fr` + avertissement
  console en dev. L'audit des clés (jeux fr/en identiques) est une
  assertion de la spec e2e neuve — pas d'outillage nouveau.
- `quand.js` prend mois, jours et gabarits du catalogue — les formes
  restent écrites à la main, exactes, testables (pas d'`Intl` brut :
  il ne produit ni « Hier » contextuel ni « 1ᵉʳ »).
- Côté Rust : commandes `lang_get`/`lang_set` au patron exact de
  `notif_pref_get/set` ; `notify.rs` reçoit la langue en paramètre
  (pas de global) et porte ses gabarits par langue, tests unitaires
  par langue.

## 4. Livraison en deux temps

### E1 — l'extraction et le réglage (l'interface)

Deux mouvements, séparément constatables :

1. **L'extraction à blanc** : inventaire exhaustif, composant par
   composant (aria-labels et `title` compris), tout part dans
   `fr.js`, AUCUN changement visible. Preuve : les sept specs e2e
   passent sans qu'on en retouche une ligne — c'est le gate de
   fidélité de l'extraction.
2. **La langue** : `en.js` complet, rangée « Langue » dans
   Affichage, `lang_get/set` + défaut système au premier lancement,
   bascule immédiate, `<html lang>` dynamique, `quand()` par langue.

e2e : les parcours existants inchangés (fr) ; une spec neuve —
bascule en anglais, balayage des écrans majeurs (nav, liste,
lecture, composition, réglages), audit des clés, aller-retour réel
(changer, relancer, constater), retour au français.

**Gate E1 :** extraction prouvée (e2e verts SANS retouche des
specs), audit de clés fr/en sans écart, aller-retour réel de la
langue, terrain CE dans les deux langues.

### E2 — la frontière (ce qui traverse le cœur)

- Les bulles : `notify.rs` par langue, le shell lit `prefs.lang` à
  l'émission (la garde existante des bulles ne bouge pas).
- Les enveloppes d'erreur de l'UI traduites (« Transfert
  impossible : ») — le détail technique du cœur reste verbatim (L-3).
- « Bonjour {prénom}, » suit la langue de l'app (L-4).

**Gate E2 :** une notification réelle constatée dans chaque langue
(synchro réelle, pas un stub), tests unitaires `notify` par langue,
e2e verts, terrain CE.

## 5. Décisions — tranchées en qualité de Chef Ingénieur

| # | Décision | Tranché |
|---|---|---|
| L-1 | Langues livrées | **fr + en.** Le catalogue est plat : une langue de plus = un fichier + une rangée au réglage — mais on n'expédie que ce qu'on sait relire. |
| L-2 | Bibliothèque ou module maison | **Maison** (`lib/texte.js`). Zéro dépendance d'exécution aujourd'hui ; un dictionnaire plat n'en justifie pas une. ADR 0016. |
| L-3 | Les erreurs du cœur | **Enveloppes traduites côté UI, détail français verbatim** (c'est du diagnostic). Une taxonomie de codes d'erreurs est un chantier à part — à inscrire en DETTE, pas ici. |
| L-4 | « Bonjour {prénom}, » | **Suit la langue de l'app** — l'auteur écrit dans sa langue ; pas de détection de la langue du correspondant (invention). |
| L-5 | Formes anglaises des dates | **Transposition** de la grammaire du prototype, table du §2 — à inscrire à A15. |
| L-6 | e2e | **Le français reste canonique** pour les sept specs ; l'anglais a SA spec (bascule + balayage), pas une duplication des parcours. |

## 6. Refus explicites

- Pas de RTL : aucune langue droite-à-gauche commandée ; c'est un
  chantier de mise en page à part entière, il aura sa commande ou
  n'existera pas.
- Pas de traduction du contenu des mails, des objets, ni des noms de
  dossiers côté serveur — seuls les libellés de l'INTERFACE parlent.
- Pas de langue par compte.
- Pas de re-mappage des raccourcis par langue (D3 est une référence).
- Pas de moteur i18n générique (ICU, CLDR, extraction automatique) —
  deux catalogues plats relus à la main.
- Le prototype reste la cible exacte du français ; l'anglais s'y
  conforme par transposition (A15), jamais par invention.

---

Le GO sur ce plan ouvre E1.

## 7. Journal de livraison (2026-08-12)

GO du Chef Ingénieur, décisions L-1 à L-6 telles que tranchées au §5.
Amendement **A15** inscrit au journal du Système ; ADR **0016** posée
(catalogues plats maison, zéro dépendance).

- **E1 livré.** Le socle : `lib/texte.svelte.js` (`t()`, langue en
  `$state`, repli sur `fr`, règle du pluriel par langue) et les deux
  catalogues `catalogue.fr.js` / `catalogue.en.js` (196 clés chacun).
  L'extraction : les 12 composants et `quand.js` ne portent plus une
  seule chaîne en dur (balayage vérifié — restent la marque
  « Discovery » et les sondes de mesure, volontairement). Le réglage :
  rangée « Langue » dans Réglages > Affichage (sélecteur natif à la
  grammaire des boutons), `lang_get`/`lang_set` au patron exact de
  `notif_pref` sur un `text_pref` neuf de mail-core (testé), défaut au
  premier lancement = langue du système (posée aussitôt en base),
  bascule immédiate, `<html lang>` dynamique. Harnais e2e : locale du
  WebView épinglée `--lang=fr` (déterminisme par construction — sans
  elle, la suite dépendrait de la machine). Cible de build vite passée
  à `esnext` (await de module pour restaurer la langue AVANT le
  montage — WebView2 seul navigateur).
- **Écarts au plan, dits :** les enveloppes d'erreur et le
  « Bonjour {prénom} », annoncés à E2, sont tombés dans l'extraction
  E1 — l'audit des clés n'aurait pas toléré des trous dans `en`. E2 se
  réduit à ce qui vit en Rust. Les préfixes de sujet (« Re : »/« Tr : »
  → "Re:"/"Fwd:") suivent la langue de l'app, même règle que L-4.
- **E2 livré.** `notify.rs` : `Lang` (Fr/En, `from_pref` — inconnu =
  français, le repli de l'UI), textes des bulles par langue y compris
  les replis « (expéditeur inconnu) »/« (sans sujet) » ; le shell lit
  `prefs.lang` à l'émission, dans la MÊME lecture de base que la garde
  des bulles. Tests unitaires par langue (10 au module).
- **Gates :** e2e **69/69** (les sept specs existantes passent SANS
  retouche — la preuve d'extraction ; la spec neuve
  `refonte-langue.spec.js` porte l'audit des clés fr/en, la bascule
  immédiate, le balayage anglais, l'aller-retour réel par rechargement
  et le retour au français), `cargo fmt`/`clippy -D warnings`/tests
  workspace verts. **Reste : le terrain CE dans les deux langues** —
  dont une bulle réelle en anglais (synchro réelle), seule vérification
  qui ne se joue pas au harnais.
- Au passage : deux écarts `cargo fmt` préexistants du chantier A14
  (« Répondre à tous », non commité) ont été réglés mécaniquement —
  aucun changement de fond.
- **Terrain CE passé le 2026-08-13** : « test ok » — bascule constatée
  dans les deux langues. Les gates E1 et E2 sont closes ; le plan est
  SOLDÉ.
