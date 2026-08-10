// Générateur déterministe d'enveloppes synthétiques. On ne tient JAMAIS
// 256 312 lignes en mémoire : on fabrique l'enveloppe d'un index à la
// demande, exactement comme le noyau sert une page au fil du défilement
// (PAGE_SIZE 200 dans l'app réelle). Déterministe pour que ①, ② et ③
// rendent EXACTEMENT le même corpus — sinon la comparaison ment.
export const TOTAL = 256312;

const EXP = ['Camille Rousseau', 'Yanis Belkacem', 'Léa Fontaine',
  'Service comptabilité', 'Paul Mérand', 'Nadia Cherif', 'Atelier Nord',
  'Hugo Meyer'];
const OBJ = ['Relecture du contrat Vantis', 'Planning de la semaine 33',
  'Compte rendu du 4 août', 'Facture de juillet', 'Proposition commerciale',
  'Notes de réunion', 'Devis atelier', 'Suivi de commande'];
const APR = ["J'ai repris les articles 4 et 7, il reste la clause de renouvellement à trancher.",
  'Deux créneaux se chevauchent mardi après-midi, je propose de décaler.',
  'Trois décisions actées, une question ouverte sur le calendrier.',
  'Merci de vérifier le montant avant le 15 du mois.'];

// mulberry32 — PRNG déterministe, une graine par index.
function prng(graine) {
  let a = graine | 0;
  return function () {
    a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function enveloppe(i) {
  const r = prng(i + 1);
  const h = 8 + Math.floor(r() * 10);
  const m = Math.floor(r() * 60);
  return {
    exp: EXP[i % EXP.length],
    heure: String(h).padStart(2, '0') + ':' + String(m).padStart(2, '0'),
    objet: OBJ[(i * 7) % OBJ.length] + (r() < 0.4 ? ' et ses annexes tarifaires détaillées' : ''),
    apercu: APR[(i * 3) % APR.length],
    messages: r() < 0.25 ? 2 + Math.floor(r() * 6) : 0,
    fichiers: r() < 0.35 ? 1 + Math.floor(r() * 3) : 0,
    nonlu: r() < 0.2,
  };
}
