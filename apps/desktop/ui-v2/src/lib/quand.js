// L'heure de la ligne, aux formes exactes du prototype — désormais par
// langue (PLAN-LANGUES, A15) : mois, jours et gabarits viennent du
// catalogue, la grammaire ne bouge pas. En français : « 09:12 »
// aujourd'hui, « Hier », « Lundi » de 2 à 6 jours (dette D-3, soldée à
// E2 des Réglages), « 5 août » dans l'année, « 5 août 2024 » au-delà.
// En anglais, la transposition d'A15 : "Yesterday", "Monday", "Aug 5",
// "Aug 5, 2024" — l'heure reste sur 24 h dans les deux langues.
// Epoch 0 = date inconnue -> vide.
import { t } from './texte.svelte.js';

// « il y a 2 minutes » — la forme exacte du prototype pour la barre
// d'état (PLAN-SYNCHRO E1). `maintenant` (ms) vient de l'appelant : un
// `$state` re-cadencé toutes les 30 s, pour que « il y a N minutes »
// vieillisse à l'écran sans que personne ne clique.
export function depuis(epoch, maintenant) {
  const ecart = Math.max(0, Math.floor(maintenant / 1000) - epoch);
  if (ecart < 60) return t('depuis.instant');
  const minutes = Math.floor(ecart / 60);
  if (minutes < 60) return t('depuis.minutes', { n: minutes });
  const heures = Math.floor(minutes / 60);
  if (heures < 24) return t('depuis.heures', { n: heures });
  return t('depuis.jours', { n: Math.floor(heures / 24) });
}

// La forme longue des cartes du fil : « Aujourd'hui, 09:12 »,
// « Hier, 16:30 », « Lundi, 18:20 », « 5 août, 10:12 » — celle de la
// maquette Classique (terrain A45 ; la forme d'avant v3, ressuscitée
// par la langue : jour et gabarit viennent du catalogue).
export function quandLong(epoch) {
  if (!epoch) return '';
  const date = new Date(epoch * 1000);
  const heure = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  const court = quand(epoch);
  const jour = court === heure ? t('quand.aujourdhui') : court;
  return t('quand.long', { jour, heure });
}

export function quand(epoch) {
  if (!epoch) return '';
  const date = new Date(epoch * 1000);
  const maintenant = new Date();
  const jour = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const ecartJours = Math.round((jour(maintenant) - jour(date)) / 86400000);
  if (ecartJours === 0) {
    return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  }
  if (ecartJours === 1) return t('quand.hier');
  // La semaine glissante du prototype : « Lundi, 18:20 ».
  if (ecartJours >= 2 && ecartJours <= 6) return t('quand.jours')[date.getDay()];
  const quantieme = date.getDate() === 1 ? t('quand.premier') : String(date.getDate());
  const mois = t('quand.mois')[date.getMonth()];
  if (date.getFullYear() === maintenant.getFullYear()) {
    return t('quand.dansAnnee', { jour: quantieme, mois });
  }
  return t('quand.auDela', { jour: quantieme, mois, annee: date.getFullYear() });
}
