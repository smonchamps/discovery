//! Banc du gate 3 : le coût d'une page de liste dépend-il de la taille
//! de la boîte ?
//!
//! L'[ADR 0008] §4 fonde tout le regroupement sur une promesse : la liste
//! part de l'agrégat matérialisé `threads`, dont l'index
//! `idx_threads_date` porte **à la fois le tri et la pagination**, donc
//! « le coût d'une page ne dépend plus de la taille de la boîte ».
//!
//! Le gate 3 la met à l'épreuve : 87 ms par page sur 160 000
//! conversations, contre 3,8 ms sur les 2 727 de la boîte réelle.
//!
//! L'index d'origine était `threads(mailbox_id, last_epoch DESC,
//! last_uid DESC)`. La **boîte unifiée** interroge la même boîte de TOUS
//! les comptes : elle filtre sur `m.name = 'INBOX'`, pas sur un
//! `mailbox_id`. Un index préfixé par cette colonne ne pouvait donc plus
//! porter l'ordre global, et SQLite retombait sur un tri matérialisé.
//!
//! **Confirmé, puis corrigé** : `idx_threads_date_globale` porte le même
//! tri sans préfixe de boîte. Le banc reste — c'est lui qui détectera la
//! prochaine régression, et un test unitaire garde le plan
//! (`la_boite_unifiee_ne_materialise_pas_son_tri`).
//!
//! Le défilement profond, lui, a été corrigé au gate P1 de la refonte
//! (2026-08-11) : la pagination vit dans une sous-requête sur `threads`
//! seul, et `OFFSET` ne fait plus exécuter jointures et `EXISTS` sur les
//! lignes sautées — cœur mesuré de 252,6 à 14,6 ms à l'offset 200 000.
//! Le témoin ci-dessous reflète cette forme.
//!
//! Lecture seule : aucune écriture, aucune copie.
//!
//! ```powershell
//! cargo run -p mail-core --example banc_page_liste --release -- "<chemin.db>"
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

/// La forme exacte du tri et de la pagination de `Store::unified_recent`.
/// La projection est réduite : elle ne change pas la STRATÉGIE de tri,
/// qui est tout ce que le plan doit nous dire.
const PAGE_UNIFIEE: &str = "SELECT t.last_uid
     FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch
             FROM threads
            WHERE inbox_size > 0
            ORDER BY last_epoch DESC, last_uid DESC, account_id
            LIMIT 200 OFFSET 0) t
     JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
     JOIN mailboxes m ON m.id = e.mailbox_id
     JOIN accounts a ON a.id = t.account_id
     ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id";

/// La même, mais bornée à UN compte : l'index préfixé redevient utilisable.
const PAGE_UN_COMPTE: &str = "SELECT t.id
     FROM threads t
     JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
     WHERE t.account_id = ?1
     ORDER BY t.last_epoch DESC, t.last_uid DESC
     LIMIT 200 OFFSET 0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : banc_page_liste <chemin.db>")?;
    println!("base : {path}\n");

    // Ouvrir le Store D'ABORD : c'est lui qui applique le schéma, donc
    // qui crée les index. Interroger un plan avant cette ouverture
    // décrirait une base que l'application n'utilise jamais.
    let depart = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!("ouverture (schéma appliqué) : {:?}", depart.elapsed());
    drop(store);

    let conn = Connection::open(&path)?;
    let fils: i64 = conn.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
    let boites: i64 = conn.query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))?;
    // La liste ne pagine QUE les fils ayant un message reçu. Prendre le
    // total ferait mesurer des pages qui n'existent pas — le banc rendait
    // « 0 lignes » en annonçant une durée, ce qui ne mesure rien.
    //
    // L'écart entre les deux est lui-même le chiffre intéressant : c'est
    // ce que l'index partiel écarte (ADR 0009 §4).
    let visibles: i64 = conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE inbox_size > 0",
        [],
        |row| row.get(0),
    )?;
    println!("{fils} conversations réparties sur {boites} boîte(s)");
    println!("dont {visibles} avec au moins un message reçu — seules celles-là sont paginées");

    println!("\n--- plan de la boîte unifiée ---");
    plan(&conn, PAGE_UNIFIEE, rusqlite::params![])?;

    let un: Option<i64> = conn
        .query_row("SELECT id FROM accounts LIMIT 1", [], |row| row.get(0))
        .ok();
    if let Some(account_id) = un {
        println!("\n--- plan d'UN compte (témoin) ---");
        plan(&conn, PAGE_UN_COMPTE, rusqlite::params![account_id])?;
    }
    drop(conn);

    // Le vrai chemin, celui que l'UI emprunte à chaque page de
    // défilement. Trois profondeurs : si le coût suit l'OFFSET, la
    // pagination n'est pas portée par l'index.
    let store = Store::open(std::path::Path::new(&path))?;
    println!("\n--- coût réel d'une page (Store::unified_recent, 200 lignes) ---");
    for offset in [0usize, 20_000, 80_000, 150_000] {
        if offset as i64 >= visibles {
            continue;
        }
        // Deux tours : le premier chauffe le cache de pages SQLite, le
        // second mesure le régime établi — c'est celui du défilement.
        let _ = store.unified_recent(offset, 200)?;
        let depart = Instant::now();
        let lignes = store.unified_recent(offset, 200)?;
        println!(
            "offset {offset:>7} : {:>8.2} ms ({} lignes)",
            depart.elapsed().as_secs_f64() * 1000.0,
            lignes.len()
        );
    }
    Ok(())
}

fn plan(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let lignes: Vec<String> = stmt
        .query_map(params, |row| row.get::<_, String>(3))?
        .collect::<Result<_, _>>()?;
    for ligne in lignes {
        // Le mot qui décide. « FOR ORDER BY » seul = SQLite trie TOUT :
        // aucun index ne porte l'ordre, le coût suit la taille de la
        // boîte. « FOR LAST TERM OF ORDER BY » ne départage que les
        // ex æquo du dernier critère — négligeable, et à ne pas confondre
        // avec le précédent sous peine de crier au loup.
        let verdict = if ligne.contains("TEMP B-TREE FOR ORDER BY") {
            "  ← TRI COMPLET : la promesse de l'ADR 0008 §4 est rompue"
        } else if ligne.contains("TEMP B-TREE") {
            "  ← tri partiel (ex æquo seulement), sans conséquence"
        } else {
            ""
        };
        println!("  {ligne}{verdict}");
    }
    Ok(())
}
