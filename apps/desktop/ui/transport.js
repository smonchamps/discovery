// Port de transport UI <-> coeur (R0-S5).
//
// TOUT le trafic entre l'interface et mail-core passe par ce port :
// une seule operation, appel(commande, arguments) -> Promise.
//   - succes : la valeur JSON renvoyee par le coeur ;
//   - echec  : rejet portant un message (string) — le Result<T, String>
//     des commandes Rust, tel quel.
//
// Pas de canal d'evenements : la progression se lit par SONDAGE
// (sync_progress, migration_progress, backfill_status...). C'est un choix,
// pas un manque : le sondage traverse un transport distant sans rien
// changer, la ou un canal pousse (WS/SSE) exigerait une seconde
// abstraction. On ne paie pas ce cout tant que le besoin n'existe pas.
//
// Implementations :
//   - EN-PROCESSUS (livree ici) : Tauri IPC — desktop et mobile
//     (strategie A, ADR 0015).
//   - DISTANTE (esquisse, pas due en R0) : POST /api/appel/<commande>,
//     arguments en JSON ; 200 -> resultat JSON, sinon le corps de la
//     reponse est le message d'erreur. Meme vocabulaire, meme contrat —
//     seul ce fichier change, pas l'application.
'use strict';

function creerTransport() {
  const invoke = window.__TAURI__?.core?.invoke;
  if (invoke) {
    return { appel: (commande, args) => invoke(commande, args) };
  }
  // Hors Tauri (navigateur nu, impl distante pas encore branchee) :
  // echec franc et nomme, jamais un silence.
  return {
    appel: (commande) => Promise.reject(
      `transport indisponible : ${commande} (hors Tauri, impl distante non livree)`),
  };
}
