//! Banc du gate 3 : la recherche et l'ouverture d'un message tiennent-elles
//! leurs budgets à l'échelle ?
//!
//! | Budget | Cible |
//! |---|---|
//! | Recherche | < 100 ms |
//! | Ouverture d'un message | < 50 ms |
//!
//! Protocole de l'[ADR 0004] : top-50 avec classement, et **le nombre de
//! résultats affiché à côté de chaque durée**. Sans lui un chiffre de
//! recherche ne veut rien dire — le coût de FTS5 suit le nombre de
//! correspondances, puisque `ORDER BY rank` calcule BM25 sur toutes. Une
//! requête rapide sur un terme rare ne prouve rien.
//!
//! L'ADR nomme d'ailleurs le point de rupture : une requête qui matche
//! 69-90 % du corpus dépasse le budget à 200 000 messages. Le banc la
//! joue exprès, pour savoir où l'on se situe.
//!
//! Lecture seule.
//!
//! ```powershell
//! cargo run -p mail-core --example banc_recherche --release -- "<chemin.db>"
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

/// Ce que l'utilisateur tape, dans l'ordre où il le tape.
///
/// **Le dernier terme est TOUJOURS un préfixe** : `parse_query` construit
/// `"terme"*` — c'est la recherche à la frappe. La requête à préfixe
/// n'est donc pas un cas limite, c'est le chemin normal, et c'est le plus
/// cher. Mesurer un mot entier sans son étoile ne mesurerait rien de ce
/// que le produit exécute.
///
/// D'où une frappe progressive : chaque ligne est un état réel du champ
/// de recherche, à partir de trois caractères (le seuil de déclenchement).
const REQUETES: [(&str, &str); 6] = [
    ("terme rare (traîne)", "ref12345"),
    ("3 car. — le seuil", "fac"),
    ("5 car.", "factu"),
    ("mot entier", "facture"),
    ("deux termes", "facture réu"),
    ("mot très commun", "réunion"),
];

/// L'expression FTS que `search` construira — reproduite ici pour que le
/// nombre de correspondances corresponde à la durée mesurée.
///
/// Couplage assumé et nommé : si `parse_query` change sa règle, ce banc
/// ment. C'est le prix d'un compte juste sans ouvrir l'API du noyau.
fn expression_fts(saisie: &str) -> String {
    let termes: Vec<&str> = saisie.split_whitespace().collect();
    let dernier = termes.len().saturating_sub(1);
    termes
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == dernier {
                format!("\"{t}\"*")
            } else {
                format!("\"{t}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : banc_recherche <chemin.db>")?;
    println!("base : {path}\n");

    let conn = Connection::open(&path)?;
    let messages: i64 = conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?;
    let corps: i64 = conn.query_row("SELECT COUNT(*) FROM bodies", [], |row| row.get(0))?;
    println!("{messages} messages, {corps} corps stockés");
    drop(conn);

    let store = Store::open(std::path::Path::new(&path))?;

    println!("\n--- recherche (top 50, budget < 100 ms) ---");
    for (etiquette, requete) in REQUETES {
        // Un tour à blanc : on mesure le régime établi, pas le premier
        // défaut de cache — la recherche se fait à la frappe, donc à
        // chaud.
        let _ = store.search(requete, 50)?;
        let depart = Instant::now();
        let resultats = store.search(requete, 50)?;
        let duree = depart.elapsed().as_secs_f64() * 1000.0;
        let expression = expression_fts(requete);
        let total = correspondances(&path, &expression).unwrap_or(-1);
        let verdict = if duree > 100.0 {
            "  ✗ HORS BUDGET"
        } else {
            ""
        };
        // Le coût unitaire est le seul chiffre transférable : le corpus
        // de ce banc est synthétique, donc sa SÉLECTIVITÉ ne vaut rien,
        // mais « tant de µs par correspondance » se reporte sur une vraie
        // boîte dont on connaît la sélectivité.
        let unitaire = if total > 0 {
            duree * 1000.0 / total as f64
        } else {
            0.0
        };
        println!(
            "{etiquette:<22} « {requete:<12} » → {expression:<22} {duree:>7.2} ms — \
             {:>2} rendus sur {total} correspondance(s) ({unitaire:.2} µs/corr.){verdict}",
            resultats.len()
        );
    }

    println!("\n--- ouverture d'un message (budget < 50 ms) ---");
    ouvertures(&store, &path)?;

    Ok(())
}

/// Le nombre RÉEL de correspondances, que `search` masque en tronquant à
/// `limit`. C'est lui qui explique la durée : `ORDER BY rank` calcule
/// BM25 sur **toutes** les correspondances, pas seulement sur les 50
/// rendues (ADR 0004).
fn correspondances(path: &str, expression: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    let total = conn.query_row(
        "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH ?1",
        [expression],
        |row| row.get(0),
    )?;
    Ok(total)
}

/// Le corps est-il servi depuis le cache assez vite ? On prend des
/// messages qui EN ONT un : mesurer une absence ne mesure rien.
fn ouvertures(store: &Store, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT m.account_id, m.name, b.uid
         FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
         ORDER BY b.uid DESC LIMIT 5",
    )?;
    let cibles: Vec<(i64, String, u32)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    drop(conn);

    for (account_id, mailbox, uid) in cibles {
        let depart = Instant::now();
        let corps = store.body(account_id, &mailbox, uid)?;
        let duree = depart.elapsed().as_secs_f64() * 1000.0;
        let verdict = if duree > 50.0 {
            "  ✗ HORS BUDGET"
        } else {
            ""
        };
        println!(
            "compte {account_id} uid {uid:<6} : {duree:>6.2} ms — {} octets{verdict}",
            corps.map(|html| html.len()).unwrap_or(0)
        );
    }
    Ok(())
}
