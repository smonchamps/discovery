//! Spike Phase 0 — moteur de synchronisation (problème dur n°1 du plan).
//!
//! Question à trancher : peut-on synchroniser les enveloppes d'INBOX vers
//! SQLite de façon incrémentale, avec une liste utilisable instantanément
//! hors-ligne, dans les budgets du plan (liste < 1 s) ?
//! Code jetable : il valide des décisions, il ne rejoindra pas `mail-core` tel quel.

mod db;
mod gmail;
mod sync;

use std::path::PathBuf;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let db_path = PathBuf::from(
        std::env::var("SPIKE_DB_PATH").unwrap_or_else(|_| "target/spike-sync.db".to_string()),
    );
    let mut store = db::Store::open(&db_path)?;

    // 1. Lecture offline AVANT toute connexion réseau : c'est le cœur de la
    //    promesse produit — la liste doit être utilisable immédiatement.
    let offline_timer = Instant::now();
    let cached = store.recent(10)?;
    let cached_total = store.count()?;
    let offline_elapsed = offline_timer.elapsed();
    if cached.is_empty() {
        println!("Cache vide (premier lancement) : synchronisation initiale nécessaire.");
    } else {
        println!(
            "Liste offline servie en {offline_elapsed:?} ({cached_total} enveloppes en cache) :"
        );
        print_rows(&cached);
    }

    // 2. Connexion (OAuth silencieux via le refresh token du spike oauth-gmail).
    let connect_timer = Instant::now();
    let (mut session, email) = gmail::connect()?;
    let connect_elapsed = connect_timer.elapsed();
    println!("\nConnecté ({email}) en {connect_elapsed:?}");
    println!(
        "Capabilities utiles du serveur : {:?}",
        gmail::interesting_capabilities(&mut session)?
    );

    // 3. Synchronisation.
    let sync_timer = Instant::now();
    let report = sync::sync_inbox(&mut session, &mut store)?;
    let sync_elapsed = sync_timer.elapsed();
    let _ = session.logout();

    // 4. Relecture depuis SQLite : ce que verrait l'utilisateur après sync.
    let list_timer = Instant::now();
    let fresh = store.recent(10)?;
    let list_elapsed = list_timer.elapsed();
    println!("\nListe à jour (servie depuis SQLite en {list_elapsed:?}) :");
    print_rows(&fresh);

    print_summary(&store, &db_path, &report, sync_elapsed, offline_elapsed)?;
    Ok(())
}

fn print_rows(rows: &[db::EnvelopeRow]) {
    for row in rows {
        let date = row.date.chars().take(10).collect::<String>();
        println!(
            "  {date}  {:30}  {}",
            truncate(&row.sender, 30),
            truncate(&row.subject, 60)
        );
    }
}

fn print_summary(
    store: &db::Store,
    db_path: &std::path::Path,
    report: &sync::SyncReport,
    sync_elapsed: std::time::Duration,
    offline_elapsed: std::time::Duration,
) -> anyhow::Result<()> {
    let db_size_kb = std::fs::metadata(db_path)
        .map(|m| m.len() / 1024)
        .unwrap_or(0);
    println!("\n--- Bilan ---");
    println!(
        "Synchronisation {} : {} enveloppe(s) récupérée(s), {} supprimée(s), en {sync_elapsed:?}",
        report.mode, report.fetched, report.deleted
    );
    println!(
        "Enveloppes en base : {} (serveur : {})",
        store.count()?,
        report.total_on_server
    );
    println!("Base SQLite : {db_size_kb} Ko ({})", db_path.display());
    println!("Lecture offline : {offline_elapsed:?} (budget du plan : < 1 s)");
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}
