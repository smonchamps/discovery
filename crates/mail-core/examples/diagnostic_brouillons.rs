//! Diagnostic de la synchronisation des brouillons.
//!
//! Répond à la question que l'écran ne peut pas montrer : **le tirage
//! a-t-il fait son travail ?** Le bandeau des brouillons n'affiche que le
//! sujet et le destinataire — deux versions successives du même brouillon
//! y sont donc visuellement identiques, et « rien n'a changé » ne prouve
//! rien.
//!
//! Même discipline que les autres diagnostics : ni sujet, ni destinataire,
//! ni corps. Seulement des repères techniques et la TAILLE du texte, qui
//! suffit à distinguer deux versions sans en révéler une seule.
//!
//! ```powershell
//! cargo run -p mail-core --example diagnostic_brouillons -- "$env:APPDATA\dev.discovery.app\discovery.db"
//! ```

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : diagnostic_brouillons <chemin.db>")?;
    let conn = Connection::open(&path)?;
    println!("base : {path}\n");

    let mut stmt = conn.prepare(
        "SELECT d.id, a.email, d.remote_uid, d.updated_epoch, d.pushed_epoch,
                LENGTH(d.body), LENGTH(d.to_raw), LENGTH(d.subject)
         FROM drafts d LEFT JOIN accounts a ON a.id = d.account_id
         ORDER BY d.account_id, d.id",
    )?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        i64,
        Option<String>,
        Option<u32>,
        i64,
        Option<i64>,
        i64,
        i64,
        i64,
    )> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    println!("--- brouillons ---");
    if rows.is_empty() {
        println!("aucun");
    }
    for (id, email, remote_uid, updated, pushed, body, to, subject) in rows {
        // « Miroir » : une copie distante existe et rien n'a été tapé ici
        // depuis. C'est la seule condition sous laquelle le tirage
        // s'autorise à le remplacer.
        let etat = match (remote_uid, pushed) {
            (Some(_), Some(pushed)) if pushed >= updated => "miroir (remplaçable)",
            (Some(_), Some(_)) => "ÉDITÉ ICI depuis la poussée",
            (Some(_), None) => "copie distante sans repère",
            (None, _) => "jamais poussé",
        };
        let uid = remote_uid
            .map(|uid| uid.to_string())
            .unwrap_or_else(|| "—".to_string());
        println!(
            "#{id} [{}] uid distant {uid} — {etat}\n    \
             texte {body} car., destinataire {to} car., sujet {subject} car.\n    \
             modifié {updated}, poussé {}",
            email.unwrap_or_else(|| "(compte inconnu)".to_string()),
            pushed
                .map(|epoch| epoch.to_string())
                .unwrap_or_else(|| "jamais".to_string()),
        );
    }

    println!("\n--- repères distants ---");
    let mut stmt = conn.prepare(
        "SELECT a.email, r.uid_validity,
                (SELECT COUNT(*) FROM draft_tombstones t WHERE t.account_id = r.account_id)
         FROM drafts_remote r LEFT JOIN accounts a ON a.id = r.account_id",
    )?;
    let reperes: Vec<(Option<String>, i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    if reperes.is_empty() {
        println!("aucun — le cycle de brouillons n'a jamais abouti");
    }
    for (email, validity, tombstones) in reperes {
        println!(
            "{} : UIDVALIDITY {validity}, {tombstones} copie(s) en attente de purge",
            email.unwrap_or_else(|| "(compte inconnu)".to_string())
        );
    }

    Ok(())
}
