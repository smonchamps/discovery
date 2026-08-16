// L'avatar aux initiales (UI v3, décision D2) : VISUEL seul — jamais
// un bouton, la sélection en lot est une feature différée. Deux
// lettres au plus, des deux premiers mots ; un nom vide (rare :
// brouillon sans destinataire) rend un tiret, jamais un blanc.
// Partagé liste/fil depuis le terrain A45 (cartes du volet).
export function initiales(nom) {
  const lettres = (nom ?? '').trim().split(/\s+/, 2)
    .map((mot) => mot[0])
    .join('')
    .toUpperCase();
  return lettres || '—';
}
