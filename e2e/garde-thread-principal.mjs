// Gate du thread principal (PLAN-GELS E1, décision D1) : dans Tauri 2,
// une commande déclarée SANS `async` s'exécute sur le thread principal —
// celui de la pompe de messages Windows. Toute commande synchrone qui
// ouvre la base, touche un fichier ou le keyring gèle donc la fenêtre
// pour toute sa durée (constat du 2026-08-15 : gels de 2 à 4,6 s au
// démarrage, 25,2 s cumulés sur 40 s — la fenêtre « ne répond pas »).
// Et `async` seul ne suffit pas : le corps bloquant passe par
// `hors_pompe` (spawn_blocking + verrou des commandes), sinon il épingle
// un worker tokio — le gel quitte la fenêtre pour la file IPC.
//
//   node garde-thread-principal.mjs   -> commandes fautives + verdict
//
// La règle est INVERSÉE à dessein : toute commande `#[tauri::command]`
// est `async fn`, SAUF les commandes pures d'état nommées ci-dessous.
// Une liste de marqueurs bloquants ne tiendrait pas : elle raterait la
// commande qui bloque au travers d'une aide (`queue_removal` ouvre la
// base pour `archive_message`). L'exemption, elle, se voit et se
// justifie.
//
// Deux gardes de l'instrument lui-même (le bogue « ink2 », payé deux
// fois — contraste.mjs:24, coherence-systeme.mjs:41) : la regex accepte
// les attributs paramétrés, `pub(crate)` et les chiffres ; et chaque
// occurrence textuelle de `#[tauri::command` doit correspondre à une
// prise — zéro prise ou un écart de compte est un ÉCHEC, jamais un vert
// silencieux.
//
// Le remède est toujours le même : passer la commande en `async fn` et
// son corps par `hors_pompe` — jamais allonger l'exemption sans la même
// preuve de pureté.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const sources = path.join(root, 'apps', 'desktop', 'src');

// Les exemptées, chacune avec sa raison d'être synchrone :
// - sync_activity : verrous TRÈS courts (trois Mutex de texte écrits une
//   fois par compte/boîte par le cycle) — pas des atomiques, mais des
//   fenêtres de quelques microsecondes ; le budget de la sonde
//   (`sonde-gel.py`, 150 ms) attraperait toute dérive ;
// - migration_progress : lecture d'atomiques partagés ;
// - migration_cancel : écriture d'un atomique ;
// - reseau_etat : un atomique, plus UN `sync_reculs.lock()` court
//   (clear d'une petite map au retour du réseau) — même garde-fou que
//   sync_activity : la sonde mesure, le budget tranche ;
// - app_version : lecture du manifeste en mémoire ;
// - open_link : ShellExecuteW DÉTACHÉ (open::that_detached) ;
// - telemetry_selftest_panic : ne bloque pas, elle PANIQUE — et
//   l'ADR 0014 a validé le double-panic du THREAD PRINCIPAL (frontière
//   FFI WebView2) : la déplacer changerait ce que l'auto-test exerce.
const PURES = new Set([
  'sync_activity',
  'migration_progress',
  'migration_cancel',
  'reseau_etat',
  'app_version',
  'open_link',
  'telemetry_selftest_panic',
]);

// Ce qui BLOQUE à coup sûr : base (chaque commande ouvre sa connexion),
// fichiers, coffre de l'OS. Si une exemptée s'en approche, elle perd
// l'exemption. (Détection best-effort — l'aide indirecte lui échappe,
// c'est pour cela que l'exemption est une LISTE et pas une heuristique.)
const MARQUEURS = ['Store::', 'db_path(', 'std::fs', 'File::', 'keyring', 'read_to_string'];

let echecs = 0;
const echec = (message) => {
  echecs += 1;
  console.log(`ECHEC ${message}`);
};

// Extrait le corps d'une fonction par équilibrage d'accolades, à partir
// de l'offset de sa première `{`. Heuristique assumée : une accolade
// non appariée dans une chaîne fausserait la borne — les corps exemptés
// sont courts et relus à chaque ajout à PURES.
function corps(texte, debut) {
  let profondeur = 0;
  for (let i = debut; i < texte.length; i += 1) {
    if (texte[i] === '{') profondeur += 1;
    else if (texte[i] === '}') {
      profondeur -= 1;
      if (profondeur === 0) return texte.slice(debut, i + 1);
    }
  }
  return texte.slice(debut);
}

let attributs = 0;
let prises = 0;
for (const fichier of readdirSync(sources).filter((f) => f.endsWith('.rs'))) {
  const texte = readFileSync(path.join(sources, fichier), 'utf8');
  attributs += (texte.match(/#\[tauri::command/g) ?? []).length;
  const commandes = texte.matchAll(
    /#\[tauri::command[^\]]*\]\s*(?:#\[[^\]]*\]\s*|\/\/[^\n]*\n\s*)*pub(?:\([^)]*\))?\s+(async\s+)?fn\s+([A-Za-z0-9_]+)/g,
  );
  for (const prise of commandes) {
    prises += 1;
    const [, estAsync, nom] = prise;
    if (estAsync) {
      if (PURES.has(nom)) {
        echec(
          `${fichier} : \`${nom}\` est async mais figure dans l'exemption des pures — retirer l'un ou l'autre`,
        );
      }
      continue;
    }
    if (!PURES.has(nom)) {
      echec(
        `${fichier} : la commande synchrone \`${nom}\` s'exécute sur le thread principal — la passer en \`async fn\` + \`hors_pompe\` (ou prouver sa pureté et l'exempter)`,
      );
      continue;
    }
    const ouvrante = texte.indexOf('{', prise.index + prise[0].length);
    if (ouvrante === -1) continue;
    const interieur = corps(texte, ouvrante);
    const trouves = MARQUEURS.filter((m) => interieur.includes(m));
    if (trouves.length > 0) {
      echec(
        `${fichier} : \`${nom}\` est exemptée comme pure mais touche ${trouves.join(', ')} — la passer en \`async fn\` + \`hors_pompe\``,
      );
    }
  }
}

// L'instrument se vérifie comme le reste (PASSATION §9) : chaque
// attribut doit avoir sa prise, et zéro commande = la gate ne regarde
// plus rien (déplacement de dossier, changement de forme).
if (prises === 0) {
  echec('aucune commande trouvée — la gate ne vérifie plus rien (dossier déplacé ? forme changée ?)');
} else if (prises !== attributs) {
  echec(
    `${attributs} attributs #[tauri::command] mais ${prises} prises — la regex rate des commandes`,
  );
}

if (echecs > 0) {
  console.log(`\n${echecs} défaut(s) sur le thread principal.`);
  process.exitCode = 1;
} else {
  console.log(
    `OK : ${prises} commandes vérifiées, aucune bloquante sur le thread principal.`,
  );
}
