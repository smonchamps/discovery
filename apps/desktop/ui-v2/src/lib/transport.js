// Port de transport UI <-> coeur (R0-S5) — module ES pour la v2.
// Meme contrat que apps/desktop/ui/transport.js : une seule operation,
// appel(commande, arguments) -> Promise. Succes = valeur JSON du coeur ;
// echec = rejet portant le message (string) du Result<T, String>, tel
// quel. Pas de canal d'evenements : la progression se lit par sondage.
//
// Impl EN-PROCESSUS (Tauri IPC). Hors Tauri : echec franc et nomme,
// jamais un silence — l'impl distante (POST /api/appel/<commande>)
// remplacera ce rejet sans changer l'application.

const invoke = globalThis.window?.__TAURI__?.core?.invoke;

export const appel = invoke
  ? (commande, args) => invoke(commande, args)
  : (commande) => Promise.reject(
      `transport indisponible : ${commande} (hors Tauri, impl distante non livree)`);

// Le selecteur de fichiers natif (plugin dialog), par le MEME canal
// invoke que le reste — pas d'API globale a injecter, une seule
// permission (dialog:allow-open). Rend une liste de chemins, vide si
// l'utilisateur annule.
//
// Couture e2e (PLAN-PIECES-JOINTES §7) : la boite de dialogue native
// n'est pas pilotable par Playwright — la suite depose ses chemins de
// fixtures dans `window.__e2ePieces` et le selecteur ne s'ouvre jamais ;
// tout le reste du chemin (attach_files → puces → envoi) est le vrai.
export const choisirFichiers = async () => {
  const injectes = globalThis.window?.__e2ePieces;
  if (injectes !== undefined) return Array.isArray(injectes) ? injectes : [];
  const choix = await appel('plugin:dialog|open', { options: { multiple: true } });
  if (!choix) return [];
  return Array.isArray(choix) ? choix : [choix];
};
