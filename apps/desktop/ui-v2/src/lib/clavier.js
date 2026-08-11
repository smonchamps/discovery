// A8 — le clavier active ce que le clic active : Entrée et Espace, sur
// tout élément qui porte role="button" sans être un <button> (les
// rangées et puces dont la géométrie du prototype interdit l'élément
// natif). L'anneau de focus vit dans systeme.css (:focus-visible).
export const activation = (faire) => (event) => {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    faire();
  }
};
