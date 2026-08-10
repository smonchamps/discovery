# Rapport de spike — socle UI v2 (① / ② / ③)

**Décor :** Edge headless (Chromium = moteur de WebView2), corpus synthétique
déterministe de **256 312** enveloppes, ligne riche du Système (expéditeur +
objet 18 px tronqué + aperçu 13 px + puces), fenêtrage réel (~20 lignes dans
le DOM). Scénario identique pour toutes : premier rendu, **300** fenêtres
réparties sur toute la profondeur, **60** bascules de thème nature ↔ nuit,
tas JS (`--enable-precise-memory-info`), poids **gzippé** expédié. 3 tirs par
option, valeurs médianes ci-dessous.

## Mesures

| option | 1er rendu | page p50 | page p95 | page max | thème p50/p95 | tas JS | poids gz |
|---|--:|--:|--:|--:|--:|--:|--:|
| **① vanilla + Web Components** | 0,5 ms | 0,9 ms | 1,5 ms | ~8 ms | 0,2 / 0,3 ms | ~1,1 Mo | **5,2 Ko** |
| **② Svelte 5 (compile-time)** | 3,3 ms | 1,1 ms | 2,4 ms | ~8 ms | 0,2 / 0,3 ms | ~7,3 Mo | **16,5 Ko** |
| **③ Rust → WASM (Leptos/Dioxus)** | *≈ ①②* | *≈ ①②* | *≈ ①②* | *≈ ①②* | *≈ ①②* | +qq Mo | **~150–400 Ko** † |

† **③ n'a PAS été buildé** : ni Rust, ni cible `wasm32`, ni `trunk`/`wasm-pack`
dans l'environnement. Chiffres en *estimation étayée*, jamais mesurés ici :
- **Rendu et bascule ≈ ①②** par construction — Leptos/Dioxus émettent du **vrai
  DOM + CSS**, donc le fenêtrage et le thème piloté par variables CSS jouent à
  l'identique. Le framework ne touche pas ce que le navigateur fait du restyle.
- **Poids** : plancher d'un binaire WASM + glue pour une app liste réaliste,
  d'après les tailles publiées de Leptos/Dioxus — **ordre de 150 à 400 Ko gz**,
  soit **~10 à 40× ①** et **~9 à 24× ②**. Plancher mémoire WASM : quelques Mo.

## Mesure Android-classe (webview mobile, CPU ralenti)

Moteur **réel** d'Android System WebView (Blink/V8 = celui d'Edge/Chromium),
piloté par CDP : viewport **390 × 844**, DPR 3, tactile, UA Android, et
**ralentissement CPU ×1 / ×4 / ×6** (convention DevTools : ×4 ≈ milieu de
gamme, ×6 ≈ entrée de gamme, RELATIF à cette machine). Voir
[`mesure-mobile.mjs`](mesure-mobile.mjs).

| option | CPU | 1er rendu | page p50 | page p95 | thème p95 | tas JS |
|---|---|--:|--:|--:|--:|--:|
| **① vanilla** | ×1 | 0,6 ms | 1,4 ms | 3,0 ms | 0,4 ms | ~13 Mo |
| **① vanilla** | ×4 | 2,1 ms | 7,7 ms | **13,3 ms** | 3,3 ms | ~12 Mo |
| **① vanilla** | ×6 | 5,6 ms | 15,0 ms | **22,5 ms** | 5,3 ms | ~12 Mo |
| **② svelte** | ×1 | 3,4 ms | 1,6 ms | 2,5 ms | 0,4 ms | ~30 Mo |
| **② svelte** | ×4 | 15,1 ms | 13,4 ms | **19,9 ms** | 2,6 ms | ~62 Mo † |
| **② svelte** | ×6 | 20,1 ms | 17,5 ms | **29,0 ms** | 4,4 ms | ~27 Mo |

† bruit GC (le ×6 retombe à 27 Mo). Lire le `max` comme bruité aussi ; le
p95 est le signal stable.

Enseignements — tous **confirment et durcissent** le verdict desktop :

1. **La neutralisation TIENT sur CPU mobile.** Même à ×6, la page p95 reste
   **22–29 ms, soit 3–4× sous le budget de 100 ms** ; la bascule de thème
   ≤ 5,3 ms. Le rendu n'est pas le problème mobile.
2. **La taxe de framework devient visible quand le CPU est lent** — là où elle
   compte : montage ② **20 ms** vs ① **6 ms** (×6) ; page p95 **+30 %** ; tas
   **~2–4×**. Ça **favorise ①** sans disqualifier ② (tout tient sous budget).
3. **③ renforcé dans l'élimination** : parse/compile/instanciation WASM +
   poids 150–400 Ko frappent le plus fort au **démarrage à froid sur CPU
   lent** — le pire cas mobile.

