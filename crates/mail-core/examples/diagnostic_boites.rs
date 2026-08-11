//! Diagnostic P2 (refonte UI) : les six dossiers canoniques du prototype
//! — réception, envoyés, brouillons, indésirables, archives, corbeille —
//! se retrouvent-ils dans les boîtes RÉELLES de chaque compte ?
//!
//! La nav de l'écran 02 n'affiche que ces six catégories : ce diagnostic
//! classe les dossiers du cache (`folders`) par motifs canoniques et
//! signale ce qui manque ou reste ambigu. « Envoyés » n'est pas deviné :
//! `accounts.sent_mailbox` fait foi (ADR 0009 §7).
//!
//! Règle de la maison : rien de personnel. Seuls les noms RECONNUS
//! canoniques s'affichent en clair ; les autres sont comptés et rendus
//! en forme seule (initiale + longueur). Les adresses sont masquées.
//!
//! ```powershell
//! cargo run -p mail-core --example diagnostic_boites --release -- <chemin.db>
//! ```

use rusqlite::Connection;

// Leçon du premier passage sur la vraie base : un simple `contains()`
// explosait — un compte Gmail portant une migration PST donnait 26
// candidats « archive » (`.../Archive/Sport`, etc.). La règle devient
// POSITIONNELLE : seul le DERNIER segment compte, et le dossier doit
// vivre à la racine ou sous le seul préfixe fournisseur (`[Gmail]/x`) —
// jamais en profondeur. À candidats multiples, le préfixe fournisseur
// l'emporte sur l'homonyme racine.
const CATEGORIES: &[(&str, &[&str])] = &[
    ("réception", &["inbox"]),
    ("brouillons", &["drafts", "brouillons"]),
    (
        "indésirables",
        &[
            "spam",
            "junk",
            "junk e-mail",
            "courrier ind\u{e9}sirable",
            "ind\u{e9}sirables",
        ],
    ),
    (
        "corbeille",
        &[
            "trash",
            "corbeille",
            "deleted",
            "deleted items",
            "\u{e9}l\u{e9}ments supprim\u{e9}s",
        ],
    ),
    (
        "archives",
        &["archive", "archives", "all mail", "tous les messages"],
    ),
];

/// Racine, ou exactement un niveau sous `[Gmail]` — rien de plus profond.
fn segments(display: &str) -> Option<(bool, String)> {
    let parts: Vec<&str> = display.split('/').collect();
    match parts.as_slice() {
        [seul] => Some((false, seul.to_lowercase())),
        [prefixe, feuille] if prefixe.eq_ignore_ascii_case("[Gmail]") => {
            Some((true, feuille.to_lowercase()))
        }
        _ => None,
    }
}

fn masque(nom: &str) -> String {
    let initiale = nom.chars().next().unwrap_or('?');
    format!("{initiale}···({} car.)", nom.chars().count())
}

/// Les chemins wire peuvent CONTENIR une adresse (dossiers migrés d'un
/// autre compte) : caviardée avant tout affichage — « les diagnostics ne
/// divulguent rien », identifiants compris.
fn sans_adresse(nom: &str) -> String {
    nom.split_whitespace()
        .map(|mot| {
            if mot.contains('@') {
                "\u{2039}adresse\u{203a}".to_string()
            } else {
                mot.split('/')
                    .map(|seg| {
                        if seg.contains('@') {
                            "\u{2039}adresse\u{203a}"
                        } else {
                            seg
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("/")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : diagnostic_boites <chemin.db>")?;
    let conn = Connection::open(&path)?;

    let comptes: Vec<(i64, Option<String>)> = conn
        .prepare("SELECT id, sent_mailbox FROM accounts ORDER BY id")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    println!("{} compte(s)\n", comptes.len());

    for (account_id, sent) in comptes {
        let dossiers: Vec<(String, String, bool)> = conn
            .prepare(
                "SELECT wire, display, selectable FROM folders
                 WHERE account_id = ?1 ORDER BY display",
            )?
            .query_map([account_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        println!(
            "compte #{account_id} — {} dossier(s) au cache",
            dossiers.len()
        );

        match &sent {
            Some(nom) => println!(
                "  envoyés      : {}  (accounts.sent_mailbox, autoritaire)",
                sans_adresse(nom)
            ),
            None => println!("  envoyés      : ABSENT — sent_mailbox non déclaré"),
        }

        let mut classes: Vec<&str> = Vec::new();
        for (categorie, motifs) in CATEGORIES {
            let trouves: Vec<&(String, String, bool)> = dossiers
                .iter()
                .filter(|(_, display, _)| {
                    segments(display).is_some_and(|(_, feuille)| motifs.contains(&feuille.as_str()))
                })
                .collect();
            // Priorité au préfixe fournisseur : `[Gmail]/Corbeille` bat
            // l'homonyme racine `Corbeille`.
            let retenu = trouves
                .iter()
                .find(|(_, display, _)| segments(display).is_some_and(|(gmail, _)| gmail))
                .or_else(|| trouves.first());
            match retenu {
                None => println!("  {categorie:<12} : AUCUN dossier reconnu"),
                Some((wire, display, selectable)) => {
                    for (w, _, _) in &trouves {
                        classes.push(w);
                    }
                    let doublons = trouves.len().saturating_sub(1);
                    println!(
                        "  {categorie:<12} : {} (wire {}{}{})",
                        sans_adresse(display),
                        sans_adresse(wire),
                        if *selectable {
                            ""
                        } else {
                            ", NON sélectionnable"
                        },
                        if doublons > 0 {
                            format!(", {doublons} homonyme(s) écarté(s)")
                        } else {
                            String::new()
                        }
                    );
                }
            }
        }

        let autres: Vec<&(String, String, bool)> = dossiers
            .iter()
            .filter(|(wire, _, _)| {
                !classes.contains(&wire.as_str()) && Some(wire.as_str()) != sent.as_deref()
            })
            .collect();
        println!(
            "  non classés  : {} — {}",
            autres.len(),
            autres
                .iter()
                .map(|(_, display, _)| masque(display))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
    }
    Ok(())
}
