// L'heure de la ligne, aux formes exactes du prototype : « 09:12 »
// aujourd'hui, « Hier », « 5 août » dans l'année, « 5 août 2024 »
// au-delà. Epoch 0 = date inconnue -> vide.

const MOIS = [
  'janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin',
  'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.',
];

// La forme longue du volet de lecture : « Aujourd'hui, 09:12 »,
// « Hier, 16:30 », « 5 août, 10:12 » — celle du prototype.
export function quandLong(epoch) {
  if (!epoch) return '';
  const date = new Date(epoch * 1000);
  const heure = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  const court = quand(epoch);
  if (court === heure) return `Aujourd'hui, ${heure}`;
  if (court === 'Hier') return `Hier, ${heure}`;
  return `${court}, ${heure}`;
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
  if (ecartJours === 1) return 'Hier';
  const quantieme = date.getDate() === 1 ? '1ᵉʳ' : String(date.getDate());
  if (date.getFullYear() === maintenant.getFullYear()) {
    return `${quantieme} ${MOIS[date.getMonth()]}`;
  }
  return `${quantieme} ${MOIS[date.getMonth()]} ${date.getFullYear()}`;
}
