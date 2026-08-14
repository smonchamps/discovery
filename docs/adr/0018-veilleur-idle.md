# ADR 0018 — Le veilleur IDLE (le temps réel, par compte)

**Date** : 2026-08-14 · **Statut** : accepté sur l'architecture,
**activation sous réserve de la gate spike** (mesures terrain :
p50 ≤ 5 s, p95 ≤ 30 s, reconnexion coupure + veille/reprise, trois
fournisseurs — PLAN-SYNCHRO §7 « E4 spike »). Le câblage production ne
part qu'une fois cette gate verte et les budgets re-mesurés.

## Contexte

Terrain des 2026-08-13/14 : E2a/E2c ont ramené le cycle de ~38 min à
~4 s, mais la latence d'arrivée d'un mail reste **la cadence de sondage
(5 min)** — le modèle est du sondage, pas de l'événementiel. La plainte
bêta n°1 (« pas de vrai temps réel ») vise cette latence. P0-bis a
déjà mis de l'événementiel sur l'état réseau ; IDLE (RFC 2177) met de
l'événementiel sur l'arrivée du courrier : le serveur pousse un `EXISTS`
dès qu'un message tombe dans INBOX, sans que le client redemande.

Le spike (`spikes/idle/`) a établi la faisabilité et livré un premier
constat d'intégration : **la poignée `idle()` de la crate pose son
propre timeout de lecture pendant la veille puis le remet à `None` en
sortant** — elle effacerait le garde-fou P0 (120 s) posé à la connexion.

## Décision

1. **Un veilleur par compte, thread dédié du shell, sur une connexion
   IMAP DÉDIÉE** — jamais celle du cycle. Deux raisons : la connexion du
   cycle est éphémère (ouverte/fermée par relève), un veilleur doit
   TENIR ; et la poignée `idle` malmène le timeout de la socket — l'isoler
   sur sa propre connexion protège le cycle de vie du timeout P0.

2. **`idle` est une capacité de l'ADAPTATEUR (`mail-imap`), pas du trait
   `MailServer`.** Le moteur « enveloppes d'abord » (`mail-core`) ne
   connaît pas IDLE : c'est une opération BLOQUANTE qui vit hors de son
   flux de commandes. Le shell orchestre les veilleurs ; le moteur reste
   pur. (Alternative écartée : `idle` au trait — elle imposerait le
   blocage à toutes les implémentations et fuiterait un détail de
   transport dans le noyau.)

3. **Cycle de vie du veilleur** :
   - **relance IDLE COURTE : ~3 min, pas 28** — 1ᵉʳ terrain du spike
     (2026-08-14) : le timeout de lecture de la veille est AUSSI le seul
     détecteur d'une connexion morte. Coupure Wi-Fi et veille Windows ne
     produisent AUCUNE erreur — la lecture bloque en silence jusqu'à
     l'échéance (le « hang » de Thunderbird sur changement d'IP, bug
     Mozilla 284152). À 28 min de relance, le veilleur serait aveugle
     28 min ; à 3 min, la mort se détecte en ≤ 3 min au DONE/re-IDLE,
     pour 2 commandes par cycle — coût nul, et très en deçà des 29 min
     de la RFC 2177 ;
   - **reconnexion à délai doublé** (2 s → 60 s, réarmé après 2 min de
     session stable), repris du spike ;
   - **jeton OAuth relu au trousseau à chaque reconnexion** — une session
     qui tombe après l'expiration repart seule ;
   - **timeout de lecture re-posé après chaque sortie de veille** (le
     défaut de la crate, §Contexte) — ou la socket resterait sans garde.

4. **Un `EXISTS` réveille la passe légère du compte concerné** (E3,
   `sync_inbox_light` ciblé sur ce compte) : elle relève INBOX si ça a
   bougé (E2a), compte le courrier (P1) et émet les bulles — **parité
   téléphone**. Puis un **événement Tauri** pousse l'UI à recharger liste
   et nav. Le veilleur ne touche JAMAIS la base lui-même : il ne fait que
   signaler, la passe légère fait le travail (un seul chemin de relève).
   **ET la passe légère part aussi à chaque (RE)CONNEXION du veilleur** —
   2ᵉ terrain (2026-08-14) : un mail arrivé PENDANT une coupure est déjà
   dans la boîte à la reconnexion, aucun `EXISTS` ne le signalera jamais ;
   sans la passe de reconnexion, il attendrait le cycle de 5 min.

