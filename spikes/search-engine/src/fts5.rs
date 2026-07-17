//! Banc SQLite FTS5 — l'hypothèse gelée du plan (PLAN.md §2.4).
//!
//! Configuration visée en production : tokenizer `unicode61` avec
//! `remove_diacritics 2` (« reunion » trouve « réunion »), WAL, ranking
//! bm25 natif (`ORDER BY rank`), préfixe natif (`budg*`).

use std::path::Path;
use std::time::Instant;

use anyhow::Context;
use rusqlite::Connection;

use crate::corpus::Doc;
use crate::report::{EngineReport, QueryStat, measure};

pub const QUERIES: &[(&str, &str)] = &[
    ("terme rare (~121 docs)", "kilimandjaro"),
    ("terme commun (~16 667 docs)", "budgetaire"),
    ("ET (~477 docs)", "montagne AND horizon"),
    ("phrase (~1 000 docs)", "\"comite directeur\""),
    ("préfixe search-as-you-type", "budg*"),
    ("accents : reunion → réunion", "reunion"),
];

pub fn run(db_path: &Path, docs: &[Doc], incremental: &[Doc]) -> anyhow::Result<EngineReport> {
    let _ = std::fs::remove_file(db_path);
    let conn = Connection::open(db_path).context("ouverture base")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE fts USING fts5(
            subject, sender, body,
            tokenize='unicode61 remove_diacritics 2'
        );",
    )
    .context("création de la table FTS5 (bundled avec FTS5 ?)")?;

    let build_timer = Instant::now();
    insert(&conn, docs)?;
    let build = build_timer.elapsed();

    let incremental_timer = Instant::now();
    insert(&conn, incremental)?;
    let incremental_elapsed = incremental_timer.elapsed();

    let mut queries = Vec::new();
    for (label, match_expr) in QUERIES {
        let hits: usize = conn.query_row(
            "SELECT count(*) FROM fts WHERE fts MATCH ?1",
            [match_expr],
            |row| row.get(0),
        )?;
        let mut stmt =
            conn.prepare("SELECT rowid FROM fts WHERE fts MATCH ?1 ORDER BY rank LIMIT 50")?;
        let (median, p95) = measure(|| {
            stmt.query_map([match_expr], |row| row.get::<_, i64>(0))
                .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
                .map(|rows| rows.len())
                .unwrap_or(0)
        });
        queries.push(QueryStat {
            label,
            hits,
            median,
            p95,
        });
    }

    // Taille honnête : après repli du WAL dans le fichier principal.
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    let disk_bytes = std::fs::metadata(db_path)?.len();

    Ok(EngineReport {
        name: "SQLite FTS5 (unicode61, remove_diacritics)",
        indexed: docs.len(),
        build,
        incremental: incremental_elapsed,
        disk_bytes,
        queries,
    })
}

fn insert(conn: &Connection, docs: &[Doc]) -> anyhow::Result<()> {
    for chunk in docs.chunks(5_000) {
        conn.execute_batch("BEGIN;")?;
        {
            let mut stmt = conn.prepare_cached(
                "INSERT INTO fts (rowid, subject, sender, body) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for doc in chunk {
                stmt.execute(rusqlite::params![doc.id, doc.subject, doc.sender, doc.body])?;
            }
        }
        conn.execute_batch("COMMIT;")?;
    }
    Ok(())
}
