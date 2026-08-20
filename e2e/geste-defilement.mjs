// Le geste du constat terrain (PLAN-DEFILEMENT-PROFOND) : la barre de
// défilement TENUE au clic — une rampe de scrollTop à ~60 événements
// par seconde. Partagé entre la spec (refonte-defilement.spec.js) et
// le banc (mesure-defilement.mjs) : le même geste des deux côtés, sans
// quoi les chiffres de l'un ne décriraient pas ce que l'autre garde.
export async function tenirBarre(page, { pas = 60, fraction = 1 / 3, intervalleMs = 16 } = {}) {
  await page.evaluate(
    async ({ pas, fraction, intervalleMs }) => {
      const cadre = document.querySelector('[data-testid="liste"] .cadre');
      const cible = cadre.scrollHeight * fraction;
      for (let k = 1; k <= pas; k++) {
        cadre.scrollTop = (cible * k) / pas;
        await new Promise((resolve) => setTimeout(resolve, intervalleMs));
      }
    },
    { pas, fraction, intervalleMs },
  );
}
