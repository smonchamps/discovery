# PLAN-RETOURS-6 — signatures, envoi différé, important, entête du composeur

> Chantier ouvert le 2026-08-21 (`/chantier`), sur quatre retours CE :
> (1) feature — gestionnaire de signature email ; (2) feature — envoyer
> un email en différé à une date et heure choisies ; (3) feature —
> bouton « important » dans le composeur ; (4) amélioration — l'entête
> de la fenêtre « Nouveau message » à la même couleur que le pied de
> page de Wind.

---

## Constat — faits vérifiés sur pièces (2026-08-21)

### 1. Le composeur et son entête (R4)

- Le pied de page de Wind (la barre d'état, `App.svelte` `.statut`)
  est sur **`var(--panel)`** avec un filet `var(--border)`.
- L'entête du composeur (`Composition.svelte` `.tete`) n'a **pas de
  fond propre** : il hérite du fond de la carte, `var(--surface)`.
- La barre de mise en forme du composeur (`.format`, bas de carte) est
  **déjà** sur `var(--panel)` — le geste demandé (entête assorti au
  pied) donne à la carte une symétrie haut/bas qui existe à moitié.
- Le texte de l'entête (kicker) est `var(--muted)` ; la combinaison
  `--muted` sur `--panel` est déjà en service dans la barre d'état
  (12 px) — la gate des contrastes la couvre sur les 28 thèmes.

### 2. La signature (R1) : ce que le produit sait déjà

- Le corps du composeur est **riche** depuis la 0.2.0
  (PLAN-COMPOSITION-HTML) : `contenteditable`, sortie = allowlist
  ammonia exacte, LA frontière `frontiere_corps` (commands.rs) assainit
  tout HTML qui entre (save_draft, queue_send). Une signature riche
  emprunterait le même chemin — **aucune surface d'injection nouvelle**.
- Le stockage : la table `prefs` (clé/valeur texte, `text_pref` /
  `set_text_pref`) existe et sert déjà la langue et les notifications.
  Une clé par compte (`signature.<account_id>`) ne demande **aucune
  migration**.
- Réglages est en groupes (Comptes, Thèmes, Affichage, Notifications,
  Raccourcis, À propos) — la règle maison : « un groupe ne s'expédie
  qu'avec du contenu réel ». Un groupe « Signature » aurait du contenu
  réel : un éditeur par compte.
- **Piège identifié (anti-churn)** : `vide()` du composeur juge le
  corps sur son `textContent`. Une signature insérée à l'ouverture
  rendrait toute composition vierge « non vide » → fermer sans un mot
  créerait un brouillon fantôme à chaque ouverture. La garde : une
  composition sans frappe (`corpsModifie` faux), sans destinataire,
  sans objet et sans pièce reste vide, signature ou pas.

### 3. L'envoi différé (R2) : ce que l'architecture permet — et sa limite

- La boîte d'envoi journalise AVANT tout réseau (`outbox`, règles
  d'or) ; `outbox_to_send` sert `state = 'queued'` dans l'ordre
  d'émission ; `flush_outbox` vidange. Une colonne `send_at_epoch`
  (migration additive, patron existant) + un filtre « échu seulement »
  dans `outbox_to_send` suffit au cœur : un envoi programmé est un
  envoi journalisé qui **attend son heure** — les deux règles d'or le
  couvrent gratuitement (crash : il survit ; jamais renvoyé deux fois).
- Le réveil : la vidange part aujourd'hui en fin de cycle (30 min),
  à la passe légère (5 min), au geste. Le front sonde déjà
  `outbox_status` toutes les **10 s** (`sonderEnvois`) — y lire
  « un programmé est échu » et déclencher `flush_outbox` donne une
  précision de ±10 s sans mécanique nouvelle.
- **La limite, structurelle** : Wind est un client local. SMTP ne
  connaît pas l'envoi programmé — c'est Wind qui doit être **ouvert à
  l'heure dite**. App fermée à l'échéance : le message part au
  prochain lancement (première vidange). Gmail web programme côté
  serveur ; nous ne pouvons pas. C'est la décision D1.
- L'écho d'Envoyés (PLAN-REACTIVITE E3) naît à `sent` seulement : un
  envoi programmé n'aura **pas d'écho avant son heure** — comportement
  déjà juste, rien à faire.
- La barre d'état dit « N envois en attente » sur `queued` : un
  programmé pas encore échu s'y afficherait comme un envoi qui
  n'arrive pas à partir — **mensonge**. `outbox_status` devra séparer
  « en attente » de « programmé ».
- Le flux actuel **supprime le brouillon** sitôt l'envoi journalisé :
  pour un programmé, le contenu vit alors dans le seul journal
  (pièces comprises, PJ-D2) — l'annulation devra dire vers quoi elle
  rend la main (D2).

### 4. Le bouton « important » (R3)

- Aucune notion de priorité n'existe : ni colonne, ni en-tête composé.
- Le standard du courrier : en-têtes **`X-Priority: 1`** +
  **`Importance: high`** (ce que posent Outlook/Thunderbird ; Gmail
  les lit). `lettre` 0.11 accepte des en-têtes personnalisés (trait
  `Header`) — vérifiable par test sur `formatted()`.
- Portage : colonne `important` sur `drafts` (un brouillon rouvert
  garde son état) et sur `outbox` (le journal porte tout ce qui part),
  migrations additives toutes deux au patron existant.

## Périmètre

**Dans ce chantier** : entête du composeur sur `--panel` (Système
amendé, DC-D2) ; bouton « Important » de bout en bout (composeur →
brouillon → journal → en-têtes SMTP) ; une signature par compte
(Réglages, insertion au composeur selon D3/D4) ; envoi différé selon
D1/D2 (cœur + composeur + barre d'état + annulation) ; e2e de chaque
parcours ; gate complète ; terrain.

**Refus de périmètre explicites (STANDARD §2.6) :**
- **Pas d'affichage des messages importants REÇUS** (drapeau, tri,
  filtre) : R3 demande le composeur ; la lecture des en-têtes entrants
  est un chantier à part (registre des reports si le terrain le
  demande).
- **Pas de multi-signatures par compte** ni de choix de signature à la
  volée dans le composeur : une signature par compte, point. Le
  « gestionnaire » est Réglages.
- **Pas d'images dans la signature** (v1) : le vocabulaire reste
  l'allowlist ammonia existante (pas de `<img>`, pas de data-URI) —
  l'élargir est un chantier de sécurité à part.
- **Pas d'envoi programmé côté serveur** : impossible en SMTP/IMAP —
  voir D1.
- **Pas de reflet des en-têtes « important » dans le brouillon poussé
  à Gmail** (`draft_bytes`) : le reflet distant est cosmétique, l'état
  vit en local et part à l'envoi.
- **Pas de maquette d'étude** : les quatre surfaces suivent la
  grammaire déjà normée du Système (boutons 32 px, puces, cartes) —
  aucun écran neuf, des éléments dans des cadres existants.

## Options et verdicts

### O1 — Où vit la signature ?

| Option | Mécanisme | Verdict |
|---|---|---|
| **A. `prefs`, clé `signature.<account_id>`** | Aucune migration ; `text_pref` existant ; HTML assaini par LA frontière à l'écriture | **Retenue.** Une valeur par compte, chemin le plus court, assainissement au même endroit que tout HTML entrant |
| B. Colonne sur `accounts` | Migration pour une donnée facultative | Rejetée : la table des comptes porte l'identité et la connexion, pas du contenu |
| C. Table dédiée | Prête pour le multi-signatures | Rejetée : hors périmètre (refus explicite) — YAGNI |

### O2 — Le réveil de l'envoi différé

| Option | Mécanisme | Verdict |
|---|---|---|
| **A. Sonde front existante (10 s)** | `outbox_status` rapporte la prochaine échéance ; `sonderEnvois` déclenche `flush_outbox` quand elle est passée | **Retenue.** Précision ±10 s, zéro mécanique neuve, la vidange reste LA porte unique |
| B. Minuterie backend (fil dédié, façon veilleur) | Précision à la seconde | Rejetée : un fil, un réveil, une reconnexion à gérer — pour gagner ~10 s sur un geste qui se compte en minutes/heures |
| C. Programmation serveur | Précision parfaite, app fermée comprise | Impossible : SMTP/IMAP ne l'offrent pas (Gmail web le fait côté Google, hors de notre portée) |

### O3 — Les en-têtes du « important »

`X-Priority: 1` **et** `Importance: high`, les deux (c'est la paire
que posent les clients mûrs ; certains destinataires ne lisent que
l'un des deux). En-têtes personnalisés `lettre` (trait `Header`),
prouvés par test sur le message formaté. Pas de `X-MSMail-Priority`
(redondant avec les deux autres, personne ne le lit seul).

### O4 — Le geste « Envoyer plus tard » au composeur

Un bouton à côté d'« Envoyer » (icône `schedule_send`) ouvrant une
petite carte au-dessus du pied : `<input type="datetime-local">`
(contrôle natif WebView2 — calendrier + heure, localisé, clavier),
minimum = maintenant, bouton « Programmer ». Toast honnête :
« Envoi programmé pour {date} » — jamais « Message envoyé ». Même
patron de surimpression locale que le nuancier de couleurs (R4).

## Étapes

- **E1 — l'entête du composeur** (R4) : `.tete` passe sur
  `var(--panel)` (+ le même filet qu'aujourd'hui) ; Système amendé au
  même commit (DC-D2, A-n). Gate : contrastes 28 thèmes.
- **E2 — « Important » de bout en bout** (R3, TDD) : RED sur mail-smtp
  (le message formaté porte `X-Priority: 1` + `Importance: high` quand
  le journal dit important, aucun en-tête sinon) et sur le cœur
  (colonnes `drafts.important` / `outbox.important`, aller-retour
  enqueue/relecture, reprise de brouillon) ; puis bouton bascule au
  pied du composeur (icône `priority_high`, `aria-pressed`, état
  repris avec le brouillon). e2e : composer → marquer important →
  brouillon rouvert le garde. Système amendé (icône + geste, A-n).
- **E3 — la signature** (R1, selon D3/D4) : commandes
  `signature_get` / `signature_set` (assainissement par LA frontière à
  l'écriture) ; groupe « Signature » aux Réglages — un éditeur riche
  par compte (barre réduite : gras/italique/souligné),
  enregistrer/effacer, **et le choix de portée par compte** (D4 :
  « nouveaux messages seuls » ou « aussi réponses et transferts »)
  avec un geste « Appliquer à tous les comptes » ; insertion à
  l'ouverture du composeur (nouveau message : sous deux lignes vides ;
  réponse/transfert si la portée du compte le dit : entre l'amorce et
  la citation) ; **garde anti-churn** : une
  composition sans frappe/destinataire/objet/pièce reste vide malgré
  la signature (fermer ne crée rien). Changement de compte émetteur en
  cours de composition : la signature ne se réinsère PAS (le corps
  appartient à l'utilisateur dès l'ouverture). e2e : signature posée
  aux Réglages → nouveau message la contient → fermer sans frappe ne
  laisse aucun brouillon. Système amendé (A-n).
- **E4 — l'envoi différé** (R2, selon D1/D2, TDD) : RED cœur
  (`outbox_to_send(account, now)` ignore un `send_at_epoch` futur et
  sert un échu ; un programmé survit au redémarrage ; jamais purgé ni
  écho avant `sent`) ; migration additive `outbox.send_at_epoch` ;
  `queue_send(sendAtEpoch?)` ; `outbox_status` sépare « en attente »
  de « programmé » et rapporte la prochaine échéance ; front : geste
  O4, barre d'état « N programmé(s) · prochain à {heure} », réveil par
  la sonde 10 s, annulation selon D2. e2e : programmer dans le futur →
  rien ne part, la barre le dit ; échéance passée → l'envoi part.
  Système amendé (A-n).
- **E5 — qualité et sortie** : revue à regard neuf (`/code-review
  high`), gate complète, **terrain (STOP 2)** avec commandes
  PowerShell prêtes, docs (journal A-n, ETAT, DETTE si report),
  CHANGELOG **avant** release (§2.9 ⚠️), version selon D5 —
  capacités nouvelles → **0.4.0** (MINEUR).

## § Réalisation (2026-08-21)

- **E1 (entête)** : `.tete` du composeur sur `var(--panel)` — symétrie
  avec la barre de mise en forme et le pied de page. Système A66,
  écran 04 amendé. e2e : l'entête et la barre d'état ont le même fond
  calculé (comparaison de `getComputedStyle`, tenue sur tout thème).
- **E2 (important)** : TDD — 3 RED montrés (aller-retour brouillon,
  aller-retour journal, en-têtes SMTP absents) puis GREEN. Colonnes
  additives `drafts.important` / `outbox.important` (patron
  `add_missing_columns`) ; basculer le marquage SEUL avance
  l'horodatage (l'anti-churn ne l'avale pas — test dédié) ; en-têtes
  `X-Priority: 1` + `Importance: high` (en-têtes personnalisés
  `lettre`, trait `Header`), un envoi ordinaire n'en porte AUCUN
  (test). Bouton bascule au pied (aria-pressed, langage visuel de la
  barre de format). Système A67.
- **E3 (signature)** : commandes `signature_get`/`signature_set`
  (stockage `prefs`, clé par compte, AUCUNE migration ; HTML assaini
  par LA frontière `frontiere_corps` à l'écriture) ; groupe
  « Signature » aux Réglages (éditeur riche réduit G/I/S, portée D4
  par compte + « Appliquer à tous ») ; insertion à l'ouverture
  (nouveau : sous deux lignes vides ; réponse/transfert : entre
  l'amorce et la citation, si la portée du compte le dit) ; garde
  anti-churn `corpsAuto` (fermer sans frappe ne sème rien — e2e).
  Système A68.
- **E4 (envoi différé)** : TDD — 2 RED montrés (le programmé partait
  tout de suite ; l'annulation ne recréait rien) puis GREEN. Colonne
  `outbox.send_at_epoch` ; filtre « échu seulement » DANS
  `outbox_to_send` (la porte unique de la vidange) ;
  `enqueue_outbox_full` (brouillon-ancre + échéance, une transaction) ;
  `annuler_envoi_programme` (brouillon ENTIER recréé — destinataires,
  corps, marquage, pièces avec octets — dans une transaction ; ne vise
  que les entrées programmées PAS ÉCHUES, revue) ; `outbox_status`
  sépare « programmés » des « en attente » et rapporte la prochaine
  échéance. Front : carte « Envoyer plus tard » (+1 h préréglée,
  sémantique D1 dite en clair), toast d'échéance (jamais « envoyé »),
  barre d'état « N programmé(s) · départ {quand} » (repos daté, pas de
  trait qui boucle), fente d'avis avec « Annuler l'envoi » (D2), départ
  par minuterie courte armée par la sonde 10 s (< 60 s de l'échéance,
  précision ~1 s — jamais de minuterie longue qui survivrait à une
  annulation). Système A69.
- **Glyphes** : 3 neufs (`priority_high`, `schedule_send`,
  `signature`), sous-ensemble 58 → 61 (25 564 octets), cache-buster
  `?v=61`, README amendé, **preuve apercu.html rejouée : PASS 62/62
  ligatures repliées** (serveur local, 2026-08-21).
- **Revue à regard neuf** (`/code-review high`) : (1) course
  annulation ↔ vidange à la seconde d'échéance (la vidange vit HORS de
  la file sérialisée) → l'annulation ne vise plus que les entrées
  programmées pas échues, corrigé + test étendu ; (2) bras de match
  inatteignable (`Sending if programme`) → simplifié ; (3) assumé : la
  lecture de la signature partage la file sérialisée du contexte de
  réponse (latence marginale, garde `corpsModifie` intacte).
- e2e : nouveau spec `refonte-retours-6.spec.js` (4 parcours) ;
  99 → **103** ; tests Rust 353 (+5).
- **Gate complète (2026-08-21) : verte.** fmt OK ; build ui-v2 sans
  avertissement ; contrastes 28 thèmes / 700 paires ; cohérence
  Système 476 valeurs ; garde du thread principal 72 commandes ;
  clippy `-D warnings` muet ; tests Rust tous verts (`--all-targets`
  + `--doc`) ; e2e **103/103**. Un andon levé en route : le premier
  passage e2e avait un rouge — mon spec visait `ligne` au dossier
  Brouillons, dont les rangées portent `ligne-brouillon` (défaut du
  TEST, pas du produit) ; corrigé, spec rejoué en isolation (4/4)
  puis suite entière rejouée (103/103).

## § Terrain (STOP 2, 2026-08-21) — première passe : 3 constats, corrigés le jour même

Verdicts CE sur les cinq points : 1 OK · 2 constats · 3 constats ·
4 OK-mais · 5 OK.

1. **Entête (R4)** : OK.
2. **Important (R3)** — deux constats :
   - *« Reçu mais pas marqué important sur Gmail web ni sur Wind. »*
     **Instruit** : Gmail web n'affiche AUCUN indicateur pour
     `X-Priority`/`Importance` (son marqueur « important » est
     algorithmique et ignore ces en-têtes) ; Outlook/Thunderbird, eux,
     montrent le « ! ». Les en-têtes partent (prouvé par test sur le
     message formaté) — vérifiable sur le reçu : Gmail web → ⋮ →
     « Afficher l'original » → `X-Priority: 1`. Côté Wind en
     réception : refus de périmètre du plan (chantier à part si le
     terrain le demande).
   - *Placement* : le bouton passe du pied à la **barre de mise en
     forme**, au format de ses voisins (icône seule, sans libellé) —
     corrigé, Système A67 amendé.
3. **Signature (R1)** — deux constats, corrigés :
   - « Appliquer à tous les comptes » copie désormais la **signature
     ET la portée** et **se voit** (éditeurs + interrupteurs des autres
     blocs mis à jour à l'écran) ;
   - le composeur **recharge la signature au changement de compte
     émetteur** (menu « De ») — tant que le corps n'a pas été touché
     (une frappe posée prime) et sur une composition neuve (une
     réponse garde sa citation) ; jeton dédié, dernier-gagne.
   Système A68 amendé ; e2e du parcours étendus (les deux gestes).
4. **Envoi différé (R2)** : OK — mais l'accumulation de boutons
   faisait replier « Envoyer plus tard » sur deux lignes (capture).
   Corrigé : plus AUCUN libellé de bouton ne se replie
   (`white-space:nowrap`), le pied wrappe par bouton entier, et le
   déménagement d'« Important » (point 2) allège le pied.
5. **Régressions** : OK.

Re-gate après corrections : verte (spec retours-6 4/4 en isolation,
puis suite entière).

### Deuxième passe (2026-08-21) : 4 OK, 2 retouches, faites le jour même

1. Important en barre de style : OK — **infobulle** précisée :
   « Marquer le message comme important » (clé dédiée, libellé mort
   retiré des catalogues).
2. Pied sans repli : OK.
3. Appliquer à tous : OK.
4. Rechargement au changement de compte : OK — **étendu aux réponses
   et transferts** dont le corps n'a pas été touché : le gabarit de
   corps posé à l'ouverture (une recette `(signature|null) → HTML`)
   recompose amorce et citation à l'identique, seule la signature
   change, et la portée D4 du NOUVEAU compte décide ; une reprise de
   brouillon n'a pas de gabarit (son texte est la seule vérité). e2e
   étendu (réponse : signature du compte 1 → bascule compte 2 → la
   citation reste, la signature suit).

### Troisième passe (2026-08-21) — **TERRAIN VALIDÉ**

1. Infobulle : OK. 2. Signature en réponse/transfert au changement de
compte : OK. — Les cinq points du chantier sont validés au terrain,
trois passes le même jour, chaque constat corrigé dans la session.

## § Décisions CE — tranchées le 2026-08-21 (STOP 1, GO)

- **D1 — envoi différé, sémantique client** : — *Réponse CE
  (2026-08-21) : « Accepter »* — l'envoi différé est livré avec la
  sémantique locale (part à l'heure dite si Wind tourne, sinon au
  prochain lancement), dite honnêtement dans l'UI.
- **D2 — annulation d'un envoi programmé** : — *Réponse CE
  (2026-08-21) : « Retour en brouillon »* — annuler retire l'envoi du
  journal et recrée un brouillon complet (destinataires, corps, pièces)
  — rien ne se perd, le geste est réversible. Visible en barre d'état
  + fente d'avis.
- **D3 — forme de la signature** : — *Réponse CE (2026-08-21) :
  « Riche »* — éditeur `contenteditable` avec barre réduite
  (gras/italique/souligné) aux Réglages, même vocabulaire ammonia que
  le composeur.
- **D4 — portée de la signature** : — *Réponse CE (2026-08-21), mot
  pour mot : « Choisir de l'insérer sur les nouveaux messages ou sur
  les réponses dans les réglages. Permettre d'appliquer ce choix sur
  tous les comptes ou compte par compte. »* — la portée est donc un
  RÉGLAGE : par compte, « nouveaux messages seuls » ou « aussi
  réponses et transferts », avec un geste « Appliquer à tous les
  comptes ».
- **D5 — publication** : — *Réponse CE (2026-08-21) : « Une 0.4.0 »* —
  une seule release MINEUR qui emporte les quatre retours, après
  terrain validé et CI verte.
