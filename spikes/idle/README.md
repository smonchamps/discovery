# Spike E4 — la veille IDLE mesurée (PLAN-SYNCHRO)

Banc JETABLE, hors workspace. Il imprime des horodatages à la
milliseconde ; l'opérateur mesure ; l'ADR tranche. Rien d'ici ne part
en production.

## Gates chiffrées (PLAN-SYNCHRO §7, « E4 spike »)

- latence arrivée → événement : **p50 ≤ 5 s, p95 ≤ 30 s** ;
- **tenue de connexion 60 min** sans intervention ;
- **reconnexion automatique prouvée** : coupure réseau ET veille/reprise
  Windows ;
- comportement à l'**expiration du jeton OAuth** (le jeton est relu au
  trousseau à CHAQUE reconnexion — la session doit repartir seule) ;
- sur les **trois fournisseurs** : Gmail, Microsoft, IMAP générique.

## Lancer

Depuis ce dossier, avec l'environnement de dev habituel (les mêmes
variables OAuth que `cargo run` de l'application — le jeton vient du
trousseau que l'application a déjà rempli) :

```bash
# Gmail (compte déjà connecté dans Discovery)
SPIKE_FOURNISSEUR=gmail SPIKE_EMAIL=moi@gmail.com cargo run

# Microsoft
SPIKE_FOURNISSEUR=microsoft SPIKE_EMAIL=moi@outlook.com cargo run

# IMAP générique (mot de passe)
SPIKE_HOST=imap.exemple.fr SPIKE_PORT=993 SPIKE_USER=moi \
SPIKE_PASSWORD=... cargo run
```

Options : `SPIKE_BOITE` (défaut `INBOX`), `SPIKE_RELANCE_MIN` (défaut
**3** — la relance IDLE est AUSSI le délai max de détection d'une
connexion morte : 1ᵉʳ terrain du 2026-08-14, coupure Wi-Fi et veille
Windows ne produisent AUCUNE erreur, la lecture bloque en silence
jusqu'à cette échéance — à 28 min le spike était resté aveugle. Le
re-IDLE coûte 2 commandes par cycle : nul).

## Protocole (amendé au 1ᵉʳ terrain)

1. **Latence — mesurer contre la BULLE du téléphone, pas l'heure
   d'envoi** : l'envoi → `EXISTS` inclut la livraison Gmail elle-même
   (~30 s constatés), qu'IDLE ne compresse pas. La gate produit est la
   **parité téléphone** : noter l'heure de la bulle sur le téléphone et
   celle de la ligne `EXISTS` — p50/p95 sur cet écart-là. **✅ Acquise
   au 2ᵉ terrain : écart ≤ 3 s sur 5 envois — le tick de livraison
   Gmail (~60 s) retarde le téléphone autant que l'IMAP, IDLE n'a aucun
   handicap face au push propriétaire.**
2. **Tenue** : laisser tourner 60 min sans trafic. Chaque relance
   s'imprime (`relance de veille … connexion vivante`) : des relances
   régulières sans `reconnexion dans N s` = tenue prouvée.
   **✅ Acquise au 1ᵉʳ terrain : 2 h 42 (11:45 → 14:27), un EXISTS
   encore servi au bout.**
3. **Coupure** : couper le Wi-Fi 2 min en pleine veille ; attendu :
   `veille rompue …` en **≤ 3 min** (la relance), puis `reconnexion
   dans N s` (délai doublé 2 s → 60 s) et `connecté` au retour du
   réseau. **✅ Prouvée au 2ᵉ terrain : détection 1 min 48, reprise 2 s
   après le retour.**
4. **Veille/reprise** : fermer le capot 10 min ; au réveil, `veille
   rompue` puis `connecté`. **✅ Prouvée au 2ᵉ terrain : Windows avorte
   la socket au réveil (10053), reconnexion en 2 s.**
5. **OAuth** : laisser tourner au-delà de l'expiration du jeton
   (~60 min) puis provoquer une coupure : la reconnexion relit le
   trousseau — elle doit aboutir sans geste.

## Constats du 2ᵉ terrain (2026-08-14)

- **Gmail notifie sur un tick de ~60 s** : quatre EXISTS à :35 de la
  minute à 0,3 s près, envois répartis sur la minute. La latence
  envoi→EXISTS (p50 ≈ 37 s) est plafonnée par CE tick serveur — tout
  client IMAP subit le même. La gate latence se juge en parité bulle.
- **Un mail arrivé pendant une coupure n'émet jamais d'EXISTS** (déjà
  en boîte à la re-SELECT) : le veilleur de prod doit lancer la passe
  légère à chaque (re)connexion — ADR 0018, décision 4.

## Constats d'intégration (pour l'ADR, au fil du terrain)

- **Le timeout P0 et IDLE ne cohabitent pas tels quels** : la poignée
  `idle()` de la crate pose son PROPRE timeout de lecture pendant la
  veille, puis le **remet à `None` en sortant** — elle effacerait le
  garde-fou P0 (120 s) posé à la connexion. Le veilleur E4 devra
  re-poser le timeout du cycle après chaque veille, ou vivre sur une
  connexion dédiée (ce que fait ce spike). À trancher à l'ADR.
- Où vit `idle` (extension du trait `MailServer` ou capacité séparée) :
  tranché à l'ADR, sur ce que le spike apprend.
