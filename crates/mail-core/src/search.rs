//! Recherche plein-texte locale — FTS5, la décision gelée de l'ADR 0004.
//!
//! Trois invariants structurent ce module :
//! - **l'index vit DANS la base** : chaque point de mutation des messages
//!   ([`Store::upsert_envelopes`], suppressions, [`Store::save_body`],
//!   remise à zéro UIDVALIDITY) entretient l'index dans SA transaction —
//!   pas de second magasin, pas de réconciliation après crash ;
//! - **l'index est « sans contenu »** (`content=''`, `contentless_delete`) :
//!   aucun texte n'est dupliqué, seul l'index inversé est stocké — la
//!   vigilance taille de l'ADR 0004 ;
//! - **la saisie n'est JAMAIS de la syntaxe FTS5** : chaque terme est
//!   neutralisé entre guillemets — `AND`, `(`, `*` sont des mots comme
//!   les autres.
//!
//! Les rowids d'`envelopes` sont instables (`INSERT OR REPLACE`) : la
//! table `search_docs` attribue un docid stable par `(mailbox_id, uid)`.
//! Elle est sans clé étrangère à dessein — l'entretien passe uniquement
//! par les fonctions de ce module, jamais par un CASCADE silencieux.

use chrono::NaiveDate;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::{SELECT_UNIFIED, Store, UnifiedRow, row_to_unified};

/// Crée l'index à la première ouverture qui le découvre absent, et le
/// reconstruit depuis les messages déjà en base : une base des phases
/// précédentes devient cherchable sans resynchroniser.
pub(crate) fn migrate_search(conn: &Connection) -> Result<(), Error> {
    let already: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'search_docs'",
        [],
        |row| row.get(0),
    )?;
    if already > 0 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE search_docs (
            docid      INTEGER PRIMARY KEY,
            mailbox_id INTEGER NOT NULL,
            uid        INTEGER NOT NULL,
            UNIQUE (mailbox_id, uid)
         );
         CREATE VIRTUAL TABLE search_fts USING fts5(
            subject, sender, body,
            content='', contentless_delete=1,
            tokenize='unicode61 remove_diacritics 2'
         );",
    )?;
    rebuild(&tx)?;
    tx.commit()?;
    Ok(())
}