5. **Interaction P0-bis** : hors ligne (`navigator.onLine` faux), les
   veilleurs sont arrêtés (une connexion IDLE morte ne sert à rien et
   paierait des reconnexions en boucle) ; au retour `online`, relancés.
   L'UI pilote start/stop via les événements réseau déjà câblés.

6. **Interaction recul (complément P0)** : un compte en recul après
   échecs répétés ne relance pas son veilleur avant la fin de son délai —
   même discipline anti-martèlement que le cycle et la passe légère.

7. **Le cycle complet (5 min) demeure** pour ce qu'IDLE ne couvre pas :
   dossiers, brouillons, différentiel des suppressions, drapeaux
   (CONDSTORE). IDLE ne veille QUE INBOX. La cadence du cycle complet
   sera re-discutée une fois IDLE actif (S-D4, ouverte).

## Conséquences

- **Parité téléphone sur l'arrivée** : latence visée p50 ≤ 5 s. La
  plainte bêta n°1 tombe pour de bon.
- **Budget re-mesuré, obligatoire** : un veilleur = une connexion
  persistante + un thread. RAM déjà à 184-187/200 Mo. Un budget cassé
  est un **andon** — repli possible : IDLE sur le seul compte au premier
  plan, ou veilleurs mis en pause au-delà d'un plafond.
- **Complexité assumée** : threads, connexions persistantes, cycle de
  vie réseau — c'est le point dur du produit (front-loading, PASSATION
  §2.2). D'où l'ordre : spike mesuré, PUIS câblage.
- Les timeouts socket de P0 restent le filet du cas « réseau là, serveur
  muet » ; P0-bis reste la détection rapide de la coupure franche. IDLE
  s'ajoute à ces deux filets, il ne les remplace pas.

## Ce que le spike doit confirmer avant le câblage

Protocole et gates au `spikes/idle/README.md` : latence p50/p95 sur
10 arrivées, tenue 60 min, reconnexion après coupure réseau et après
veille/reprise Windows, comportement à l'expiration du jeton OAuth — sur
Gmail, Microsoft et un IMAP générique. La ligne s'arrête si une gate
casse.

**1ᵉʳ terrain (2026-08-14, Gmail)** : **tenue ✅** (2 h 42, un EXISTS
encore servi au bout) ; **latence ⚠️ à re-mesurer** — les ~30 s
constatés se comptaient depuis l'ENVOI, qui inclut la livraison Gmail ;
la mesure juste est la parité avec la bulle du téléphone ;
**reconnexion ❌ non prouvée** — la relance à 28 min rendait coupure et
veille invisibles (d'où la décision 3 amendée : relance courte). Spike
corrigé le jour même, coupure/veille/OAuth à rejouer.

**2ᵉ terrain (2026-08-14, Gmail, relance 3 min)** :
- **reconnexion sur coupure ✅** — détection en 1 min 48 (≤ 3 min, la
  relance courte au travail), reprise 2 s après le retour du réseau,
  délai doublé visible (2→4→8→16 s, échecs OAuth dits pendant la panne) ;
- **reprise de veille ✅** — Windows avorte la socket au réveil (10053) :
  détection immédiate, reconnecté en 2 s ;
- **le jeton se relit bien au trousseau** à chaque reconnexion (échecs
  hors-ligne, succès au retour) ; l'expiration ~1 h reste à observer ;
- **latence ✅ — parité bulle ACQUISE : écart bulle téléphone ↔ EXISTS
  ≤ 3 s** (mesuré sur les 5 envois). Le tick de ~60 s observé (quatre
  EXISTS à :35 de la minute à 0,3 s près) vit dans le pipeline de
  LIVRAISON Gmail, en amont : il retarde le téléphone autant que
  l'IMAP — les deux sont notifiés dans la même seconde. IDLE n'a aucun
  handicap face au push propriétaire ; la gate produit « en même temps
  (< 30 s) » est tenue avec ~27 s de marge, passe légère comprise ;
- **découverte : le mail arrivé pendant la coupure n'émet jamais
  d'EXISTS** (déjà en boîte à la re-SELECT) — d'où la décision 4
  amendée : passe légère à chaque (re)connexion.
