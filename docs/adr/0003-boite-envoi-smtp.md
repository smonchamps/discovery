# ADR 0003 — Boîte d'envoi SMTP : machine à états anti-fantôme

Date : 2026-07-17 · Statut : accepté

## Contexte

La Phase 2 ([PLAN.md](../PLAN.md) §4) exige « composer, répondre, transférer ;
envoi SMTP avec file "boîte d'envoi" (jamais d'envoi perdu) », et le concept
paper (§1) ajoute « jamais d'envoi fantôme ». Ces deux règles sont en tension :

- **Jamais perdu** pousse à réessayer agressivement tout envoi douteux.
- **Jamais fantôme** interdit de renvoyer un message dont on ignore s'il est
  parti — un crash entre la remise SMTP et l'accusé local crée exactement
  cette ambiguïté, et SMTP n'offre aucune remise idempotente.

Le doublon silencieux est pire que le retard : un retard se rattrape, un
doublon est déjà chez le destinataire.

## Décision

**1. Journal persistant avant tout réseau.** Chaque envoi est écrit dans la
table `outbox` (SQLite) *avant* toute connexion SMTP — même contrat que la
file d'actions de triage. Un crash ou une coupure ne perd jamais l'intention.

**2. Machine à états stricte, la fenêtre d'ambiguïté réduite à la remise :**

```text
queued ──→ sending ──→ sent
   ↑          │
   │          ├─ échec transitoire ──→ queued (réessai automatique)
   │          ├─ refus permanent ───→ rejected (décision utilisateur)
   │          └─ crash en vol ──────→ interrupted (quarantaine)
   └────────── requeue : décision explicite de l'utilisateur
```

Un message retrouvé en `sending` au début d'une vidange vient d'un crash :
il est **mis en quarantaine (`interrupted`), jamais renvoyé automatiquement**.
L'UI présente la décision (« Renvoyer » / « Abandonner ») avec l'avertissement
de vérifier le dossier Envoyés. Le Message-ID, généré par NOUS à la
composition et persisté dans le journal, rend l'envoi corrélable au message
réellement parti (une réconciliation automatique via IMAP est possible plus
tard sans changer le modèle).

**3. Classification transitoire/permanent déléguée à l'adaptateur.**
`mail-core` définit le port `MailTransport` (`SendError::Transient|Permanent`) ;
`mail-smtp` (lettre, XOAUTH2, smtp.gmail.com:465) classe : réponse 5xx à
l'envoi = refus du message (`rejected`, on continue la file) ; tout le reste
= transitoire (retour en `queued`, la pompe s'arrête — le réseau est tombé,
inutile d'insister). **L'authentification se joue à la connexion**
(`test_connection` + refresh token et seconde tentative côté hôte) : un token
expiré, erreur 5xx en SMTP, ne peut ainsi jamais être confondu avec un refus
permanent d'un message sain.

**4. Vidange sérialisée.** Une seule pompe à la fois (verrou côté desktop) :
deux vidanges concurrentes mettraient en quarantaine les envois « en vol »
l'une de l'autre.

## Périmètre v1 (refus explicites)

- **Texte brut seul** ; le HTML sortant viendra plus tard, s'il le faut.
- **Répondre = destinataire + Re: + In-Reply-To/References** (le References
  complet du fil exigerait de stocker les References de chaque message reçu).
- Pas de citation du message d'origine, pas de transfert, pas de brouillons
  synchronisés — le reste de la Phase 2 les couvrira.
- Gmail place lui-même les envois SMTP dans « Envoyés » : pas d'APPEND IMAP.
  Les autres fournisseurs l'exigeront (Phase 3, multi-comptes).

## Conséquences et vigilances

- Les enveloppes synchronisées avant cette version n'ont ni adresse brute ni
  Message-ID (colonnes ajoutées par migration, valeur NULL) : « Répondre »
  sur un vieux message demande une resynchronisation complète. Acceptable :
  on répond à du courrier récent.
- Les envois `sent` restent dans le journal (historique prouvable) ; une
  purge viendra si le volume le justifie.
- `EmailAddress` refuse désormais blancs, virgules, points-virgules et
  chevrons : ferme l'injection d'en-têtes et rend le stockage des listes de
  destinataires sûr par construction.
- Tests des deux règles d'or dans `mail-core::outbox` :
  `queued_send_survives_process_restart`,
  `network_cut_keeps_message_queued_then_next_flush_sends_it`,
  `inflight_message_is_quarantined_never_resent`,
  `user_requeue_is_the_only_way_out_of_quarantine`.