fn rebuild(conn: &Connection) -> Result<(), Error> {
    let mut stmt = conn.prepare(
        "SELECT e.mailbox_id, e.uid, e.subject, e.sender, e.sender_address, b.html
         FROM envelopes e
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Uid>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (mailbox_id, uid, subject, sender, address, html) in rows {
        index_message(
            conn,
            mailbox_id,
            uid,
            subject.as_deref(),
            sender.as_deref(),
            address.as_deref(),
            html.as_deref(),
        )?;
    }
    Ok(())
}

/// (Ré)indexe un message. À appeler dans la transaction qui écrit le
/// message lui-même — l'index et la donnée vivent ou meurent ensemble.
pub(crate) fn index_message(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
    subject: Option<&str>,
    sender: Option<&str>,
    sender_address: Option<&str>,
    body_html: Option<&str>,
) -> Result<(), Error> {
    deindex_message(conn, mailbox_id, uid)?;
    conn.execute(
        "INSERT INTO search_docs (mailbox_id, uid) VALUES (?1, ?2)",
        params![mailbox_id, uid],
    )?;
    let docid = conn.last_insert_rowid();
    let sender_field = [sender, sender_address]
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    conn.execute(
        "INSERT INTO search_fts (rowid, subject, sender, body) VALUES (?1, ?2, ?3, ?4)",
        params![
            docid,
            subject.unwrap_or(""),
            sender_field,
            body_html.map(indexable_text).unwrap_or_default()
        ],
    )?;
    Ok(())
}

pub(crate) fn deindex_message(conn: &Connection, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
    let docid: Option<i64> = conn
        .query_row(
            "SELECT docid FROM search_docs WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(docid) = docid {
        conn.execute("DELETE FROM search_fts WHERE rowid = ?1", [docid])?;
        conn.execute("DELETE FROM search_docs WHERE docid = ?1", [docid])?;
    }
    Ok(())
}

pub(crate) fn deindex_mailbox(conn: &Connection, mailbox_id: i64) -> Result<(), Error> {
    let docids: Vec<i64> = conn
        .prepare("SELECT docid FROM search_docs WHERE mailbox_id = ?1")?
        .query_map([mailbox_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for docid in docids {
        conn.execute("DELETE FROM search_fts WHERE rowid = ?1", [docid])?;
    }
    conn.execute(
        "DELETE FROM search_docs WHERE mailbox_id = ?1",
        [mailbox_id],
    )?;
    Ok(())
}

impl Store {
    /// Recherche sur TOUS les comptes — les résultats sont des lignes de
    /// la boîte unifiée, triées par pertinence (BM25 ; un mot du sujet
    /// pèse plus lourd qu'un mot du corps). Le dernier terme est un
    /// préfixe : « budg » trouve « budgétaire » pendant la frappe.
    ///
    /// Filtres : `from:`/`de:` (nom ou adresse de l'expéditeur),
    /// `date:AAAA`, `date:AAAA-MM`, `date:AAAA-MM-JJ`. Un filtre seul,
    /// sans terme, liste les messages correspondants par date.
    pub fn search(&self, input: &str, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        let (match_expr, filters) = parse_query(input);
        if match_expr.is_none() && filters.is_empty() {
            return Ok(Vec::new());
        }
        let mut clauses = String::new();
        let mut values: Vec<Value> = Vec::new();
        if let Some(expr) = &match_expr {
            values.push(expr.clone().into());
        }
        if let Some(from) = &filters.from {
            clauses.push_str(
                " AND (e.sender LIKE '%' || ? || '%' OR e.sender_address LIKE '%' || ? || '%')",
            );
            values.push(from.clone().into());
            values.push(from.clone().into());
        }
        if let Some(since) = filters.since {
            clauses.push_str(" AND e.date_epoch >= ?");
            values.push(since.into());
        }
        if let Some(until) = filters.until {
            clauses.push_str(" AND e.date_epoch < ?");
            values.push(until.into());
        }
        values.push((limit as i64).into());
        let sql = if match_expr.is_some() {
            format!(
                "{SELECT_UNIFIED}
                 FROM search_fts
                 JOIN search_docs d ON d.docid = search_fts.rowid
                 JOIN envelopes e ON e.mailbox_id = d.mailbox_id AND e.uid = d.uid
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 JOIN accounts a ON a.id = m.account_id
                 WHERE search_fts MATCH ?{clauses}
                 ORDER BY bm25(search_fts, 10.0, 5.0, 1.0), e.date_epoch DESC
                 LIMIT ?"
            )
        } else {
            format!(
                "{SELECT_UNIFIED}
                 FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 JOIN accounts a ON a.id = m.account_id
                 WHERE 1 = 1{clauses}
                 ORDER BY e.date_epoch DESC, e.uid DESC
                 LIMIT ?"
            )
        };
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(values), row_to_unified)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Default)]
struct Filters {
    from: Option<String>,
    since: Option<i64>,
    until: Option<i64>,
}

impl Filters {
    fn is_empty(&self) -> bool {
        self.from.is_none() && self.since.is_none() && self.until.is_none()
    }
}

/// Découpe la saisie en termes et filtres. Chaque terme est neutralisé
/// entre guillemets FTS5 (les guillemets de l'utilisateur sont retirés) :
/// la syntaxe du moteur est inatteignable depuis le champ de recherche.
fn parse_query(input: &str) -> (Option<String>, Filters) {
    let mut terms: Vec<String> = Vec::new();
    let mut filters = Filters::default();
    for token in input.split_whitespace() {
        let lower = token.to_lowercase();
        if let Some(value) = lower
            .strip_prefix("from:")
            .or_else(|| lower.strip_prefix("de:"))
        {
            if !value.is_empty() {
                filters.from = Some(value.to_string());
            }
        } else if let Some(value) = lower.strip_prefix("date:") {
            // Un filtre de date illisible est ignoré plutôt qu'appliqué
            // de travers : pas de résultat surprise.
            if let Some((since, until)) = parse_date_range(value) {
                filters.since = Some(since);
                filters.until = Some(until);
            }
        } else {
            let clean: String = token.chars().filter(|c| *c != '"').collect();
            if !clean.is_empty() {
                terms.push(clean);
            }
        }
    }
    let last = terms.len().saturating_sub(1);
    let match_expr = (!terms.is_empty()).then(|| {
        terms
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if i == last {
                    format!("\"{t}\"*")
                } else {
                    format!("\"{t}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    });
    (match_expr, filters)
}

/// `2026` → l'année, `2026-07` → le mois, `2026-07-18` → le jour.
/// Bornes UTC, intervalle semi-ouvert `[début, fin)`.
fn parse_date_range(value: &str) -> Option<(i64, i64)> {
    let mut parts = value.splitn(3, '-');
    let year: i32 = parts
        .next()?
        .parse()
        .ok()
        .filter(|y| (1970..=9999).contains(y))?;
    let month: Option<u32> = match parts.next() {
        Some(m) => Some(m.parse().ok()?),
        None => None,
    };
    let day: Option<u32> = match parts.next() {
        Some(d) => Some(d.parse().ok()?),
        None => None,
    };
    let (start, end) = match (month, day) {
        (None, _) => (
            NaiveDate::from_ymd_opt(year, 1, 1)?,
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?,
        ),
        (Some(m), None) => {
            let start = NaiveDate::from_ymd_opt(year, m, 1)?;
            let end = if m == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(year, m + 1, 1)?
            };
            (start, end)
        }
        (Some(m), Some(d)) => {
            let start = NaiveDate::from_ymd_opt(year, m, d)?;
            (start, start.succ_opt()?)
        }
    };
    Some((
        start.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
        end.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
    ))
}

/// Réduit un HTML en mots indexables : balises et contenus `<script>` /
/// `<style>` disparaissent, les entités courantes (dont les accents
/// français) sont décodées, les blancs s'effondrent. Volontairement
/// minimal : l'index a besoin de mots, pas de mise en forme —
/// `mail-render` garde l'extraction fidèle pour la citation.
fn indexable_text(html: &str) -> String {
    // Ombre en minuscules ASCII : mêmes longueurs d'octets, recherches
    // de balises insensibles à la casse sans réallouer à chaque balise.
    let shadow = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() / 2);
    let mut i = 0;
    while let Some(open) = shadow[i..].find('<').map(|p| i + p) {
        out.push_str(&html[i..open]);
        out.push(' ');
        let Some(close) = shadow[open..].find('>').map(|p| open + p) else {
            // Balise jamais fermée : la fin est du bruit de balisage.
            i = html.len();
            break;
        };
        i = close + 1;
        let inner = shadow[open + 1..close].trim_start_matches('/');
        let name_end = inner
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(inner.len());
        let name = &inner[..name_end];
        let is_closing = shadow[open + 1..close].starts_with('/');
        if !is_closing && (name == "script" || name == "style") {
            i = skip_past_closing_tag(&shadow, i, name);
        }
    }
    out.push_str(&html[i..]);
    let decoded = decode_entities(&out);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Position juste après `</name...>`, ou la fin si la fermeture manque.
fn skip_past_closing_tag(shadow: &str, from: usize, name: &str) -> usize {
    let closer = format!("</{name}");
    let Some(at) = shadow[from..].find(&closer).map(|p| from + p) else {
        return shadow.len();
    };
    match shadow[at..].find('>') {
        Some(p) => at + p + 1,
        None => shadow.len(),
    }
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        // Une entité plausible tient en peu de caractères ; au-delà,
        // c'est une esperluette littérale. On limite en CARACTÈRES,
        // pas en octets, pour ne pas couper un caractère multi-octets
        // (ex. 'è') en plein milieu.
        let semi = rest
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(i, _)| i);
        match semi.and_then(|s| decode_entity(&rest[1..s]).map(|c| (c, s))) {
            Some((decoded, s)) => {
                out.push(decoded);
                rest = &rest[s + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    if let Some(num) = entity.strip_prefix('#') {
        let code = match num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match entity {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "agrave" => 'à',
        "acirc" => 'â',
        "ccedil" => 'ç',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "oelig" => 'œ',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: Uid, subject: &str, sender: &str, address: &str, epoch: i64) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some(sender.to_string()),
            sender_address: Some(address.to_string()),
            message_id: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: false,
            flagged: false,
        }
    }

    fn store_with_inbox(email: &str) -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store.adopt_or_create_account(email, "gmail").unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, mailbox)
    }

    fn subjects(rows: &[UnifiedRow]) -> Vec<String> {
        rows.iter()
            .map(|r| r.envelope.subject.clone().unwrap_or_default())
            .collect()
    }

    fn indexed_count(store: &Store) -> i64 {
        store
            .conn()
            .query_row("SELECT COUNT(*) FROM search_docs", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn finds_by_subject_across_accounts() {
        let (mut store, inbox_one) = store_with_inbox("un@exemple.fr");
        let account_two = store
            .adopt_or_create_account("deux@exemple.fr", "gmail")
            .unwrap();
        let inbox_two = store.create_mailbox(account_two, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox_one,
                &[envelope(1, "Rapport mensuel", "Alice", "alice@ex.fr", 100)],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_two,
                &[envelope(1, "Rapport annuel", "Bob", "bob@ex.fr", 200)],
            )
            .unwrap();

        let rows = store.search("rapport", 50).unwrap();
        assert_eq!(rows.len(), 2);
        let emails: Vec<&str> = rows.iter().map(|r| r.account_email.as_str()).collect();
        assert!(emails.contains(&"un@exemple.fr"));
        assert!(emails.contains(&"deux@exemple.fr"));
    }

    #[test]
    fn accents_fold_in_both_directions() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "Réunion budgétaire", "Alice", "a@ex.fr", 100)],
            )
            .unwrap();

        assert_eq!(store.search("reunion", 50).unwrap().len(), 1);
        assert_eq!(store.search("réunion", 50).unwrap().len(), 1);
        assert_eq!(store.search("budgetaire", 50).unwrap().len(), 1);
    }

    #[test]
    fn last_term_is_a_prefix_while_typing() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "Budget prévisionnel", "Alice", "a@ex.fr", 100)],
            )
            .unwrap();

        assert_eq!(store.search("prévi", 50).unwrap().len(), 1);
        assert_eq!(store.search("budget prévi", 50).unwrap().len(), 1);
        assert_eq!(
            store.search("prévi budget", 50).unwrap().len(),
            0,
            "seul le dernier terme est un préfixe : les autres sont des mots entiers"
        );
    }

    #[test]
    fn body_words_are_indexed_markup_is_not() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "Sans indice", "Alice", "a@ex.fr", 100)],
            )
            .unwrap();
        assert_eq!(store.search("contrat", 50).unwrap().len(), 0);

        store
            .save_body(
                inbox,
                1,
                "<div style=\"color:red\">le contrat est sign\u{e9}</div>\
                 <style>.x{font-size:12px}</style>\
                 <script>var couleur = \"bleu\";</script>",
            )
            .unwrap();

        assert_eq!(store.search("contrat", 50).unwrap().len(), 1);
        assert_eq!(store.search("signe", 50).unwrap().len(), 1, "accent replié");
        assert_eq!(
            store.search("color", 50).unwrap().len(),
            0,
            "attributs exclus"
        );
        assert_eq!(store.search("div", 50).unwrap().len(), 0, "balises exclues");
        assert_eq!(store.search("bleu", 50).unwrap().len(), 0, "scripts exclus");
    }

    #[test]
    fn html_entities_decode_for_french_words() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Invitation", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store
            .save_body(
                inbox,
                1,
                "<p>r&eacute;union &amp; caf&eacute; &#233;quipe</p>",
            )
            .unwrap();

        assert_eq!(store.search("reunion", 50).unwrap().len(), 1);
        assert_eq!(store.search("cafe", 50).unwrap().len(), 1);
        assert_eq!(store.search("equipe", 50).unwrap().len(), 1);
    }

    #[test]
    fn subject_hit_outranks_body_hit() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Divers", "Alice", "a@ex.fr", 300),
                    envelope(2, "Facture du mois", "Alice", "a@ex.fr", 100),
                ],
            )
            .unwrap();
        store
            .save_body(inbox, 1, "<p>la facture est jointe</p>")
            .unwrap();

        let rows = store.search("facture", 50).unwrap();
        assert_eq!(
            subjects(&rows),
            vec!["Facture du mois", "Divers"],
            "le sujet pèse plus lourd que le corps, malgré une date plus ancienne"
        );
    }

    #[test]
    fn reupsert_replaces_the_index_entry() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(inbox, &[envelope(1, "avant", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "après", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert_eq!(store.search("avant", 50).unwrap().len(), 0);
        assert_eq!(store.search("après", 50).unwrap().len(), 1);
        assert_eq!(
            indexed_count(&store),
            1,
            "une seule entrée d'index par message"
        );
    }

    #[test]
    fn reupsert_keeps_the_indexed_body() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Sujet", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store.save_body(inbox, 1, "<p>le contrat</p>").unwrap();
        // Une synchro repasse sur l'enveloppe (drapeau lu, par exemple).
        store
            .upsert_envelopes(inbox, &[envelope(1, "Sujet", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert_eq!(
            store.search("contrat", 50).unwrap().len(),
            1,
            "le corps déjà en cache reste indexé après réécriture de l'enveloppe"
        );
    }

    #[test]
    fn local_removal_cleans_the_index() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Éphémère", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store.remove_local(inbox, 1).unwrap();

        assert_eq!(store.search("éphémère", 50).unwrap().len(), 0);
        assert_eq!(indexed_count(&store), 0);
    }

    #[test]
    fn absent_removal_cleans_the_index() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Disparu du serveur", "Alice", "a@ex.fr", 100),
                    envelope(2, "Toujours là", "Alice", "a@ex.fr", 200),
                ],
            )
            .unwrap();
        store
            .remove_absent(inbox, &std::collections::HashSet::from([2]))
            .unwrap();

        assert_eq!(store.search("disparu", 50).unwrap().len(), 0);
        assert_eq!(store.search("toujours", 50).unwrap().len(), 1);
    }

    #[test]
    fn uidvalidity_reset_clears_only_that_mailbox() {
        let (mut store, inbox_one) = store_with_inbox("un@exemple.fr");
        let account_two = store
            .adopt_or_create_account("deux@exemple.fr", "gmail")
            .unwrap();
        let inbox_two = store.create_mailbox(account_two, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox_one, &[envelope(1, "Rapport un", "A", "a@ex.fr", 100)])
            .unwrap();
        store
            .upsert_envelopes(
                inbox_two,
                &[envelope(1, "Rapport deux", "B", "b@ex.fr", 200)],
            )
            .unwrap();

        store.reset_mailbox(inbox_one, 2).unwrap();

        assert_eq!(
            subjects(&store.search("rapport", 50).unwrap()),
            vec!["Rapport deux"]
        );
        assert_eq!(indexed_count(&store), 1);
    }

    #[test]
    fn hostile_input_is_literal_never_fts_syntax() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "budget (T3) \"spécial\"",
                    "Alice",
                    "a@ex.fr",
                    100,
                )],
            )
            .unwrap();

        for hostile in [
            "budget AND",
            "AND",
            "OR NOT",
            "(",
            ")",
            "*",
            "\"",
            "\" OR \"",
            "NEAR(",
            "bud*get",
            "sujet:x",
            "-budget",
        ] {
            assert!(
                store.search(hostile, 50).is_ok(),
                "la saisie « {hostile} » ne doit jamais être de la syntaxe FTS5"
            );
        }
        // Les opérateurs sont des mots : « AND » seul ne matche rien ici.
        assert_eq!(store.search("AND", 50).unwrap().len(), 0);
    }

    #[test]
    fn from_filter_narrows_by_name_or_address() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Rapport ventes", "Alice Martin", "alice@ex.fr", 100),
                    envelope(2, "Rapport achats", "Bob Durand", "bob@ex.fr", 200),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("rapport from:alice", 50).unwrap()),
            vec!["Rapport ventes"]
        );
        assert_eq!(
            subjects(&store.search("rapport de:durand", 50).unwrap()),
            vec!["Rapport achats"],
            "le filtre matche le nom affiché comme l'adresse"
        );
    }

    #[test]
    fn date_filter_bounds_by_year_month_or_day() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        let in_2025 = Utc
            .with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        let in_2026 = Utc
            .with_ymd_and_hms(2026, 7, 1, 9, 0, 0)
            .unwrap()
            .timestamp();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Rapport ancien", "Alice", "a@ex.fr", in_2025),
                    envelope(2, "Rapport récent", "Alice", "a@ex.fr", in_2026),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("rapport date:2026", 50).unwrap()),
            vec!["Rapport récent"]
        );
        assert_eq!(
            subjects(&store.search("rapport date:2026-07", 50).unwrap()),
            vec!["Rapport récent"]
        );
        assert_eq!(
            subjects(&store.search("rapport date:2025-06-15", 50).unwrap()),
            vec!["Rapport ancien"]
        );
        assert_eq!(store.search("rapport date:2024", 50).unwrap().len(), 0);
        assert_eq!(
            store.search("rapport date:n'importe", 50).unwrap().len(),
            2,
            "un filtre de date illisible est ignoré, pas appliqué de travers"
        );
    }

    #[test]
    fn filter_alone_lists_by_date_without_terms() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Premier d'Alice", "Alice", "alice@ex.fr", 100),
                    envelope(2, "De Bob", "Bob", "bob@ex.fr", 200),
                    envelope(3, "Second d'Alice", "Alice", "alice@ex.fr", 300),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("from:alice", 50).unwrap()),
            vec!["Second d'Alice", "Premier d'Alice"],
            "un filtre seul liste par date, du plus récent au plus ancien"
        );
    }

    #[test]
    fn blank_query_returns_nothing() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Sujet", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert!(store.search("", 50).unwrap().is_empty());
        assert!(store.search("   ", 50).unwrap().is_empty());
        assert!(store.search("\"\"", 50).unwrap().is_empty());
    }

    #[test]
    fn limit_caps_the_result_set() {
        let (mut store, inbox) = store_with_inbox("test@exemple.fr");
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "Rapport", "Alice", "a@ex.fr", uid as i64))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();

        assert_eq!(store.search("rapport", 3).unwrap().len(), 3);
    }

    #[test]
    fn strips_markup_and_collapses_whitespace() {
        assert_eq!(
            indexable_text("<p>un\n  <b>deux</b></p>   trois"),
            "un deux trois"
        );
        assert_eq!(
            indexable_text("avant <img src=x"),
            "avant",
            "balise jamais fermée"
        );
        assert_eq!(
            indexable_text("caf&eacute; &amp; th&eacute; &inconnu; &#x41;"),
            "café & thé &inconnu; A"
        );
    }

    /// Régression : une esperluette suivie d'un caractère multi-octets
    /// juste après la fenêtre de 12 caractères ne doit pas couper le
    /// caractère en plein milieu.
    #[test]
    fn ampersand_before_multibyte_char_does_not_panic() {
        assert_eq!(
            indexable_text("&quot; (modèle avec médecins)"),
            "\" (modèle avec médecins)"
        );
        // Esperluette non suivie d'entité, avec un caractère multi-octets
        // qui déborde de la limite des 12 caractères.
        assert_eq!(
            indexable_text("modèle & clinique de médecins"),
            "modèle & clinique de médecins"
        );
    }
}
