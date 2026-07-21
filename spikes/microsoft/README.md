# Spike Phase 3 — Microsoft 365 : IMAP+OAuth2 contre Graph

Départager par des chiffres la décision que la Phase 0 avait explicitement
reportée ([PHASE0.md](../../docs/PHASE0.md) §3, [PLAN.md](../../docs/PLAN.md)
§2.4 : *« Accès Microsoft : IMAP+OAuth vs Graph API — fiabilité, quotas,
effort »*).

## Étape 1 — la voie IMAP+OAuth2 tient-elle ? (ce banc)

```powershell
$env:MICROSOFT_CLIENT_ID = "<application (client) ID>"
cargo run -- vous@exemple.com
```

**Aucun message n'est envoyé** : la connexion SMTP est ouverte et
authentifiée (`test_connection`), ce qui suffit à valider le scope.

Le banc répond à quatre questions, dans l'ordre où elles peuvent tuer
l'option :

| # | Question | Pourquoi elle est décisive |
|---|---|---|
| 1 | Le consentement accorde-t-il vraiment les scopes Outlook ? | Leçon Google de la Phase 0 : un jeton est délivré même consentement partiel — seule la liste **accordée** fait foi |
| 2 | L'authentification IMAP XOAUTH2 passe-t-elle ? | Sans elle, la voie est morte |
| 3 | **SMTP AUTH est-il ouvert sur ce compte ?** | Le risque nommé : Microsoft le ferme par défaut sur certains tenants. Sans lui, « jamais d'envoi perdu » n'a plus de support et l'envoi devrait passer par Graph |
| 4 | Quels dossiers spéciaux (RFC 6154) ? | Décide la sémantique d'archivage : `\Archive` (déplacer) ou `\All` (expurger) — cf. le correctif de perte de données `e37a105` |

### Le piège n°1 : les scopes

Ce ne sont **pas** les noms courts de Graph. La
[doc Microsoft](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth)
insiste : *« specify the full scopes, including Outlook resource URLs »*.

| Protocole | Scope |
|---|---|
| IMAP | `https://outlook.office.com/IMAP.AccessAsUser.All` |
| SMTP | `https://outlook.office.com/SMTP.Send` |
| Refresh | `offline_access` |

Serveurs : `outlook.office365.com:993` (TLS implicite) et
`smtp.office365.com:587` (**STARTTLS** — Office 365 n'écoute pas en 465).

## Mesures

> À remplir après le premier passage sur compte réel.

| Mesure | Valeur |
|---|---|
| Consentement OAuth2 | — |
| Refresh token reçu | — |
| Connexion IMAP XOAUTH2 | — |
| LIST (dossiers) | — |
| SELECT INBOX | — |
| FETCH 200 enveloppes | — |
| SMTP AUTH | — |
| `\Archive` / `\All` annoncés | — |

## Étape 2 — Graph, seulement si nécessaire

L'étape 2 (banc Graph équivalent) n'a de sens que si l'étape 1 échoue sur
un point rédhibitoire — typiquement SMTP AUTH fermé. Sinon, l'asymétrie
d'effort est écrasante : IMAP+OAuth2 réutilise le moteur de synchro, la
boîte d'envoi et ses règles d'or, les brouillons et le stockage, sans une
ligne de neuf ; Graph exigerait un adaptateur REST complet
(`MailServer` + `MailTransport`, pagination, delta, quotas).

**Règle de départage** (celle de l'[ADR 0004](../../docs/adr/0004-moteur-de-recherche-fts5.md)) :
l'alternative doit battre l'hypothèse *nettement* pour la déloger.
