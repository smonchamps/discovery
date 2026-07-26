# ADR 0014 — Télémétrie de crash : locale, opt-in, sans contenu

Date : 2026-07-26 · Statut : accepté — **validé au terrain le jour
même** : un self-test a paniqué avec une fausse adresse dans le message,
et le `crash-*.json` écrit sur la vraie machine ne la portait pas. Le
terrain a aussi révélé un défaut, corrigé aussitôt (§6).

## Contexte

La Phase 5 ([PLAN.md](../PLAN.md) §5) prévoit une « télémétrie de crash
opt-in ». Reconnaissance faite sur le code, trois faits ont cadré la
décision :

1. **On part d'une page blanche** : aucune infrastructure de log ou de
   télémétrie (`tracing`, `log`, `sentry` : absents), aucun panic hook,
   aucune table de préférences.
2. **Un « crash » ici est étroit** : `unsafe_code = "forbid"` sur tout le
   workspace, `unwrap_used`/`expect_used` en warning et zéro en
   production → un crash de notre code est presque toujours un **panic
   Rust**, capturable (`panic = "unwind"` par défaut). La plupart sont
   même interceptés (Tauri pour les commandes, nos `spawn_blocking` pour
   les tâches) ; les vrais plantages sont rares.
3. **La surface vie privée est réelle et précise** : un rapport naïf
   (`format!("{err:?}")`) fuiterait — `Error::InvalidEmailAddress`
   contient **une adresse**. C'est le piège du §9 de la passation (un
   diagnostic qui divulguait des identifiants).

## Décisions (arbitrées par le Chef Ingénieur)

1. **Destination : fichier local seul.** Un fichier par plantage dans
   `app_data_dir/crashes/`. **Aucun réseau, aucun tiers.** L'app montre
   les rapports ; l'utilisateur décide de les envoyer. Colle au modèle
   bêta où le Chef Ingénieur dépouille chaque retour lui-même. Contre
   Sentry (envoi à un tiers, dépendance externe) et contre un endpoint
   auto-hébergé (à monter/maintenir), écartés pour la v1.

2. **Périmètre : panics du backend Rust seulement.** Un panic hook les
   capture — là où vit la logique. Les minidumps (crashes natifs) et les
   erreurs JS sont hors v1 : complexité disproportionnée pour un code
   sans `unsafe`.

3. **Opt-in, off par défaut**, révocable, demandé **une fois**
   (état « unset » → bandeau ; puis plus jamais).

4. **Le message du panic est SUPPRIMÉ.** C'est le seul champ libre pouvant
   porter une donnée personnelle. Le rapport ne garde que des artefacts
   de **code et d'environnement** : localisation `fichier:ligne`, pile de
   symboles, versions app/OS, horodatage. Prouvablement sans donnée
   personnelle, plutôt qu'une rédaction par motif fragile à prouver.

## Architecture — le motif du projet

- **Pur, dans `mail-core`** ([`crash.rs`](../../crates/mail-core/src/crash.rs)) :
  `redact(RawPanic) -> CrashReport` écarte le message. Zéro dépendance,
  zéro I/O. C'est le cœur prouvé.
- **Platform, dans le desktop** ([`telemetry.rs`](../../apps/desktop/src/telemetry.rs)) :
  le panic hook, le consentement, l'écriture du fichier, les commandes.

**Deux règles dures, tirées de la reconnaissance :**
- **Le hook ne touche jamais la base.** Elle est peut-être la cause du
  panic, ou tient un verrou empoisonné. Le consentement vit donc dans un
  **fichier** (`telemetry.json`), lu au démarrage dans un `AtomicBool`
  que le hook consulte ; le rapport s'écrit en `std::fs` pur.
- **Le hook ne panique jamais à son tour** (un panic pendant un panic =
  `abort`) : tout y est enveloppé (`catch_unwind`) et sans `unwrap`.

## Preuves

- **Invariant en mémoire** (`mail-core`,
  `le_rapport_n_emporte_aucune_donnee_du_message`) : un message avec
  adresse + sujet → le rapport, scanné via sa représentation `Debug`
  (donc tout champ futur compris), n'en garde rien.
