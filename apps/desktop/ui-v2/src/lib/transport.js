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
