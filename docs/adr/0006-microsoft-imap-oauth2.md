# ADR 0006 — Microsoft 365 : IMAP+OAuth2 confirmé, Graph en plan B chiffré

Date : 2026-07-21 · Statut : accepté

## Contexte

La Phase 0 avait explicitement **reporté** ce départage à la Phase 3
([PHASE0.md](../PHASE0.md) §3), la grille set-based du plan
([PLAN.md](../PLAN.md) §2.4) posant *Graph API* en hypothèse du Chef
Ingénieur, sur critères d'élimination : **fiabilité, quotas, effort**.

Deux faits devaient être établis avant de trancher — aucun ne pouvait
l'être par raisonnement :

1. **IMAP+OAuth2 est-il encore supporté ?** L'argument qui aurait imposé
   Graph était « IMAP est condamné ».
2. **SMTP AUTH est-il ouvert ?** Sans lui, la règle d'or « jamais d'envoi
   perdu » ([ADR 0003](0003-boite-envoi-smtp.md)) n'a plus de support côté
   Microsoft, et l'envoi devrait passer par Graph.

## Recherche : l'argument « IMAP condamné » est faux

Microsoft n'a jamais déprécié les *protocoles* — seulement **Basic Auth**
(achevé fin 2022 pour IMAP/POP/EAS). Ce qui bouge en 2026 ne concerne que
SMTP AUTH **en Basic Auth** : désactivé par défaut fin décembre 2026,
retrait final annoncé au 2ᵉ semestre 2027. Pour les nouveaux tenants,
**OAuth est explicitement la méthode retenue**.

Sources : [Deprecation of Basic authentication in Exchange Online](https://learn.microsoft.com/en-us/exchange/clients-and-mobile-in-exchange-online/deprecation-of-basic-authentication-exchange-online) ·
[Updated SMTP AUTH Basic Authentication Deprecation Timeline](https://techcommunity.microsoft.com/blog/exchange/updated-exchange-online-smtp-auth-basic-authentication-deprecation-timeline/4489835)

## Mesures — compte Outlook.com réel ([`spikes/microsoft`](../../spikes/microsoft/README.md))

| Mesure | Résultat |
|---|---|
| Scopes **accordés** | `IMAP.AccessAsUser.All` + `SMTP.Send` — pas de consentement partiel |
| Refresh token | reçu → reconnexion silencieuse possible |
| Connexion IMAP XOAUTH2 | 389–551 ms |
| LIST (41 dossiers) | 54–144 ms |
| **SMTP AUTH** (`smtp.office365.com:587` STARTTLS) | **OUVERT**, 0,8–1,2 s |

## Décision

**IMAP+OAuth2 est confirmé** pour Microsoft 365 / Outlook.com en v1 ;
**Graph reste le plan B**, documenté et chiffré.

La règle de départage est celle de l'[ADR 0004](0004-moteur-de-recherche-fts5.md) :
l'alternative doit battre l'hypothèse **nettement**. Ici, Graph ne le fait
pas — son seul avantage décisif (« IMAP est condamné ») est réfuté, et
l'asymétrie d'effort est écrasante :

| | IMAP+OAuth2 | Graph |
|---|---|---|
| Moteur de synchro, boîte d'envoi et ses règles d'or, brouillons, stockage | **réutilisés sans une ligne de neuf** | à réécrire contre REST |
| Adaptateurs | déjà paramétrés par hôte (`connect_xoauth2(host, port, …)`) | nouvel adaptateur `MailServer` + `MailTransport` |
| Reste à faire | endpoints/scopes par fournisseur, hôtes par compte | pagination, delta, quotas, modèle propre |

### Les deux pièges, gelés ici

1. **Les scopes sont ceux de la RESSOURCE Outlook**, pas les noms courts de
   Graph — `https://outlook.office.com/IMAP.AccessAsUser.All` et
   `https://outlook.office.com/SMTP.Send`, plus `offline_access`.
2. **SMTP est en 587 STARTTLS**, jamais en 465 implicite. ~~Le chemin
   XOAUTH2 de `mail-smtp` câble encore 465 en dur.~~ **Soldé** : les deux
   modes d'authentification passent désormais par un chemin unique
   (`transport_builder`), et deux tests hors-ligne prouvent que le port
   demandé est bien celui joint. La duplication qui avait laissé le
   correctif `fb11538` ne profiter qu'au mot de passe n'existe plus.

## Conséquence inattendue : l'archivage

Exchange annonce `\Drafts`, `\Junk`, `\Sent` et `\Trash`, mais **ni
`\Archive` ni `\All`** — alors que le dossier `Archive` existe et sert
(13 sous-dossiers sur le compte mesuré). Le garde-fou anti-destruction de
[`e37a105`](../../crates/mail-imap/src/convert.rs) aurait donc **refusé**
d'archiver sur tout compte Microsoft — comportement correct, mais
fonctionnalité indisponible.

D'où un **repli par nom** (`archive`, `archives`) après les attributs
annoncés : exception délibérée à la règle « jamais de nom en dur »,
**justifiée par la mesure et par elle seule**. Ordre de priorité gelé :
`\Archive` → `\All` → nom connu → refus.

## Conséquences

- La productionisation suit : généraliser la couche OAuth par fournisseur
  (`GmailAuth` est figé sur Google), sortir les hôtes des constantes de
  `commands.rs`, corriger le port SMTP XOAUTH2.
- **Risque nommé** : SMTP AUTH est ouvert sur le tenant mesuré, mais
  Microsoft le ferme par défaut sur certains tenants d'entreprise. Le cas
  se manifestera par un refus à la connexion, pas par une perte — la
  boîte d'envoi conserve le message. À traiter en bêta si le cas survient.
- **Dette repérée** : les noms de dossiers reviennent en **UTF-7 modifié**
  non décodé (`Actualit&AOk-`). Sans effet ici (comparaison ASCII), mais
  **bloquant pour la fonctionnalité « dossiers / déplacer »** du backlog
  Phase 3.
- **Bascule vers Graph** si l'un de ces trois signaux apparaît : SMTP AUTH
  massivement fermé chez les utilisateurs bêta, annonce de retrait d'IMAP
  OAuth, ou quotas IMAP rédhibitoires à l'échelle. Le banc reste dans
  `spikes/microsoft`, relançable.
