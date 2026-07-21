# ADR 0005 — Le gate E2E vit hors de la CI hébergée : hook pré-push

Date : 2026-07-21 · Statut : accepté

## Contexte

Deux défauts livrés en annonçant « clippy et tests verts » ont échappé à
toute vérification automatique :

- le **port SMTP ignoré** pour un compte générique (`relay()` câblait un
  TLS implicite 465 en dur) — aucun test n'exerce un adaptateur réseau ;
  [depuis : `mail-smtp` en a deux, qui joignent un faux serveur sur un
  port éphémère — le trou était moins large qu'il n'y paraissait] ;
- le **menu d'ajout de compte affiché en permanence** (`#add-menu`,
  spécificité d'ID écrasant `[hidden]`) — invisible sans piloter l'UI.

Les deux étaient attrapables par la suite E2E existante
([`e2e/`](../../e2e/README.md)), qui pilote la vraie fenêtre via CDP. Le
seul manque était l'**obligation** de la jouer. D'où la contre-mesure
visée : un gate E2E automatique.

## L'hypothèse testée, puis tuée

Hypothèse initiale : faire tourner la suite E2E dans la CI GitHub
(`windows-latest`), en check requis. **Trois passages mesurés l'ont
réfutée.**

| Passage | Contre-mesure tentée | Résultat |
|---|---|---|
| 1 | Job E2E nu | `CDP inaccessible sur le port 9222 après 30 s`, sans le moindre indice (l'app était lancée avec `stdio: 'ignore'`) |
| 2 | Sortie de l'app capturée, attente portée à 90 s, attente de la **page** et non du port | `CDP injoignable après 90 s` · **`(l'application n'a rien écrit sur sa sortie)`** · processus **non mort** |
| 3 | Vérification + installation du runtime WebView2 | **`WebView2 present, version 150.0.4078.65`** · échec identique |

Faits établis, par élimination :

1. le binaire se construit et se lance — le processus **vit** 90 s ;
2. le runtime **WebView2 est présent** sur le runner ;
3. la fenêtre ne s'initialise **jamais**, sans erreur, sans code de
   sortie, sans un octet sur stdout/stderr ;
4. ce n'est ni la lenteur (90 s), ni le profil WebView2, ni une course
   entre le port CDP et la création de la page — les trois ont été
   corrigés et écartés.

Il reste la cause structurelle : **un runner GitHub hébergé n'offre pas la
session de bureau interactive dont WebView2 a besoin pour créer une
fenêtre.** Cela ne se corrige pas par configuration.

## Décision

**La suite E2E ne tourne pas dans la CI hébergée.** Elle est jouée par un
hook Git **`pre-push`** versionné dans [`.githooks/pre-push`](../../.githooks/pre-push),
activé par `git config core.hooksPath .githooks`.

Le hook exécute le gate complet, du plus rapide au plus lent : `cargo fmt
--check`, `cargo clippy -D warnings`, `cargo test --workspace`, puis les
10 parcours E2E. S'il passe, la CI est verte par construction.

La CI hébergée conserve ce qu'elle sait faire de façon fiable :
`quality` (fmt + clippy + tests) et `audit` (CVE).

### Pourquoi pas un job E2E rouge « informatif »

Un andon qui hurle en permanence n'est plus un andon : on cesse de le
regarder, et le jour où il signale un vrai défaut, personne ne l'entend.
Un job durablement rouge est pire que pas de job.

### Pourquoi pas (encore) un runner auto-hébergé

C'est la solution rigoureuse : le gate resterait **en CI et bloquant**,
non contournable. Elle est écartée pour l'instant au titre du
dimensionnement juste — sur un projet à un seul développeur, elle ajoute
un agent à installer et à maintenir, et impose que la machine soit
allumée, pour se prémunir d'un contournement (`--no-verify`) que ce même
développeur devrait s'infliger volontairement. **À reprendre dès qu'un
deuxième contributeur arrive** : c'est là que la faiblesse devient réelle.

## Conséquences

- Le hook est **versionné**, donc partagé : une nouvelle machine n'a
  qu'à lancer `git config core.hooksPath .githooks` (documenté dans
  [`e2e/README.md`](../../e2e/README.md)).
- [`.gitattributes`](../../.gitattributes) force les fins de ligne **LF**
  sur `.githooks/**` : en CRLF, le shebang casse (« bad interpreter »).
- **Faiblesse assumée et nommée** : `git push --no-verify` contourne le
  gate, et celui-ci ne protège que les machines qui l'ont activé.
- Les correctifs apportés au harnais pendant l'enquête sont acquis et
  utiles en local : sortie de l'application capturée et recrachée en cas
  d'échec, détection de la mort du processus, attente de la page plutôt
  que du port (une course réelle, que le démarrage à chaud masquait).
- **Bascule si le contexte change** : runner auto-hébergé (gate bloquant
  retrouvé) dès qu'un deuxième contributeur ou une release publique le
  justifie.
