//! Spike Phase 3 : FTS5 contre Tantivy, départagés par des chiffres sur
//! le critère du plan — recherche < 100 ms sur 100 000 messages
//! (PLAN.md §1 et §2.4). Corpus déterministe, protocole identique.
//!
//! ```powershell
//! cargo run --release -- [nombre_de_docs]
//! ```

mod corpus;
mod fts5;
mod report;
mod tantivy_bench;

use std::path::Path;
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    let count: u64 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let out = Path::new("target/spike");
    std::fs::create_dir_all(out)?;

    println!("génération du corpus ({count} + 500 docs incrémentaux)…");
    let timer = Instant::now();
    let docs = corpus::generate(0..count);
    let incremental = corpus::generate(count..count + 500);
    println!("corpus prêt en {:.2?}", timer.elapsed());

    let fts = fts5::run(&out.join("fts5.db"), &docs, &incremental)?;
    let tantivy = tantivy_bench::run(&out.join("tantivy"), &docs, &incremental)?;

    report::print(&[fts, tantivy], Duration::from_millis(100));

    println!(
        "\nRappels d'interprétation :\n\
         - budget du plan : p95 < 100 ms sur 100 000 messages ;\n\
         - « accents » et « préfixe » relèvent des CAPACITÉS, pas que la vitesse\n\
           (Tantivy : pas de remove_diacritics par défaut, préfixe via regex) ;\n\
         - FTS5 vit DANS la base SQLite : suppression de message et d'index\n\
           dans la même transaction — Tantivy est un second magasin à réconcilier."
    );
    Ok(())
}
