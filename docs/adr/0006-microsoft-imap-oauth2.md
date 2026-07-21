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
| Reste à faire | ~~endpoints/scopes par fournisseur, hôtes par compte~~ → **fait** ; reste l'UI d'ajout | pagination, delta, quotas, modèle propre |

### La couche d'authentification, généralisée

`GmailAuth` était un nom qui mentait : la classe portait Google en dur
dans ses constantes, et l'application ses serveurs dans les siennes.
Le parcours est désormais **unique** ; ce qui distingue un fournisseur
est décrit en **données** dans `mail-auth::provider` — endpoints, scopes,
règle de vérification du consentement, hôte de redirection, politique de
secret client, stratégie d'identité, serveurs IMAP/SMTP.

Trois choix méritent d'être gelés ici :

- **Les descripteurs sont testés contre le spike**, pas contre la doc.
  Les valeurs Microsoft qui figurent dans le code sont celles qu'un
  compte réel a effectivement acceptées.
- **Trois identifiants sont épinglés par des tests** : la clé du coffre
  (`gmail-refresh:`), la valeur en base (`accounts.provider = "gmail"`)
  et leur unicité entre fournisseurs. Aucun de ces renommages ne casse
  quoi que ce soit à la compilation ; tous déconnecteraient
  silencieusement les comptes existants.
- **Microsoft ne livre pas l'identité du compte** dans le périmètre de
  scopes mesuré : `Identity::Declared`, l'adresse est saisie. La piste
  `openid profile email` + `graph.microsoft.com/oidc/userinfo` est
  documentée dans le code mais **non mesurée** — donc non retenue.

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

## Validation terrain (2026-07-21, compte Microsoft réel)

Le parcours complet a été joué **depuis l'application**, pas depuis le
banc. Les cinq points passent :

| | Vérifié |
|---|---|
| Ajout du compte | consentement navigateur, adresse déclarée, pastille affichée |
| Synchro | les messages Outlook remontent dans la boîte unifiée |
| Rattrapage | le bandeau repart sur le nouveau compte |
| Reconnexion | **silencieuse** au relancement — le refresh token du coffre tient |
| Envoi | **un vrai message part en 587/STARTTLS** |

Le dernier point est le plus important, et il ne pouvait pas être obtenu
autrement. Le correctif du bug #3 n'était prouvé que **contre un faux
serveur** : les tests montrent quel port est joint, jamais qu'un message
en sort. C'est la validation terrain, et elle seule, qui ferme cette
boucle — exactement le rôle qu'on lui donne.

La reconnexion silencieuse valide au passage deux décisions prises sans
mesure directe : `offline_access` suffit bien à obtenir un refresh token
côté Microsoft (là où Google exige `access_type=offline` + `prompt=consent`),
et les préfixes de coffre disjoints font cohabiter les fournisseurs.

**Non levé** : `Identity::Declared`. L'adresse saisie a fonctionné, ce qui
ne dit rien de la piste OIDC — elle reste non mesurée, donc non retenue.

## Conséquences

- ~~La productionisation suit : généraliser la couche OAuth par fournisseur
  (`GmailAuth` est figé sur Google), sortir les hôtes des constantes de
  `commands.rs`, corriger le port SMTP XOAUTH2.~~ **Fait et validé sur le
  terrain.** Microsoft est un fournisseur de premier rang.
- **Risque nommé** : SMTP AUTH est ouvert sur le tenant mesuré, mais
  Microsoft le ferme par défaut sur certains tenants d'entreprise. Le cas
  se manifestera par un refus à la connexion, pas par une perte — la
  boîte d'envoi conserve le message. À traiter en bêta si le cas survient.
- ~~**Dette repérée** : les noms de dossiers reviennent en UTF-7 modifié
  non décodé (`Actualit&AOk-`).~~ **Soldée** : `mail-imap::mutf7` décode
  RFC 3501 §5.1.3, avec une règle explicite — on décode pour l'**œil** et
  pour les **comparaisons**, jamais pour le protocole. Le nom réseau reste
  celui qu'on renvoie au serveur ; les deux coexistent.

  Effet immédiat et non recherché : le repli par nom de l'archivage
  reconnaît désormais un dossier `Archiv&AOk-s`. Sur un serveur
  francophone sans attribut `\Archive` — exactement le cas Exchange qui a
  motivé ce repli — l'archivage était jusqu'ici indisponible.
- **Bascule vers Graph** si l'un de ces trois signaux apparaît : SMTP AUTH
  massivement fermé chez les utilisateurs bêta, annonce de retrait d'IMAP
  OAuth, ou quotas IMAP rédhibitoires à l'échelle. Le banc reste dans
  `spikes/microsoft`, relançable.
