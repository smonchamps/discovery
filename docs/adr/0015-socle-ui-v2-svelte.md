# ADR 0015 — Socle UI v2 : Svelte, front web unique, porté partout par Tauri 2

Date : 2026-08-10 · Statut : accepté — arbitré par le Chef Ingénieur sur
mesures (spike jetable, deux moteurs : Blink desktop **et** Android-classe).

## Contexte

Un handoff de design « Douceur », réalisé séparément du code, fixe la
direction visuelle de la refonte. Ce qui lie Discovery n'est PAS ses choix
techniques de prototype (un runtime maison, non repris) mais les **règles
de son document Système** : 14 jetons pilotant toute couleur (bascule de
thème à chaud, 9 thèmes), une **signature** unique (surface + filet accent
2 px à gauche + ombre chaude), **deux** rayons (10 surfaces / 6 contrôles),
**une** élévation, typographie graduée **par la taille** (graisse 600
réservée à l'affichage), icônes Material Symbols Rounded, troncature à une
ligne (hauteur constante).

Une exigence neuve cadre la décision : la v2 doit se porter **simplement et
efficacement vers web, Linux, macOS, iOS et Android** — *pas forcément la
même techno par plateforme*.

Contraintes gelées qui servent de couperet :
1. **`mail-core` est le seul cerveau** (ADR 0001) — l'UI affiche un état,
   émet des intentions. Le cœur compile déjà pour les 6 cibles.
2. **Budgets = gates** (PLAN §1) : démarrage < 1 s, ouverture < 50 ms,
   **page de liste < 100 ms**, RAM < 200 Mo, à **256 312** messages en
   liste virtualisée.
3. **HTML de mail rendu en bac à sable** (iframe + CSP, images distantes
   bloquées, jamais `innerHTML`) — invariant de sécurité, non négociable.
4. **Web = moteur côté serveur** (Phase 4) : garder un front web rend cette
   phase quasi gratuite.
5. **Gate E2E pilote la vraie webview par CDP** (ADR 0005).

v1 (Windows, Tauri 2 + JS vanilla, ADR 0002) **reste en place** ; cet ADR
décide le **socle de la refonte v2**, pas son calendrier.

## La grille set-based

Sept familles techniques, rangées en trois **stratégies de portage** — le
fait décisif étant : le cœur est déjà partagé, donc le seul coût réécrit N
fois est **le Système**.

- **A — un seul front web**, porté par Tauri 2 (Win/Linux/macOS/iOS/Android
  via les webviews système) + navigateur (web). Système écrit **1 fois**.
- **B — un seul front natif multiplateforme** (Flutter / Dioxus / Slint) :
  1 fois, mais HTML-mail via webview embarquée par plateforme, nouveau
  harnais de test, et (Flutter) une langue neuve + FFI.
- **C — cœur partagé + face native par plateforme** (web+desktop ;
  SwiftUI ; Compose) : Système écrit **2–3 fois**.

**Stratégie A retenue comme épine** : moindre regret (atteint les 6 cibles
avec le substrat déjà en place), et elle ne ferme aucune porte — on ajoute
une face native par plateforme (vers C) *plus tard et seulement si une
mesure l'exige*, sans toucher au cœur. B (Flutter) ne déloge A que si un
rendu natif mobile est **exigé dès la v2** : il ne l'est pas.

Restait à départager la **face web de A** : ① vanilla + Web Components,
② Svelte, ③ Rust → WASM (Leptos/Dioxus). Spike jetable
[`spikes/ui-socle-v2`](../../spikes/ui-socle-v2/RAPPORT.md), point dur
unique — **liste virtualisée à 256 312 messages** (ligne riche du Système)
+ **bascule de thème** — mesuré à l'identique dans deux moteurs.

## Mesures décisives

**Desktop** (Edge headless = Chromium ; p95, 300 fenêtres réparties sur la
profondeur, 60 bascules) :

| | page p95 | thème p50/p95 | tas JS | poids gz |
|---|--:|--:|--:|--:|
| **① vanilla** | 1,5 ms | 0,2 / 0,3 ms | ~1 Mo | **5,2 Ko** |
| **② Svelte** | 2,4 ms | 0,2 / 0,3 ms | ~7 Mo | **16,5 Ko** |
| **③ WASM** *(estimé †)* | ≈ ①② | ≈ ①② | +qq Mo | **~150–400 Ko** |

**Android-classe** (moteur Blink RÉEL d'Android System WebView, viewport
390×844 DPR 3, tactile, **CPU ×6 = entrée de gamme**) :

| | montage | page p95 | thème p95 | tas JS |
|---|--:|--:|--:|--:|
| **① vanilla** | 5,6 ms | **22,5 ms** | 5,3 ms | ~12 Mo |
| **② Svelte** | 20,1 ms | **29,0 ms** | 4,4 ms | ~27 Mo |

† **③ non buildé** (ni Rust, ni chaîne WASM dans l'environnement) : rendu et
bascule ≈ ①② par construction (émet du vrai DOM+CSS) ; poids en estimation
étayée sur les tailles publiées de Leptos/Dioxus. Jamais un chiffre inventé.

Trois enseignements, confirmés dans les deux moteurs :
1. **Le rendu est neutralisé** par le fenêtrage (≤ 20 lignes dans le DOM,
   quel que soit le total) et le thème piloté par variables CSS (un restyle
   navigateur, pas du JS de framework). Même à CPU ×6, la page p95 reste
   **22–29 ms — 3 à 4× sous le budget de 100 ms**.
2. **La décision ne se joue donc PAS sur la vitesse**, mais sur le **poids
   expédié**, la **mémoire de base** et l'**ergonomie de maintenance**. Sur
   ces axes : **① < ② < ③**.
3. La **taxe de framework** devient visible quand le CPU est lent (montage
   ② 20 ms vs ① 6 ms) mais ne sort ② d'aucun budget.

## Décision (arbitrée par le Chef Ingénieur)

1. **Socle UI v2 = ② Svelte 5**, dans la **Stratégie A** : un front web
   unique, porté par Tauri 2 (desktop + iOS + Android via les webviews
   système) et par le navigateur (web, moteur côté serveur — Phase 4). Le
   **Système est exprimé une seule fois.** Motif : les deux moteurs viables
   tiennent tous les budgets ; le surcoût de ② (16 Ko, ~7 Mo, montage 20 ms
   à ×6) est **trivial** devant < 1 s / < 200 Mo ; et l'avantage décisif à
   l'échelle de **toute** l'app est la **maintenabilité** — l'`app.js` v1
   fait déjà 1 638 lignes de DOM impératif, la douleur que ② retire.

2. **③ (Rust → WASM) écarté du socle.** Poids et mémoire les plus lourds,
   maturité (Tauri + Leptos + mobile) la plus risquée, pour **zéro gain de
   rendu** ; le mobile enfonce le clou (parse/compile WASM au démarrage à
   froid sur CPU lent, le pire cas). À rouvrir seulement si l'équipe décide
   stratégiquement du « Rust partout ».

3. **① (vanilla) est le repli documenté**, à même socle de règles (jetons
   CSS, fenêtrage, bascule triviale), si un cas pèse le « zéro build / zéro
   dépendance / poids minimal absolu » au-dessus de tout (cible mobile
   extrême).

4. **La frontière UI ↔ cœur est un port de transport** à deux
   implémentations : **en-processus** (Tauri IPC / FFI, desktop + mobile) et
   **distant** (HTTP/WS, web). Écrit une fois, il rend le socle portable et
   garde `mail-core` sans dépendance UI (ADR 0001 intact).

## Vigilances et garde-fous (mesurés/nommés, pas supposés)

- **iOS / WKWebView = validation terrain DUE.** Non testé ici : WebKit est
  un moteur DIFFÉRENT de Blink (profil DOM/CSS distinct), simulateur macOS
  uniquement. **La vraie inconnue restante** — à valider sur matériel Apple
  réel ou ferme d'appareils avant tout envoi iOS. Report assumé, comme le
  plan B de l'ADR 0004.
- **Le matériel mobile réel** (GPU/compositeur, pression RAM, thermique) et
  la **fluidité de défilement au compositeur** ne sont qu'approchés par le
  ralentissement CPU : à confirmer sur appareil.
- **Hauteur de ligne fixe** : le spike fige 104 px (la règle de troncature
  du Système veut une « hauteur constante »). Les puces à hauteur variable
  exigeraient une virtualisation *mesurée* — décision d'ingénierie **égale
  aux trois familles**, donc sans effet sur ce choix, à acter à
  l'implémentation.
- **Material Symbols vendorisées localement** (sous-ensemble des 34 glyphes
  du Système) : l'offline-first et la CSP interdisent le CDN Google Fonts au
  runtime.
- **Collision mobile ↔ ADR 0010.** La synchronisation intégrale (base vers
  ~13 Go) est intenable sur un téléphone → politique de synchro
  fenêtrée/à la demande. **Décision de cœur, pas d'UI** : à traiter par un
  ADR distinct avant l'envoi mobile.
- **Coffre et push par OS** : `keyring` est desktop ; iOS Keychain /
  Android Keystore à intégrer ; IDLE cède à APNs/FCM.
- **Layouts du Système = desktop, 3 colonnes.** Jetons / typo / signature
  sont portables ; les layouts non. Une déclinaison **compacte/tactile**
  (colonne unique, tiroir de nav, cibles ≥ 44 px, gestes) est du design
  **dû** avant tout envoi mobile.
- **Gate E2E** : la refonte renomme le DOM → figer des `data-testid`
  stables pour que refonte et tests bougent en lockstep ; un harnais de
  test **mobile** viendra (le CDP desktop actuel ne couvre pas iOS).

## Conséquences

- v1 (Windows, JS vanilla) **reste en place** jusqu'à la bascule ; la
  migration se fera écran par écran suivant un plan de phase — **jetons
  d'abord** (socle invisible, app v1 repeinte pour prouver le système de
  jetons de bout en bout), puis modale Réglages + persistance, puis layouts.
- Nouvelle dépendance de build (**Svelte + Vite**) circonscrite à l'app UI.
  `mail-core` reste sans dépendance UI (ADR 0001) — invariant intact.
- **Étend l'ADR 0002** (shell desktop Tauri) : Tauri **2**, cibles desktop
  **et** mobile via les webviews système ; le web reste servi par le même
  moteur côté serveur (Phase 4).
- Le spike [`spikes/ui-socle-v2`](../../spikes/ui-socle-v2/RAPPORT.md) a
  joué son rôle — éliminer ③, prouver ①/② sur deux moteurs. **Jetable** :
  supprimable une fois cet ADR lu.
