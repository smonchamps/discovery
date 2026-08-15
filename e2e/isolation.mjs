// Contrat d'isolation OAuth des bancs et suites e2e — la LISTE vit dans
// `isolation-oauth.json`, source unique partagée avec les outils Python
// (sonde-gel.py la lit telle quelle) : un fournisseur ajouté à un seul
// endroit couvre tous les lanceurs.
//
// Pourquoi purger : avec un client OAuth posé dans l'environnement, un
// test qui touche la route OAuth ouvrirait le VRAI consentement
// navigateur — et resterait suspendu dessus. Sans ces variables, aucun
// test ne peut toucher au vrai compte, même par accident.
import { readFileSync } from 'node:fs';
import path from 'node:path';

export const VARIABLES_OAUTH = JSON.parse(
  readFileSync(path.join(import.meta.dirname, 'isolation-oauth.json'), 'utf8'),
);

export function purgerOAuth(env) {
  for (const variable of VARIABLES_OAUTH) delete env[variable];
}
