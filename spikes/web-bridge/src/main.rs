//! Spike Phase 0 — pont web (problème dur n°2 du plan).
//!
//! Question à trancher : un navigateur ne peut pas parler IMAP ; que coûte
//! l'architecture « un seul cerveau » où la même base SQLite que le desktop
//! est servie en JSON à une page web par le moteur Rust ?
//!
//! Deux modes : `--offline` (données de démonstration, aucun credential) ou
//! par défaut synchro Gmail réelle puis service HTTP.
//! Code jetable : il valide des décisions, il ne rejoindra pas `mail-core` tel quel.

mod db;
mod gmail;
mod server;
mod sync;

use std::path::PathBuf;
use std::time::Instant;

const PORT: u16 = 8990;

fn main() -> anyhow::Result<()> {
    let offline = std::env::args().any(|arg| arg == "--offline");
    let db_path = PathBuf::from(
        std::env::var("SPIKE_DB_PATH").unwrap_or_else(|_| "target/spike-web.db".to_string()),
    );
    let mut store = db::Store::open(&db_path)?;

    let session = if offline {
        println!("Mode hors-ligne (--offline) : pas d'IMAP, données de démonstration.");
        seed_demo_data(&mut store)?;
        None
    } else {
        let timer = Instant::now();
        let (mut session, email) = gmail::connect()?;
        println!("Connecté ({email}) en {:?}", timer.elapsed());
        let timer = Instant::now();
        let report = sync::sync_inbox(&mut session, &mut store)?;
        println!(
            "Synchronisation {} : {} enveloppe(s) récupérée(s), {} supprimée(s), \
             {} sur le serveur, en {:?}",
            report.mode,
            report.fetched,
            report.deleted,
            report.total_on_server,
            timer.elapsed()
        );
        Some(session)
    };

    server::serve(store, session, PORT)
}

fn seed_demo_data(store: &mut db::Store) -> anyhow::Result<()> {
    if store.count()? > 0 {
        return Ok(());
    }
    let rows: Vec<db::EnvelopeRow> = (1..=5)
        .map(|i| db::EnvelopeRow {
            uid: i,
            subject: format!("Message de démonstration n°{i}"),
            sender: "demo@example.com".to_string(),
            date: format!("2026-07-{:02}T09:00:00+00:00", 6 + i),
        })
        .collect();
    store.insert(&rows)
}
