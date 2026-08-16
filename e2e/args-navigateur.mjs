// Arguments navigateur des lanceurs e2e et des bancs — UNE source :
// `additionalBrowserArgs` de apps/desktop/tauri.conf.json (revue
// 2026-08-16). Le loader WebView2 fait ÉCRASER la conf par la variable
// d'environnement WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS : tout process
// qui la pose doit donc REPRENDRE les arguments de production, sans
// quoi il teste un navigateur différent de celui livré (SmartScreen et
// OOUI actifs, barres classiques au lieu de l'overlay A44).
//
// Piège Chromium : un drapeau répété (deux `--enable-features=`) n'est
// pas fusionné — le DERNIER gagne. Un supplément qui veut ajouter une
// feature doit réécrire la liste complète, virgules à l'appui.
import { readFileSync } from 'node:fs';
import path from 'node:path';

export function argsNavigateur(root, port, supplement = '') {
  const conf = JSON.parse(
    readFileSync(path.join(root, 'apps', 'desktop', 'tauri.conf.json'), 'utf8'),
  );
  const prod = conf.app.windows[0].additionalBrowserArgs ?? '';
  return [prod, `--remote-debugging-port=${port}`, '--lang=fr', supplement]
    .filter(Boolean)
    .join(' ');
}
