//! Banc du gate 3 : que coûte l'adoption des messages hérités ?
//!
//! [`mail_core`] rattache à un fil, **à l'ouverture de la base**, tous les
//! messages qui n'en ont pas encore. C'était instantané sur les 2 800
//! messages de la boîte réelle. Le gate 3 en demande 200 000, et cette
//! adoption se paie intégralement sur le budget de **démarrage** (< 1 s,
//! [`PLAN.md`] §1) : c'est le risque nommé au §8 de la passation.
//!
//! Deux régimes se mesurent séparément, car ils ne coûtent pas la même
//! chose et ne surviennent pas au même moment :
//!
//! | Régime | Quand | Ce qui se passe |
//! |---|---|---|
//! | **adoption** | base héritée, jamais regroupée | chaque message est rattaché |
//! | **à jour** | base déjà regroupée | on ne lit qu'un `PRAGMA` |
//!
//! Le second est le cas courant — celui que l'utilisateur paie à *chaque*
//! démarrage. Le premier ne se paie qu'une fois, mais c'est celui qui peut
//! faire sauter le budget.
//!
//! Contrairement aux `diagnostic_*`, ce banc **écrit** : il travaille donc
//! sur une copie qu'il fabrique lui-même par `VACUUM INTO`, et ne touche
//! jamais la base visée. (`VACUUM` compacte au passage : le chiffre est
//! donc un minorant très léger, la fragmentation réelle en moins.)
//!
//! ```powershell
//! cargo run -p mail-core --example banc_migration_fils --release -- "<chemin.db>"
//! ```

use std::path::PathBuf;
use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : banc_migration_fils <chemin.db>")?;
    println!("base : {path}\n");

    let copie = copie_de_travail(&path)?;

    // Ce que la base contient, avant toute chose.
    let source = Connection::open(&copie)?;
    let messages: i64 = source.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?;
    let boites: i64 = source.query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))?;
    println!("{messages} messages, {boites} boîte(s)");

    // Rembobiner : un marqueur de version en retard suffit à faire
    // reprendre le regroupement de zéro à la prochaine ouverture — c'est
    // exactement l'état d'une base qui n'a jamais vu les conversations.
    source.execute_batch("PRAGMA user_version = 0;")?;
    drop(source);

    let depart = Instant::now();
    let store = Store::open(&copie)?;
    let adoption = depart.elapsed();
    drop(store);

    // Deuxième ouverture : la base est désormais à jour. C'est le coût
    // que l'utilisateur paie à CHAQUE démarrage, et le seul qui compte
    // pour le budget en régime courant.
    let depart = Instant::now();
    let store = Store::open(&copie)?;
    let a_jour = depart.elapsed();
    drop(store);

    let verif = Connection::open(&copie)?;
    let fils: i64 = verif.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
    let liens: i64 = verif.query_row("SELECT COUNT(*) FROM thread_links", [], |row| row.get(0))?;
    let orphelins: i64 = verif.query_row(
        "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
        [],
        |row| row.get(0),
    )?;

    println!("\n--- ouverture ---");
    println!("adoption (base héritée) : {adoption:?}");
    println!("à jour   (cas courant)  : {a_jour:?}");
    println!("\n--- résultat du regroupement ---");
    println!("{fils} fils, {liens} liens d'annuaire, {orphelins} message(s) non rattaché(s)");
    if orphelins > 0 {
        println!("⚠ un message non rattaché n'a PAS de ligne dans la liste (ADR 0008 §4)");
    }

    let _ = std::fs::remove_file(&copie);
    Ok(())
}

/// Une copie cohérente, sans toucher à la base visée ni à son WAL.
fn copie_de_travail(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let copie = std::env::temp_dir().join("banc-migration-fils.db");
    let _ = std::fs::remove_file(&copie);
    let source = Connection::open(path)?;
    // `VACUUM INTO` lit la base telle qu'elle est, WAL compris, et écrit
    // un fichier autonome. Aucune écriture sur l'original.
    source.execute("VACUUM INTO ?1", [copie.to_string_lossy().as_ref()])?;
    Ok(copie)
}