## Ce que le spike établit (mesuré, reproductible)

1. **L'hypothèse de départ est CONFIRMÉE.** Avec fenêtrage + thème par variables
   CSS, les deux « points durs » — liste à 256 k et bascule de thème — sont
   **neutralisés** : le coût de rendu ne dépend PAS du framework.
   - Page : p95 de **1,5 ms (①)** vs **2,4 ms (②)** — les deux **~40–60× sous le
     budget de 100 ms**. L'écart entre les deux est du bruit à l'échelle du budget.
   - Bascule de thème : **0,2 ms partout** — c'est un restyle navigateur, pas du
     JS de framework. Le choix de framework n'y change **rien**.
   - Tas JS : 1,1 Mo (①) / 7,3 Mo (②) — **confirme qu'on ne tient jamais 256 k
     lignes** (fenêtrage réel). Négligeable devant le budget RAM de 200 Mo.
2. **La décision NE se joue donc PAS sur la vitesse de rendu.** Elle se déplace,
   comme prévu, vers **le poids expédié** (démarrage à froid, surtout en webview
   mobile d'entrée de gamme) et **l'ergonomie de maintenance** de toute l'app.

Sur le seul axe qui discrimine (poids + mémoire de base) : **① < ② < ③**.

## Élimination set-based

- **③ est écarté du socle v2.** Il paie la **taxe de poids et de mémoire la plus
  lourde** (et le risque de maturité le plus haut : intégration Tauri + Leptos +
  mobile encore niche) pour **zéro gain de rendu**. Son seul vrai atout — « une
  seule langue, Rust partout » — ne justifie pas ce coût maintenant. À rouvrir
  seulement si l'équipe décide stratégiquement du Rust-partout.
- **Le concours réel est ① vs ②.** Les deux écrasent tous les budgets. Le
  départage est un axe que **ce spike ne mesure pas** — la maintenabilité de
  *toute* l'app (pas d'un seul écran) :
  - **①** — le plus léger (5 Ko, ~1 Mo), **zéro dépendance, zéro build** :
    surface d'approvisionnement minimale, plus proche de l'app actuelle (déjà
    vanilla). Coût caché : on réécrit à la main réactivité, modales, état du
    composeur — l'`app.js` réel fait déjà **1 638 lignes** de DOM impératif.
  - **②** — +11 Ko et +6 Mo (tous deux **triviaux** devant < 1 s / < 200 Mo)
    pour une **réactivité idiomatique** : thème, dépli de fil, état de
    composition, modales. C'est précisément la douleur que ② retire à l'échelle
    de l'app entière.

## Recommandation du Chef Ingénieur

**② Svelte** comme socle, **si** l'équipe accepte une étape de build — parce
que le facteur décisif à l'échelle de toute l'app est la **maintenabilité**, et
que son surcoût mesuré est **négligeable** contre les budgets. **①** reste le
repli légitime si l'on pondère au-dessus de tout le « zéro build / zéro
dépendance / poids minimal absolu » (utile pour les cibles mobiles les plus
contraintes ou pour réduire la chaîne d'approvisionnement). **③ : non.**

C'est un arbitrage produit — il revient au Chef Ingénieur ; le spike a fait son
travail : il a **éliminé ③ sur mesure** et prouvé que ① et ② tiennent, ramenant
la question à un seul axe non-perf.

## Ce que le spike ne tranche PAS (honnêteté de mesure)

- **RAM totale du procédé empaqueté** (working set privé, 7 procédés — ADR 0002)
  ne se mesure que sur l'app Tauri réelle ; le spike n'a pas de noyau.
- **Android : moteur réel mesuré** (Blink/V8, CPU ×1/×4/×6, viewport téléphone)
  — voir section dédiée. Ce qui reste NON couvert par ce moteur : le **matériel
  réel** (GPU/compositeur, pression RAM, throttling thermique) que le
  ralentissement CPU ne fait qu'approcher, et la **fluidité de défilement au
  compositeur** (inertie tactile), qui exige un appareil réel.
- **iOS / WKWebView : NON testé, et non testable sur cette machine.** WebKit est
  un moteur DIFFÉRENT (profil DOM/CSS distinct de Blink), simulateur macOS
  uniquement. C'est **la vraie inconnue restante** : à valider sur matériel
  Apple réel ou une ferme d'appareils avant tout envoi iOS. Report assumé.
- **Hauteur de ligne fixe** : le spike fige la ligne à 104 px (la règle de
  troncature du Système veut une « hauteur constante », ce qui la favorise).
  Les puces à hauteur variable exigeraient une virtualisation *mesurée* — une
  décision d'ingénierie **égale aux trois familles**, donc sans effet sur le
  verdict, mais à acter dans l'ADR 0015.
- **Ergonomie / lignes de code** sur l'app entière : jugement, non mesuré ici.