- **Invariant sur DISQUE** (desktop,
  `le_fichier_ecrit_ne_contient_aucune_donnee_du_message`) : ce qui
  compte vraiment — les octets sérialisés du fichier ne portent ni
  l'adresse, ni le sujet, mais gardent la localisation.
- **Utilité conservée** (`le_rapport_garde_de_quoi_situer_le_bug`) : la
  localisation et la pile survivent, sinon le rapport serait vide.
- **Étanchéité E2E** : en test, consentement forcé `disabled` et zéro
  rapport (garde `DISCOVERY_DB_PATH`) ; un E2E tient l'absence des deux
  bandeaux.

L'implémentation de `redact` est **triviale** (un champ écarté) : pas de
RED qui apprenne quelque chose (§2.4), la valeur est l'invariant permanent
tenu par les deux tests ci-dessus.

## Validation terrain (à jouer)

Le hook ne se prouve qu'en situation (comme les notifications, §7.2).
Une commande debug-only, `telemetry_selftest_panic`, provoque le panic —
son corps n'existe pas en release. Protocole, en session **debug**
(`cargo run -p discovery-desktop`) :

1. Au démarrage, le bandeau opt-in apparaît → **Activer**.
2. Ouvrir la console de la WebView (F12) et invoquer :
   `window.__TAURI__.core.invoke('telemetry_selftest_panic')`.
3. Rouvrir l'app : le bandeau « Discovery a rencontré un problème »
   apparaît → **Ouvrir le dossier**.
4. Ouvrir le `crash-*.json` : vérifier qu'il porte la localisation et la
   pile, et **PAS** `faux@exemple.fr` ni « secret ».
5. Refaire depuis un état « unset » (supprimer `telemetry.json`) en
   choisissant **Non merci** : aucun fichier ne doit apparaître même
   après le self-test.

## Trouvaille du terrain (2026-07-26) — le double panic

Le self-test a prouvé la rédaction (la fausse adresse était absente du
fichier), mais a révélé un défaut : **un panic sur le thread principal
en produit DEUX**. L'original (au site du bug) tente de se dérouler,
traverse la frontière FFI de WebView2 (nounwind), et déclenche un second
panic `cannot unwind` qui aborte le process. Le hook s'exécutait pour les
deux ; les deux fichiers portaient le même horodatage à la seconde →
**le second (l'abort, inutile) écrasait le premier (le bug, utile)**. Le
Chef Ingénieur a ouvert un rapport pointant `core/panicking.rs` au lieu
du site réel.

Corrigé le jour même, deux gardes :
- **Compteur `SEQ`** dans le nom de fichier : deux rapports d'une même
  seconde ne se marchent plus dessus (correction de fond, testée).
- **Filtre du panic secondaire** (`is_secondary_nounwind`) : le
  `cannot unwind` du runtime n'est pas écrit — au mieux, car il dépend
  d'un message du runtime ; s'il change, on écrit un rapport de trop
  (jamais un de moins, grâce au compteur). Testé.

Enseignement (§9 de la passation) : la capture s'est prouvée juste, mais
le **comportement de l'environnement** (double panic à la frontière FFI)
ne se voyait qu'au terrain — pas dans un test unitaire.

## Conséquences et limites assumées

- **Un panic très précoce** (avant `.setup`, où le hook s'installe) n'est
  pas capturé. Rare, et pré-consentement de toute façon.
- **Pas d'agrégation automatique** : c'est le prix du « fichier local ».
  Assumé pour la bêta ; un canal d'envoi pourra suivre si le terrain le
  réclame.
- **Le message supprimé** fait perdre des panics parfois lisibles
  (« index out of bounds »). Choix délibéré : la garantie de non-fuite
  prime, la localisation suffit dans l'écrasante majorité des cas.
- **`explorer` lancé en dur** pour ouvrir le dossier : dépendance
  Windows, cohérent avec la cible (pas de mobile, pas de web ici).
