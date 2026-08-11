//! Diagnostic du coût d'OUVERTURE du Store — né du gate P1 de la
//! refonte : sur la vraie base (1,3 Go), `Store::open` coûtait ~500 ms
//! là où la base synthétique du banc n'en coûte que quelques-unes, et
//! CHAQUE commande Tauri ouvre sa connexion.
//!
//! Trois chronos séparent les étages :
//! 1. ouverture SQLite brute (+ PRAGMA wal + SELECT 1) — le coût
//!    fichier/OS, hors de notre code ;
//! 2. `Store::open` complet — y compris schéma et migrations ;
//! 3. un SECOND `Store::open` dans le même processus — ce qui reste,
//!    c'est le coût payé À CHAQUE commande, pas une chauffe.
//!
//! N'affiche que des **durées** : aucun sujet, aucun expéditeur, aucun
//! contenu de message n'est lu ni écrit.
//!
//! ```powershell
//! cargo run -p mail-core --example diagnostic_ouverture --release -- <chemin.db>
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : diagnostic_ouverture <chemin.db>")?;
    println!("base : {path}");

    let depart = Instant::now();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.query_row("PRAGMA journal_mode = wal", [], |row| {
        row.get::<_, String>(0)
    })?;
    let _un: i64 = conn.query_row("SELECT 1", [], |row| row.get(0))?;
    println!("ouverture brute (SQLite + WAL + SELECT 1) : {:?}", depart.elapsed());
    drop(conn);

    let depart = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!("Store::open — premier du processus       : {:?}", depart.elapsed());
    drop(store);

    let depart = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!("Store::open — second, même processus     : {:?}", depart.elapsed());
    drop(store);

    // ——— Le suspect : la recherche d'orphelins, rejouée à chaque
    // ouverture. Hors portée, `thread_id` reste NULL pour toujours
    // (ADR 0010 §3) : combien de lignes la requête énumère-t-elle pour
    // les écarter ensuite ?
    let conn = Connection::open(&path)?;
    let nulls: i64 = conn.query_row(
        "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let en_portee: i64 = conn.query_row(
        "SELECT COUNT(*) FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE m.threaded = 1",
        [],
        |row| row.get(0),
    )?;
    println!("\nenveloppes à thread_id NULL : {nulls} · en portée : {en_portee}");

    // Forme ACTUELLE de `orphans()` (projection réduite, même plan).
    let actuelle = "SELECT COUNT(*) FROM envelopes e
         JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE e.thread_id IS NULL AND m.threaded = 1";
    // Forme CANDIDATE : les boîtes en portée pilotent le balayage —
    // CROSS JOIN fige l'ordre de jointure, l'index (mailbox_id, uid)
    // porte le parcours ; les hors-portée ne sont jamais énumérées.
    let candidate = "SELECT COUNT(*) FROM mailboxes m
         CROSS JOIN envelopes e ON e.mailbox_id = m.id
         WHERE m.threaded = 1 AND e.thread_id IS NULL";
    for (nom, sql) in [("actuelle", actuelle), ("candidate", candidate)] {
        let depart = Instant::now();
        let n: i64 = conn.query_row(sql, [], |row| row.get(0))?;
        println!("forme {nom} : {n} orphelin(s) en {:?}", depart.elapsed());
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let plan: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(3))?
            .collect::<Result<_, _>>()?;
        for ligne in plan {
            println!("  {ligne}");
        }
    }

    Ok(())
}
