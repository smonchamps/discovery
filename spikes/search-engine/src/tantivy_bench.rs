//! Banc Tantivy — l'alternative de la grille set-based (PLAN.md §2.4).
//!
//! Configuration par défaut honnête : tokenizer standard (minuscules,
//! PAS de suppression des diacritiques — capacité relevée par le banc),
//! ranking BM25 natif, écrivain multi-threads. Le préfixe passe par
//! RegexQuery : Tantivy n'a pas de préfixe natif côté requête simple
//! (il faudrait des edge-ngrams à l'indexation) — relevé aussi.

use std::path::Path;
use std::time::Instant;

use tantivy::collector::{Count, TopDocs};
use tantivy::query::{Query, QueryParser, RegexQuery};
use tantivy::schema::{FAST, INDEXED, STORED, Schema, TEXT};
use tantivy::{Index, IndexWriter, doc};

use crate::corpus::Doc;
use crate::report::{EngineReport, QueryStat, measure};

pub fn run(dir: &Path, docs: &[Doc], incremental: &[Doc]) -> anyhow::Result<EngineReport> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;

    let mut builder = Schema::builder();
    let id_field = builder.add_u64_field("id", INDEXED | STORED | FAST);
    let subject = builder.add_text_field("subject", TEXT);
    let sender = builder.add_text_field("sender", TEXT);
    let body = builder.add_text_field("body", TEXT);
    let schema = builder.build();

    let index = Index::create_in_dir(dir, schema)?;
    let mut writer: IndexWriter = index.writer(128_000_000)?;

    let build_timer = Instant::now();
    for entry in docs {
        writer.add_document(doc!(
            id_field => entry.id,
            subject => entry.subject.as_str(),
            sender => entry.sender.as_str(),
            body => entry.body.as_str(),
        ))?;
    }
    writer.commit()?;
    let build = build_timer.elapsed();

    let incremental_timer = Instant::now();
    for entry in incremental {
        writer.add_document(doc!(
            id_field => entry.id,
            subject => entry.subject.as_str(),
            sender => entry.sender.as_str(),
            body => entry.body.as_str(),
        ))?;
    }
    writer.commit()?;
    let incremental_elapsed = incremental_timer.elapsed();

    let reader = index.reader()?;
    reader.reload()?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![subject, sender, body]);

    let mut queries = Vec::new();
    let parsed: Vec<(&'static str, Box<dyn Query>)> = vec![
        (
            "terme rare (~121 docs)",
            parser.parse_query("kilimandjaro")?,
        ),
        (
            "terme commun (~16 667 docs)",
            parser.parse_query("budgetaire")?,
        ),
        (
            "ET (~477 docs)",
            parser.parse_query("montagne AND horizon")?,
        ),
        (
            "phrase (~1 000 docs)",
            parser.parse_query("\"comite directeur\"")?,
        ),
        (
            "préfixe search-as-you-type",
            Box::new(RegexQuery::from_pattern("budg.*", body)?),
        ),
        (
            "accents : reunion → réunion",
            parser.parse_query("reunion")?,
        ),
    ];
    for (label, query) in parsed {
        let hits = searcher.search(&query, &Count)?;
        let (median, p95) = measure(|| {
            searcher
                .search(&query, &TopDocs::with_limit(50))
                .map(|top| top.len())
                .unwrap_or(0)
        });
        queries.push(QueryStat {
            label,
            hits,
            median,
            p95,
        });
    }

    let disk_bytes = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum();

    Ok(EngineReport {
        name: "Tantivy 0.22 (tokenizer par défaut)",
        indexed: docs.len(),
        build,
        incremental: incremental_elapsed,
        disk_bytes,
        queries,
    })
}
