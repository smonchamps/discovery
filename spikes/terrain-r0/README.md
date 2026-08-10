# Kit terrain R0 — S2 (ligne 104 px) et dû S1 (volet de lecture)

Les deux dernières vérifications de R0 appartiennent au Chef Ingénieur :
elles se jugent sur **son** contenu réel, pas sur des corps synthétiques.
Ce kit met les vrais messages sous les yeux, **sans rien écrire** dans la
base et **sans que rien ne quitte la machine**.

**Statut : jetable.** Supprimé une fois S2 et le dû S1 actés au plan.

## 0. Préalable — une vraie base sur cette machine

La machine est neuve : `%APPDATA%/dev.discovery.app/` est vide. Il faut
d'abord lancer l'application avec le compte réel et laisser la
synchronisation **et le rattrapage des corps** tourner (l'aperçu et le
volet en ont besoin) :

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files/LLVM/bin:$PATH" && export GOOGLE_CLIENT_ID="…" GOOGLE_CLIENT_SECRET="…" && cargo run -p discovery-desktop
```

(Compte Microsoft : `MICROSOFT_CLIENT_ID`. Autre base : variable
`DISCOVERY_DB_PATH` au lancement du script d'extraction.)

## 1. Extraire (lecture SEULE)

```bash
node spikes/terrain-r0/extraire.mjs
```

Produit `donnees.gen.js` — **données réelles, gitignoré, jamais commité** :
120 lignes (une par fil, comme la liste) et ~12 corps choisis pour couvrir
les cas qui fâchent (les plus riches, ceux à images distantes, les plus
simples, les plus récents).

## 2. Servir et juger

```bash
node spikes/terrain-r0/servir.mjs
```

### S2 — `ligne-104.html`

La ligne Douceur exacte du prototype (expéditeur + heure, objet 18 px,
aperçu 13 px, puces 32 px, troncature à une ligne partout), remplie de tes
messages. Deux modes **mesurés** :

- **fixe 104 px** — verdict automatique `GO — 0/N lignes débordent` ou
  `DÉFAUT — n/N` avec les fautives cerclées d'alerte ;
- **naturelle** — relevé de la hauteur réelle de chaque ligne et
  distribution (`x px × n`). Le calcul froid annonce ~98 px sans puces,
  ~138 px avec : **c'est le chiffre à trancher.**

Le curseur de largeur (320–560 px) éprouve la troncature des objets et
aperçus réels aux largeurs vraisemblables du volet.

**GO/NO-GO S2 :** si « fixe » tient (0 débordement, à toute largeur) → la
ligne fixe est actée avec sa hauteur. Si les puces débordent → soit une
hauteur fixe différente (le relevé la donne), soit puces reloties, soit
**virtualisation mesurée** (coût égal aux trois familles, ADR 0015 — sans
effet sur le socle).

### S1 (dû) — `volet-lecture.html`

Tes corps réels dans la **même frontière que la production** : iframe
`sandbox` sans jeton + CSP par message, base typographique étendue au
Système (sélecteurs simples, sans `!important`). À vérifier, surtout sur
une newsletter riche et un mail à images distantes :

1. la newsletter garde **sa** mise en page (la base ne surcharge rien) ;
2. le mail simple prend la Douceur (police système, 15 px, 1,65) ;
3. **aucune image distante ne se charge** (cadres vides/alt — c'est le
   comportement voulu, le pixel espion reste aveugle) ;
4. case « thème sombre » : le chrome bascule, le corps HTML **reste sur
   surface claire** — la décision S1.

Nota : l'assainissement `ammonia` n'est pas rejoué ici ; le bac à sable et
la CSP portent seuls la garde (suffisance prouvée par le cas 4 du spike
[`volet-lecture`](../volet-lecture/README.md)). Dans l'application, les
deux couches s'additionnent.

## 3. Acter

Reporter les deux verdicts dans
[`docs/PLAN-REFONTE-UI-V2.md`](../../docs/PLAN-REFONTE-UI-V2.md) (S2 et le
dû S1) — puis R0 est clos et R1 peut s'ouvrir.
