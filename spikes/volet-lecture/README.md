# Spike S1 — la frontière du volet de lecture

**Question à trancher :** appliquer la typographie « Clarity » du Système au
volet de lecture **sans percer le bac à sable ni écraser les styles de
l'expéditeur**.

**Statut : jetable.** Valide une décision de front-loading (R0-S1) ; sera
supprimé une fois la décision actée. Ouvrir `index.html` dans un navigateur,
ou capturer via `../../scratchpad`-style CDP.

## Le modèle de sécurité — inchangé, non négociable

Le corps reste rendu par `mail-render::email_document` dans une iframe
`sandbox` (sans jeton = verrouillage total), via `srcdoc`, avec une **CSP par
message** `default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'`.
`ammonia` retire scripts et handlers ; les images distantes deviennent un
pixel neutre. **La Clarity ne touche JAMAIS le HTML de l'expéditeur.**
(Prouvé — cas 4 : script inerte, image distante bloquée ; + tests mail-render.)

## Décision — où la Clarity s'applique

1. **Le chrome** (carte, en-tête, expéditeur/méta, barre d'actions,
   signature) : plein Clarity, dans le DOM de l'app (CSS v2), **hors iframe**.
   Liberté totale.
2. **La base typographique de l'iframe** : on **étend** le `<style>body{…}` de
   `email_document` au Système — police système, 15 px, 1,65, encre — en
   **sélecteurs simples, sans `!important`**. Ce sont des défauts : le texte
   non stylé prend la Clarity, **tout style d'expéditeur l'emporte** (cas 3).
   **Gouttière : 20 px, pas 12** — verdict terrain (2026-08-10, vrais mails) :
   à 12 px le corps colle au bord de la carte ; il s'aligne sur la
   respiration du chrome (en-tête à 20 px).
3. **La colonne de lecture 68 ch** ne s'applique **qu'au texte brut converti**
   (`is_plain_text`), pas au HTML — sinon on briderait une mise en page
   (cas 2 vs cas 1).

## Nuance de thème — l'iframe est un document séparé

L'iframe **ne voit pas les variables CSS de l'hôte**. La couleur d'encre doit
donc être **bakée par thème** : `email_document(sanitized, policy,
is_plain_text, ink)`. Caveat : les mails HTML supposent un fond clair → **fond
clair neutre pour le HTML même en thème sombre** ; le **texte brut** peut
suivre le thème (fond sombre + encre claire). Décision : HTML sur surface
claire toujours ; texte brut suit le thème.

## Ce que le spike prouve (voir la capture)

| Cas | Attendu | Observé |
|---|---|---|
| 1 — texte brut | Clarity pleine (colonne 68 ch) | ✅ |
| 2 — HTML simple, non stylé | Clarity typo, **sans** bride de largeur | ✅ |
| 3 — HTML stylé par l'expéditeur | l'expéditeur garde la main | ✅ (Georgia, couleurs, 520 px préservés) |
| 4 — image distante + script injecté | inertes | ✅ (image bloquée, script non exécuté) |

## Implémentation (phase R ultérieure, pas en R0)

Étendre `email_document(sanitized, policy, is_plain_text, ink)` ; test Rust
assérant la base (police/taille/interlignage/encre présents, **aucun
`!important`**, colonne 68 ch seulement si `is_plain_text`). CSP **inchangée**
(`style-src 'unsafe-inline'` couvre déjà la base injectée).

## Dû (genchi genbutsu)

Le spike utilise des corps **synthétiques** représentatifs. Le **Chef
Ingénieur valide sur un VRAI mail** de son compte — surtout une newsletter
riche et un mail à images distantes — avant d'inscrire la décision comme
définitive.
