# Spike — socle UI v2 (départage ① / ② / ③)

**Question à trancher :** pour la face web de la Stratégie A (un seul front,
porté partout par Tauri 2 + navigateur), laquelle de ces trois familles
tient les budgets à l'échelle du terrain ?

- **①** Vanilla + Web Components (zéro framework)
- **②** Framework réactif compile-time (Svelte)
- **③** UI en Rust → WASM (Leptos/Dioxus)

**Statut : jetable.** Ce code valide une décision (futur ADR 0015) ; il sera
supprimé une fois la décision prise.

## Ce que le spike exerce — et ce qu'il ne prétend PAS exercer

Le seul « point dur » qui discrimine ces trois familles, c'est la **liste
virtualisée à 256 312 messages** rendue avec la **ligne riche du Système**
(expéditeur + objet 18 px tronqué + aperçu 13 px + puces) **plus** la
**bascule de thème à chaud** (14 jetons, nature ↔ nuit).

Hypothèse de départ à réfuter ou confirmer, PAS à supposer :

> Avec une virtualisation correcte (fenêtrage : seules ~20 lignes existent
> dans le DOM, quel que soit le total) et un thème piloté par variables CSS
> (bascule = un `restyle` navigateur, pas du JS de framework), le **coût de
> rendu** et le **coût de bascule** sont quasi identiques d'une famille à
> l'autre. Si c'est vrai, la décision se déplace vers le **poids expédié**
> (démarrage à froid + mobile) et l'ergonomie — pas la vitesse de rendu.

Le spike mesure donc, à l'identique dans Edge headless (Chromium = WebView2) :

| Mesure | Définition exacte | Budget de référence |
|---|---|---|
| `premierRendu` | ms pour peindre la première fenêtre de liste | proxy de « page < 100 ms » |
| `pageP50/P95/Max` | ms pour reconstruire la fenêtre à 300 profondeurs réparties sur les 256 k | **page < 100 ms** |
| `themeP50/P95` | ms pour basculer les 14 jetons + forcer le recalcul de style | ressenti < 16 ms |
| `tasJsMo` | `performance.memory.usedJSHeapSize` (flag `--enable-precise-memory-info`) | part du budget RAM < 200 Mo |
| `poids gz` | octets **gzippés** réellement expédiés (runtime + app) | proxy de démarrage < 1 s, surtout en webview mobile |

Ce qui n'est **pas** mesuré ici, et pourquoi : la RAM totale du procédé
(working set privé, 7 procédés — ADR 0002) ne se mesure que sur l'app
empaquetée ; le spike n'a pas de noyau. Les données sont **synthétiques et
déterministes** (`commun/donnees.js`) : on ne tient jamais 256 k lignes en
mémoire, on fabrique l'enveloppe d'un index à la demande — exactement comme
le noyau sert une page au fil du défilement (PAGE_SIZE 200 dans l'app
réelle). La police d'icônes Material Symbols est un coût **partagé et égal**
aux trois familles : omise ici pour ne pas biaiser (elle ne discrimine pas).

## Lancer

    cd spikes/ui-socle-v2
    npm run build        # construit ② (Vite/Svelte) ; ① n'a pas de build
    node mesure.mjs      # mesure ① et ② en Edge headless, imprime le tableau

Le rapport comparatif et le verdict sont dans `RAPPORT.md`.
