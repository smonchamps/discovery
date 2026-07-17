//! Protocole de mesure partagé : les deux moteurs passent exactement le
//! même banc — 3 échauffements, 50 itérations chronométrées, médiane et
//! p95. Le budget du plan (< 100 ms sur 100 000 messages) se juge au p95.

use std::time::{Duration, Instant};

pub struct QueryStat {
    pub label: &'static str,
    pub hits: usize,
    pub median: Duration,
    pub p95: Duration,
}

pub struct EngineReport {
    pub name: &'static str,
    pub indexed: usize,
    pub build: Duration,
    pub incremental: Duration,
    pub disk_bytes: u64,
    pub queries: Vec<QueryStat>,
}

const WARMUP: usize = 3;
const RUNS: usize = 50;

/// Chronomètre une requête top-50 : `f` retourne le nombre de lignes
/// réellement matérialisées (pour empêcher toute élision).
pub fn measure<F: FnMut() -> usize>(mut f: F) -> (Duration, Duration) {
    for _ in 0..WARMUP {
        std::hint::black_box(f());
    }
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        std::hint::black_box(f());
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    (samples[RUNS / 2], samples[(RUNS * 95) / 100 - 1])
}

pub fn print(reports: &[EngineReport], budget: Duration) {
    for report in reports {
        println!("\n=== {} ===", report.name);
        println!(
            "  construction ({} docs) : {:>10.2?}",
            report.indexed, report.build
        );
        println!(
            "  incrémental (500 docs)      : {:>10.2?}",
            report.incremental
        );
        println!(
            "  taille sur disque           : {:>10.1} Mo",
            report.disk_bytes as f64 / 1_048_576.0
        );
        println!(
            "  {:<38} {:>8} {:>12} {:>12}  budget",
            "requête (top-50 + ranking)", "hits", "médiane", "p95"
        );
        for query in &report.queries {
            let verdict = if query.p95 <= budget { "✅" } else { "❌" };
            println!(
                "  {:<38} {:>8} {:>12.2?} {:>12.2?}  {}",
                query.label, query.hits, query.median, query.p95, verdict
            );
        }
    }
}
