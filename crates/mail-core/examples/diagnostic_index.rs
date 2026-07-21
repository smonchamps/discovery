//! Diagnostic de l'index de recherche, compte par compte.
//!
//! Répond à une seule question : les messages d'un compte donné sont-ils
//! présents dans l'index FTS5 ? Un écart entre « enveloppes » et
//! « indexées » désigne le défaut.
//!
//! N'affiche que des **compteurs** : aucun sujet, aucun expéditeur, aucun
//! contenu de message n'est lu ni écrit.
//!
//! ```powershell
//! cargo run -p mail-core --example diagnostic_index -- "$env:APPDATA\dev.discovery.app\discovery.db"
//! ```

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : diagnostic_index <chemin.db>")?;
    let conn = Connection::open(&path)?;

    let indexed_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'search_docs'",
        [],
        |row| row.get(0),
    )?;
    println!("base : {path}");
    println!(
        "index de recherche : {}\n",
        if indexed_exists > 0 {
            "présent"
        } else {
            "ABSENT — jamais créé"
        }
    );
    if indexed_exists == 0 {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT a.id, a.email, a.provider,
                (SELECT COUNT(*) FROM envelopes e
                   JOIN mailboxes m ON m.id = e.mailbox_id
                  WHERE m.account_id = a.id),
                (SELECT COUNT(*) FROM search_docs d
                   JOIN mailboxes m ON m.id = d.mailbox_id
                  WHERE m.account_id = a.id),
                (SELECT COUNT(*) FROM bodies b
                   JOIN mailboxes m ON m.id = b.mailbox_id
                  WHERE m.account_id = a.id)
         FROM accounts a
         ORDER BY a.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    println!(
        "{:<3} {:<32} {:<8} {:>10} {:>10} {:>8}",
        "id", "compte", "type", "enveloppes", "indexées", "corps"
    );
    for row in rows {
        let (id, email, provider, envelopes, indexed, bodies) = row?;
        let alert = if indexed < envelopes {
            "  <-- ÉCART"
        } else {
            ""
        };
        println!(
            "{id:<3} {email:<32} {provider:<8} {envelopes:>10} {indexed:>10} {bodies:>8}{alert}"
        );
    }

    let orphans: i64 = conn.query_row(
        "SELECT COUNT(*) FROM search_docs d
          WHERE NOT EXISTS (SELECT 1 FROM envelopes e
                             WHERE e.mailbox_id = d.mailbox_id AND e.uid = d.uid)",
        [],
        |row| row.get(0),
    )?;
    println!("\nentrées d'index orphelines (message disparu) : {orphans}");

    // Les boîtes connues, pour reperer un compte dont l'INBOX porterait un
    // autre nom que celui interroge par la boite unifiee.
    let mut boxes = conn.prepare(
        "SELECT m.account_id, m.name, COUNT(e.uid)
           FROM mailboxes m
           LEFT JOIN envelopes e ON e.mailbox_id = m.id
          GROUP BY m.id ORDER BY m.account_id, m.name",
    )?;
    println!("\nboîtes synchronisées :");
    for row in boxes.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (account_id, name, count) = row?;
        println!("  compte {account_id} : « {name} » — {count} messages");
    }

    if let Some(term) = std::env::args().nth(2) {
        probe_term(&conn, &path, &term)?;
    } else {
        println!("\n(ajoutez un mot en 2e argument pour sonder la recherche)");
    }
    Ok(())
}

/// Sonde un terme à DEUX niveaux, pour isoler la couche fautive :
/// l'index FTS brut d'un côté, l'API `Store::search` de l'autre. Un écart
/// entre les deux désigne la requête, pas l'indexation.
///
/// N'affiche que des compteurs : aucun sujet n'est imprimé.
fn probe_term(conn: &Connection, path: &str, term: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== sonde du terme « {term} » ===");

    // Niveau 1 : l'index FTS seul, sans la moindre jointure de confort.
    let expression = format!("\"{}\"*", term.replace('"', ""));
    let mut raw = conn.prepare(
        "SELECT m.account_id, COUNT(*)
           FROM search_fts
           JOIN search_docs d ON d.docid = search_fts.rowid
           JOIN mailboxes m ON m.id = d.mailbox_id
          WHERE search_fts MATCH ?1
          GROUP BY m.account_id ORDER BY m.account_id",
    )?;
    println!("index FTS brut :");
    let mut any = false;
    for row in raw.query_map([&expression], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (account_id, count) = row?;
        println!("  compte {account_id} : {count} correspondance(s)");
        any = true;
    }
    if !any {
        println!("  aucune correspondance — le mot n'est pas dans les champs indexés");
    }

    // Niveau 2 : ce que le produit renvoie réellement.
    let store = mail_core::Store::open(std::path::Path::new(path))?;
    let rows = store.search(term, 50)?;
    println!("Store::search : {} résultat(s)", rows.len());
    let mut per_account: std::collections::BTreeMap<String, usize> = Default::default();
    for row in &rows {
        *per_account.entry(row.account_email.clone()).or_default() += 1;
    }
    for (email, count) in per_account {
        println!("  {email} : {count}");
    }
    Ok(())
}
