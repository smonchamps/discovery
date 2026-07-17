# Revue de clôture — Phase 2 « je travaille dans mes mails » (2026-07-17)

Le triage et l'écriture du plan ([PLAN.md](PLAN.md) §4) sont livrés, validés
sur compte réel (envois, coupures, redémarrages) et re-mesurés contre les
budgets. Discovery lit, trie, répond, transfère et envoie — offline-first,
sans jamais perdre ni dupliquer un envoi.

## 1. Livré, contre le plan

| Exigence du plan | État | Preuve |
|---|---|---|
| Actions lu/non-lu, archiver, supprimer, étoiler — optimistes, file offline rejouable | ✅ | `pending_actions` rejouée en tête de synchro ; coupure simulée = file intacte (tests) |
| Composer, répondre, transférer | ✅ | Validation stricte à la frontière, « Re:/Fwd: » sans empilement, citation (`> ` / bloc transféré), In-Reply-To/References |
| Envoi SMTP, boîte d'envoi « jamais perdu » | ✅ | Journal AVANT réseau, machine à états, **quarantaine anti-fantôme** ([ADR 0003](adr/0003-boite-envoi-smtp.md)) ; validé sur Gmail réel : un seul exemplaire, Message-ID du journal |
| Brouillons synchronisés | ✅ (poussée) | Locaux d'abord (autosauvegarde, plus de texte perdu), reflétés dans le dossier Brouillons Gmail (APPEND \Draft + APPENDUID, tombstones, garde UIDVALIDITY) |
| Raccourcis clavier | ✅ | c, r, f, s, e, Suppr, j/k, Échap — avec garde de saisie |
| **Gate : zéro perte prouvée par coupure/crash + E2E des parcours critiques verts** | ✅ | Tests de coupure et de redémarrage (unitaires **et** terrain) ; 5 parcours E2E sur la vraie fenêtre (~5 s, `e2e/`) |

## 2. Budgets re-mesurés (release, 50 000 messages — `e2e/mesure.mjs`)

| Métrique | Phase 1 | Phase 2 | Budget |
|---|---|---|---|
| Démarrage → fenêtre utilisable | 348 ms | **350 ms** | < 1 s ✅ |
| RAM résidente (working set privé) | 84,5 Mo | **89,6 Mo** | < 200 Mo ✅ |
| Page de liste servie (SQLite) | — | **3,82 ms** | — |

Toute la Phase 2 (boîte d'envoi, brouillons, citations, étoile) coûte
**+5,1 Mo de RAM et +2 ms de démarrage**. 131 tests Rust, 5 parcours E2E,
clippy `-D warnings`.

## 3. Enseignements consignés

1. **Une fenêtre Tauri se pilote via CDP** : WebView2 honore
   `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port` et
   Playwright s'y attache par `connectOverCDP` — ni tauri-driver, ni
   msedgedriver, aucune danse de versions. Les E2E sont déterministes par
   construction : base seedée jetable, compte factice au jeton invalide,
   configuration OAuth retirée de l'environnement (`e2e/README.md`).
2. **L'authentification SMTP doit se jouer à la connexion** : un token
   expiré répond 5xx, indistinguable d'un refus permanent du message —
   `test_connection` à l'ouverture + refresh silencieux garantissent
   qu'un message sain ne finit jamais « rejeté » pour cause de token.
3. **Le doublon est pire que le retard** : un envoi interrompu en plein
   vol est mis en quarantaine, jamais renvoyé sans l'utilisateur ; le
   Message-ID généré AVANT l'envoi relie journal et message parti.
4. **`html_to_text` de mail-parser ne coupe que sur `<p>`/`<br>`**
   (mesuré) : les citations passent par une pré-passe fins-de-blocs →
   `<br>` sur HTML déjà assaini, sinon une newsletter citée tient sur
   une ligne.
5. **Un horodatage à la milliseconde ne suffit pas comme repère de
   synchro** : deux sauvegardes dans la même milliseconde masquaient une
   édition (attrapé par TDD) — l'horodatage d'un brouillon avance
   strictement à chaque sauvegarde.
6. **`lettre` exige un destinataire** : un brouillon sans adresse valide
   n'est pas poussable — il reste local, comportement documenté par test.

## 4. Reporté, volontairement

- **Déplacer vers un dossier** — sans navigation de dossiers dans l'UI,
  ce serait une fonctionnalité fantôme (résultat invisible) ; arrive en
  Phase 3 avec dossiers/libellés. Archiver et supprimer couvrent le
  triage réel d'INBOX.
- **Tirage des brouillons** (éditer ici un brouillon créé ailleurs) —
  Phase 3, avec la synchro multi-dossiers.
- **`References` complet du fil** (on émet In-Reply-To + le References
  minimal), **HTML sortant**, **CC/BCC** — le texte brut assumé de
  l'ADR 0003.
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — toujours côté produit-owner, chemin critique
  du lancement public (mode Testing : refresh tokens 7 jours).

## 5. Décision

**GO Phase 3 — « recherche, multi-comptes, échelle »** (PLAN.md §4) :
recherche plein-texte FTS5 (< 100 ms sur 100 000 messages), multi-comptes
(Gmail + Microsoft + IMAP générique) et boîte unifiée, pièces jointes,
notifications Windows, threading des conversations — et les reports
ci-dessus qui trouvent leur place naturelle (dossiers, tirage des
brouillons). **Gate de sortie : budgets tenus avec 3 comptes et 200 000
messages cumulés.**
