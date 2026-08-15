---
name: gate
description: Rejouer la gate complète de Wind (fmt, build ui-v2, contrastes, clippy, tests Rust, cohérence du Système, e2e) et rapporter les faits bruts. Obligatoire avant tout commit — jamais les tests seuls.
---

# /gate — la gate complète, du plus rapide au plus lent

L'ordre est celui du hook `.githooks/pre-push` (échouer tôt), plus la
vérification de cohérence du Système jouée en CI. Exécuter **tout**,
rapporter les faits bruts — chiffres, sorties d'échec — sans adoucir.

```
cargo fmt --all -- --check
(cd apps/desktop/ui-v2 && npm run build)        # zéro avertissement exigé
node e2e/contraste.mjs                          # paires WCAG (A8)
node e2e/coherence-systeme.mjs                  # Système ↔ systeme.css, valeur pour valeur
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets            # --all-targets n'est PAS décoratif (examples/)
cargo test --workspace --doc
(cd e2e && npm test)                            # la vraie fenêtre, CDP WebView2
```

## Règles

- **Un rouge = andon.** On arrête, on corrige, on rejoue la gate
  entière. Un warning clippy ou un avertissement du build ui-v2 est un
  rouge.
- **Après un sed/remplacement mécanique, toujours rejouer `fmt`** —
  la CI rouge du 2026-08-14 vient de là.
- **E2E rouge en local ≠ régression.** La suite flake sur cette machine
  (profil WebView2, OneDrive, charge). Rejouer le spec en isolation ;
  si le doute persiste, `gh run list` — **la CI est la référence**.
  Le flake connu : le brouillon fantôme (documenté au commit 0956c85).
- **La toolchain du gate doit être celle de la CI**
  (`rust-toolchain.toml` + ref épinglée dans `ci.yml`) — PASSATION §7.4.
- Le verdict final est la liste : chaque étape, verte ou rouge, avec
  ses chiffres (nombre de tests, avertissements, paires, valeurs).
