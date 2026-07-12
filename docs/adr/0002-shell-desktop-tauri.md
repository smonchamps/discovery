# ADR 0002 — Shell desktop : Tauri 2 validé au gate squelette

Date : 2026-07-12 · Statut : accepté (à re-mesurer au gate 1)

## Contexte

Le plan ([PLAN.md](../PLAN.md) §2.4) posait Tauri 2 en hypothèse de départ
pour le shell Windows, contre Slint/egui (natif) et Electron. La revue de
clôture de Phase 0 ([PHASE0.md](../PHASE0.md) §3) exigeait de la valider en
tout premier en Phase 1 : c'est l'hypothèse la plus structurante non spikée.

## Mesures (build release, squelette : fenêtre + frontend statique + noyau relié)

| Métrique | Mesure | Budget (PLAN.md §1) | Verdict |
|---|---|---|---|
| Démarrage → fenêtre utilisable* | **613 ms** | < 1 s | ✅ |
| Mémoire privée totale | **164 Mo** (5,7 app + 158,6 WebView2) | < 200 Mo | ✅ marge 36 Mo |
| Taille de l'exécutable | **8,15 Mo** | installeur < 15 Mo | ✅ trajectoire tenue |

\* mesuré du début de `main()` au premier `invoke` du frontend (DOM prêt).

Méthodologie mémoire : somme des **octets privés** (`PrivateMemorySize64`)
du processus principal et des 6 processus WebView2 enfants. La somme des
*working sets* (329 Mo) surestime en comptant plusieurs fois les pages
partagées entre processus — c'est la mesure privée qui fait foi.

## Décision

**Tauri 2 est confirmé** comme shell desktop. Le coût mémoire est presque
entièrement le forfait fixe WebView2 (~159 Mo) ; notre code y ajoute ~6 Mo.
En contrepartie : un exécutable de 8 Mo (Electron : ~80-150 Mo), le runtime
WebView2 déjà présent sur Windows 11, et l'UI web réutilisable en Phase 4.

## Conséquences et vigilances

- **La marge RAM n'est que de 36 Mo pour une fenêtre vide.** Le gate 1
  (liste virtualisée, 50 000 messages) re-mesure obligatoirement ; si le
  budget casse, le plan B documenté reste Slint/egui — d'où l'importance
  de garder l'UI « bête » et le domaine dans `mail-core`.
- La CSP du shell est `default-src 'self'` dès le squelette : aucun script
  ni style inline, même pour nous.
- L'icône est un placeholder généré (32×32) ; une vraie identité visuelle
  viendra avec la Phase 5.
