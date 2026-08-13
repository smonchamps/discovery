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
28 — relance IDLE avant l'échéance des 29 min, RFC 2177 ; descendre à
9 si un fournisseur coupe avant).

## Protocole

1. **Latence** : envoyer 10 messages depuis le téléphone en notant
   l'heure d'envoi de chacun ; la ligne `EXISTS n — nouveau courrier
   signalé` donne l'heure d'arrivée ; p50/p95 sur les 10 écarts.
2. **Tenue** : laisser tourner 60 min sans trafic ; compter les
   `reconnexion dans N s` (zéro attendu ; sinon, noter la période).
3. **Coupure** : couper le Wi-Fi 2 min en pleine veille ; la session
   doit tomber en erreur PUIS repartir seule (délai doublé 2 s → 60 s,
   réarmé après 2 min de session stable).
4. **Veille/reprise** : fermer le capot 10 min ; au réveil, noter le
   délai avant `connecté`.
5. **OAuth** : laisser tourner au-delà de l'expiration du jeton
   (~60 min) puis provoquer une coupure : la reconnexion relit le
   trousseau — elle doit aboutir sans geste.

## Constats d'intégration (pour l'ADR, au fil du terrain)

- **Le timeout P0 et IDLE ne cohabitent pas tels quels** : la poignée
  `idle()` de la crate pose son PROPRE timeout de lecture pendant la
  veille, puis le **remet à `None` en sortant** — elle effacerait le
  garde-fou P0 (120 s) posé à la connexion. Le veilleur E4 devra
  re-poser le timeout du cycle après chaque veille, ou vivre sur une
  connexion dédiée (ce que fait ce spike). À trancher à l'ADR.
- Où vit `idle` (extension du trait `MailServer` ou capacité séparée) :
  tranché à l'ADR, sur ce que le spike apprend.
