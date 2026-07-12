# Spike Phase 0 — Rendu HTML sécurisé

**Question à trancher :** peut-on afficher les emails HTML du monde réel de
façon sûre (XSS neutralisé, aucun chargement distant) sans casser la mise en
page des newsletters ?

**Statut : jetable.** Ce code valide des décisions ; il sera supprimé une fois
la Phase 0 conclue.

## L'architecture testée : défense en profondeur à trois couches

1. **Assainissement `ammonia`** — scripts, handlers d'événements, `javascript:`,
   iframes et formulaires retirés ; tableaux, styles inline et attributs de
   mise en page conservés.
2. **Vie privée** — chaque image distante est remplacée par un pixel neutre
   (pas de pixel espion, pas de fuite d'IP) ; les `url()` CSS sont filtrés.
3. **Isolation** — le message s'affiche dans une iframe `sandbox` (aucune
   permission) dont le document embarque une CSP `default-src 'none'` :
   même un contournement des couches 1-2 ne peut ni exécuter ni exfiltrer.

Le principe est vérifié par 10 tests unitaires, dont un qui **documente la
limite assumée** : un échappement CSS (`\75rl(`) traverse le filtre naïf —
c'est exactement pourquoi la couche 3 n'est pas optionnelle.

## Lancer

```powershell
# Mode hors-ligne (corpus embarqué : newsletter, attaque XSS, texte simple)
cargo run -p spike-html-render -- --sample

# Mode réel : les 20 derniers messages de votre Gmail
# (mêmes prérequis que le spike sync-engine)
$env:GOOGLE_CLIENT_ID = "…"
$env:GOOGLE_CLIENT_SECRET = "…"
cargo run -p spike-html-render
```

Puis ouvrez `target/spike-html/index.html` dans un navigateur et jugez la
fidélité de chaque message par rapport à Gmail.

## Verdict sécurité (validé le 2026-07-11, corpus embarqué + inspection navigateur)

Sur le message d'attaque : script voleur de cookies **supprimé avec son
contenu**, `onerror` **supprimé**, lien `javascript:` et lien `data:`
**désarmés**, iframe et formulaire de phishing **supprimés**, image distante
**remplacée par le pixel neutre**. Seul résidu : une chaîne CSS invalide et
inerte (le vecteur d'échappement documenté), neutralisée par la CSP.

Sur la newsletter : tableaux, largeurs et alignements **conservés** ;
`background-image: url(…)` distant supprimé mais `background-color`
**conservé** (dégradation élégante) ; pixel espion bloqué ; lien légitime
conservé ; accents corrects. Assainissement : max 1,6 ms par message —
compatible avec le budget « ouverture < 50 ms ».

## Verdict fidélité (validé le 2026-07-12, 20 derniers messages réels)

**0 parfait / 20 dégradés / 0 cassé.** Tout est lisible et sûr ; rien n'est
fidèle. Les deux causes sont identifiées et comprises :

1. **Blocs `<style>` supprimés** par ammonia — la plupart des newsletters y
   rangent leur habillage. C'est LE chantier fidélité de la Phase 1 : parser
   les blocs CSS (`lightningcss`), en retirer les chargements distants et les
   réinjecter scopés au message (l'approche de Gmail).
2. **Images distantes bloquées par défaut** — dégradation *voulue* (vie
   privée), pas un défaut : la Phase 1 ajoutera « Afficher les images » par
   expéditeur, avec cache local.

**Conclusion du spike : la sécurité est acquise par construction (3 couches) ;
la fidélité est un travail de CSS, pas d'architecture — aucun obstacle
structurel découvert.**

## Enseignements pour la Phase 1

1. **La CSP n'est pas une option** : le filtrage CSS textuel est contournable
   par construction ; la production ajoutera un vrai parseur CSS
   (`lightningcss`) en couche 2, la CSP restant le filet.
2. **Les blocs `<style>` sont supprimés par ammonia** : les newsletters qui en
   dépendent perdront leur habillage. À traiter en Phase 1 (parser et
   réécrire le CSS des blocs, comme le fait Gmail).
3. `mail-parser` convertit lui-même les messages texte en HTML sûr — un cas
   de moins à gérer.
4. Les images `cid:` (pièces jointes inline) sont autorisées mais non
   résolues ici — la Phase 1 les servira depuis le cache local.
