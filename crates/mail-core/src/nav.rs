//! La navigation de l'écran 02 (refonte v2, PLAN-UI-V2 §P2) : les six
//! dossiers canoniques du prototype — réception, envoyés, brouillons,
//! indésirables, archives, corbeille — résolus sur les boîtes RÉELLES,
//! leurs compteurs, et les pages de liste par catégorie.
//!
//! Le classement est POSITIONNEL — leçon du terrain
//! (`diagnostic_boites`) : un simple `contains()` donnait 26 candidats
//! « archive » sur un compte Gmail portant une migration PST. Seul le
//! DERNIER segment compte, et le dossier doit vivre à la racine ou sous
//! le seul préfixe fournisseur (`[Gmail]/x`) — jamais en profondeur. À
//! candidats multiples, le préfixe fournisseur l'emporte sur l'homonyme
//! racine. « Envoyés » n'est pas deviné : `accounts.sent_mailbox` fait
//! foi (ADR 0009 §7). Le séparateur observé est `/` ; un serveur à
//! séparateur exotique dégrade proprement — seules les racines matchent.
//!
//! Tout est LECTURE : la nav affiche un état, rien de plus (ADR 0001).

use rusqlite::params;

use crate::error::Error;
use crate::store::{
    PINNED_THREADS, SELECT_UNIFIED, Store, THREAD_AGGREGATE, UNIFIED_JOIN_TAIL, UnifiedRow,
    row_to_threaded, unified_page_sql,
};
use crate::thread::RECEIVED_MAILBOX;

/// Les dossiers canoniques d'UN compte, en noms RÉSEAU (`folders.wire`,
/// le même vocabulaire que `sync_state`). `None` = la catégorie n'a pas
/// de dossier reconnu sur ce compte — la nav l'affiche vide, jamais un
/// mauvais choix (« un nom inconnu vaut mieux qu'un mauvais choix »,
/// `mail-imap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFolders {
    pub reception: String,
    pub envoyes: Option<String>,
    pub brouillons: Option<String>,
    pub indesirables: Option<String>,
    pub archives: Option<String>,
    /// Vrai quand `archives` est une INTÉGRALE (« Tous les messages ») :
    /// la catégorie doit alors exclure les messages des autres
    /// canoniques, sinon elle montre toute la boîte.
    pub archives_integrale: bool,
    pub corbeille: Option<String>,
}

/// Les compteurs de la nav pour UN compte. Réception et indésirables
/// portent le héros non-lu du prototype ; les autres, un total simple.
/// Réception compte des CONVERSATIONS (c'est ce que la liste affiche) ;
/// les autres catégories comptent des messages.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NavCounts {
    pub reception_total: u64,
    pub reception_non_lues: u64,
    pub envoyes: u64,
    pub brouillons: u64,
    pub indesirables_total: u64,
    pub indesirables_non_lus: u64,
    pub archives: u64,
    pub corbeille: u64,
}

impl CanonicalFolders {
    /// La boîte d'une catégorie, prête pour `sync_state` — `None` quand
    /// la catégorie n'est pas résolue sur ce compte, ou inconnue.
    pub fn boite(&self, categorie: &str) -> Option<String> {
        match categorie {
            "reception" => Some(self.reception.clone()),
            "envoyes" => self.envoyes.clone(),
            "brouillons" => self.brouillons.clone(),
            "indesirables" => self.indesirables.clone(),
            "archives" => self.archives.clone(),
            "corbeille" => self.corbeille.clone(),
            _ => None,
        }
    }
}

const BROUILLONS: &[&str] = &["drafts", "brouillons"];
const INDESIRABLES: &[&str] = &[
    "spam",
    "junk",
    "junk e-mail",
    "courrier indésirable",
    "indésirables",
];
const CORBEILLE: &[&str] = &[
    "trash",
    "corbeille",
    "deleted",
    "deleted items",
    "éléments supprimés",
];
const ARCHIVES: &[&str] = &["archive", "archives"];
// Les INTÉGRALES : « Tous les messages » de Gmail contient TOUT — boîte
// de réception comprise. Servir ça tel quel dans Archives montrerait
// toute la boîte (défaut vu au terrain, 2026-08-12) : l'intégrale n'est
// une archive QUE privée des messages vivant dans une autre canonique.
const INTEGRALES: &[&str] = &["all mail", "tous les messages"];

/// Racine, ou exactement un niveau sous `[Gmail]` — rien de plus profond.
fn feuille_canonique(display: &str) -> Option<(bool, String)> {
    let segments: Vec<&str> = display.split('/').collect();
    match segments.as_slice() {
        [seul] => Some((false, seul.to_lowercase())),
        [prefixe, feuille] if prefixe.eq_ignore_ascii_case("[Gmail]") => {
            Some((true, feuille.to_lowercase()))
        }
        _ => None,
    }
}

/// La clause d'exclusion d'une intégrale : un message dont le
/// `message_id` vit AUSSI dans l'une des boîtes données n'est pas
/// archivé. Les identifiants viennent de NOTRE base (i64) : inclusion
/// littérale, l'index partiel `idx_envelopes_message` porte la sonde.
/// Un message sans `message_id` reste : on ne peut pas prouver qu'il
/// vit ailleurs.
fn clause_exclusion(exclure: &[i64]) -> String {
    if exclure.is_empty() {
        return String::new();
    }
    let liste = exclure
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        " AND (e.message_id IS NULL OR NOT EXISTS (
             SELECT 1 FROM envelopes x
             WHERE x.message_id = e.message_id AND x.mailbox_id IN ({liste})))"
    )
}

fn retenir(dossiers: &[(String, String)], motifs: &[&str]) -> Option<String> {
    let candidats: Vec<&(String, String)> = dossiers
        .iter()
        .filter(|(_, display)| {
            feuille_canonique(display)
                .is_some_and(|(_, feuille)| motifs.contains(&feuille.as_str()))
        })
        .collect();
    candidats
        .iter()
        .find(|(_, display)| feuille_canonique(display).is_some_and(|(gmail, _)| gmail))
        .or_else(|| candidats.first())
        .map(|(wire, _)| wire.clone())
}

impl Store {
    /// Résout les six dossiers canoniques d'un compte depuis le cache
    /// `folders` (rempli par la synchro) et `accounts.sent_mailbox`.
    pub fn canonical_folders(&self, account_id: i64) -> Result<CanonicalFolders, Error> {
        let envoyes: Option<String> = self
            .conn()
            .query_row(
                "SELECT sent_mailbox FROM accounts WHERE id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        let dossiers: Vec<(String, String)> = self
            .conn()
            .prepare(
                "SELECT wire, display FROM folders
                 WHERE account_id = ?1 AND selectable ORDER BY display",
            )?
            .query_map(params![account_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        // Un dossier d'archives PUR d'abord ; l'intégrale Gmail en repli,
        // marquée comme telle.
        let (archives, archives_integrale) = match retenir(&dossiers, ARCHIVES) {
            Some(pur) => (Some(pur), false),
            None => match retenir(&dossiers, INTEGRALES) {
                Some(integrale) => (Some(integrale), true),
                None => (None, false),
            },
        };
        Ok(CanonicalFolders {
            reception: RECEIVED_MAILBOX.to_string(),
            envoyes,
            brouillons: retenir(&dossiers, BROUILLONS),
            indesirables: retenir(&dossiers, INDESIRABLES),
            archives,
            archives_integrale,
            corbeille: retenir(&dossiers, CORBEILLE),
        })
    }

    /// Les `mailbox_id` des canoniques AUTRES que les archives — la
    /// clause d'exclusion d'une intégrale : un message présent dans
    /// l'une d'elles n'est pas archivé.
    pub fn canoniques_hors_archives(
        &self,
        account_id: i64,
        dossiers: &CanonicalFolders,
    ) -> Result<Vec<i64>, Error> {
        let mut ids = Vec::new();
        let noms = [
            Some(dossiers.reception.as_str()),
            dossiers.envoyes.as_deref(),
            dossiers.brouillons.as_deref(),
            dossiers.indesirables.as_deref(),
            dossiers.corbeille.as_deref(),
        ];
        for nom in noms.into_iter().flatten() {
            if let Some(state) = self.sync_state(account_id, nom)? {
                ids.push(state.mailbox_id);
            }
        }
        Ok(ids)
    }

    /// `(total, non lus)` des messages d'une boîte désignée par son nom
    /// réseau. Une boîte jamais synchronisée compte zéro — pas d'erreur :
    /// la nav s'affiche avant la première synchro. `exclure` : les
    /// boîtes dont la présence d'un même `message_id` disqualifie
    /// (l'intégrale privée des autres canoniques).
    fn compte_boite(
        &self,
        account_id: i64,
        name: Option<&str>,
        exclure: &[i64],
    ) -> Result<(u64, u64), Error> {
        let Some(name) = name else { return Ok((0, 0)) };
        let Some(state) = self.sync_state(account_id, name)? else {
            return Ok((0, 0));
        };
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0)
             FROM envelopes e WHERE e.mailbox_id = ?1{}",
            clause_exclusion(exclure)
        );
        let (total, non_lus): (i64, i64) =
            self.conn()
                .query_row(&sql, params![state.mailbox_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
        Ok((total as u64, non_lus as u64))
    }

    /// Les compteurs de nav d'un compte, sur ses dossiers résolus.
    pub fn nav_counts(
        &self,
        account_id: i64,
        dossiers: &CanonicalFolders,
    ) -> Result<NavCounts, Error> {
        let (reception_total, reception_non_lues): (i64, i64) = self.conn().query_row(
            "SELECT COUNT(*), COALESCE(SUM(unseen > 0), 0)
             FROM threads WHERE account_id = ?1 AND inbox_size > 0",
            params![account_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (envoyes, _) = self.compte_boite(account_id, dossiers.envoyes.as_deref(), &[])?;
        // PLAN-BROUILLONS (B-D1) : le dossier Brouillons montre les
        // brouillons LOCAUX — le compteur compte la même chose, jamais
        // la boîte IMAP miroir : un chiffre qui ne correspond pas à ce
        // que le clic ouvre serait un mensonge de nav.
        let brouillons: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM drafts WHERE account_id = ?1",
            params![account_id],
            |row| row.get(0),
        )?;
        let (indesirables_total, indesirables_non_lus) =
            self.compte_boite(account_id, dossiers.indesirables.as_deref(), &[])?;
        let exclusion_archives = if dossiers.archives_integrale {
            self.canoniques_hors_archives(account_id, dossiers)?
        } else {
            Vec::new()
        };
        let (archives, _) = self.compte_boite(
            account_id,
            dossiers.archives.as_deref(),
            &exclusion_archives,
        )?;
        let (corbeille, _) = self.compte_boite(account_id, dossiers.corbeille.as_deref(), &[])?;
        // Les échos locaux (PLAN-REACTIVITE E3) comptent avec les
        // enveloppes : le compteur dit ce que le clic ouvre — jamais
        // deux vérités entre la nav et la liste.
        Ok(NavCounts {
            reception_total: reception_total as u64,
            reception_non_lues: reception_non_lues as u64,
            envoyes: envoyes + self.compte_echos("envoyes", Some(account_id))?,
            brouillons: brouillons as u64,
            indesirables_total,
            indesirables_non_lus,
            archives: archives + self.compte_echos("archives", Some(account_id))?,
            corbeille: corbeille + self.compte_echos("corbeille", Some(account_id))?,
        })
    }

    /// Les deux seuls compteurs que la nav AFFICHE (A29 : la nav ne dit
    /// que le non-lu) — le chemin de la sonde à 10 s. Les huit
    /// compteurs de [`Store::nav_counts`] restent l'inventaire complet
    /// (tests d'invariants, écrans à venir) mais ne se paient plus à la
    /// cadence de la nav : le total des archives d'une intégrale (sonde
    /// `NOT EXISTS` par ligne) coûtait ~240 ms par compte et par sonde,
    /// posé devant chaque premier rendu (terrain 2026-08-20,
    /// PLAN-DEFILEMENT-PROFOND).
    pub fn nav_unread_counts(
        &self,
        account_id: i64,
        dossiers: &CanonicalFolders,
    ) -> Result<(u64, u64), Error> {
        let reception: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(unseen > 0), 0)
             FROM threads WHERE account_id = ?1 AND inbox_size > 0",
            params![account_id],
            |row| row.get(0),
        )?;
        let (_, indesirables) =
            self.compte_boite(account_id, dossiers.indesirables.as_deref(), &[])?;
        Ok((reception as u64, indesirables))
    }

    /// La boîte unifiée, bornée à un compte quand la nav filtre par
    /// « Boîte », aux non-lues quand l'onglet du prototype l'exige —
    /// même squelette de pagination que [`Store::unified_recent`].
    pub fn unified_recent_scoped(
        &self,
        account_id: Option<i64>,
        non_lues: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&unified_page_sql(account_id.is_some(), non_lues))?;
        let rows = match account_id {
            None => stmt
                .query_map(params![limit as i64, offset as i64], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![limit as i64, offset as i64, id], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Total de la boîte unifiée, sous les mêmes bornes que la page.
    pub fn unified_count_scoped(
        &self,
        account_id: Option<i64>,
        non_lues: bool,
    ) -> Result<u64, Error> {
        let filtre_compte = if account_id.is_some() {
            " AND account_id = ?1"
        } else {
            ""
        };
        let filtre_non_lues = if non_lues { " AND unseen > 0" } else { "" };
        // R4 (D5) : le total suit le FLOT — les épinglées, servies à
        // part en tête, n'y comptent pas ; sans cette exclusion, la
        // barre de défilement réserverait des lignes fantômes et le
        // total d'une page courte contredirait `category_total`.
        let sql = format!(
            "SELECT COUNT(*) FROM threads
              WHERE inbox_size > 0 AND id NOT IN ({PINNED_THREADS}){filtre_compte}{filtre_non_lues}"
        );
        let count: i64 = match account_id {
            None => self.conn().query_row(&sql, [], |row| row.get(0))?,
            Some(id) => self.conn().query_row(&sql, params![id], |row| row.get(0))?,
        };
        Ok(count as u64)
    }

    /// Les conversations ÉPINGLÉES de la portée (R4) — servies À PART,
    /// en tête de la page 0 de la Réception (D4 : Réception seule),
    /// jamais dans le flot paginé (D5). Mêmes colonnes et même queue de
    /// jointures/tri que la page ([`UNIFIED_JOIN_TAIL`] — les deux
    /// requêtes ne peuvent pas dériver), ordre de la liste (date
    /// décroissante, O3). Sans LIMIT : quelques épingles au plus, et la
    /// jointure sur `envelopes` écarte d'elle-même les épingles
    /// orphelines d'un message expurgé. LECTURE seule — les écritures
    /// (`toggle_pin`) vivent au stockage, `store.rs`.
    pub fn pinned_unified_scoped(
        &self,
        account_id: Option<i64>,
        non_lues: bool,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let filtre = if account_id.is_some() {
            " AND account_id = ?1"
        } else {
            ""
        };
        let non_lues_seulement = if non_lues { " AND unseen > 0" } else { "" };
        let sql = format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                     FROM threads
                    WHERE inbox_size > 0 AND id IN ({PINNED_THREADS}){filtre}{non_lues_seulement}) t{UNIFIED_JOIN_TAIL}"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = match account_id {
            None => stmt
                .query_map([], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![id], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// `(total, non lus)` cumulés des boîtes données — le total de la
    /// pagination d'une catégorie, et le héros non-lu des indésirables.
    ///
    /// `echos` (PLAN-REACTIVITE E3) : la catégorie et les comptes dont
    /// les échos locaux comptent AUSSI — le compteur et la liste disent
    /// la même chose, jamais deux vérités. Un écho est lu par nature :
    /// il n'entre jamais dans `non_lus`.
    pub fn category_totals(
        &self,
        mailbox_ids: &[i64],
        exclure: &[i64],
        echos: Option<(&str, &[i64])>,
    ) -> Result<(u64, u64), Error> {
        let mut total = 0u64;
        let mut non_lus = 0u64;
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0)
             FROM envelopes e WHERE e.mailbox_id = ?1{}",
            clause_exclusion(exclure)
        );
        for id in mailbox_ids {
            let (t, n): (i64, i64) = self
                .conn()
                .query_row(&sql, params![id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            total += t as u64;
            non_lus += n as u64;
        }
        if let Some((destination, comptes)) = echos {
            for compte in comptes {
                total += self.compte_echos(destination, Some(*compte))?;
            }
        }
        Ok((total, non_lus))
    }

    /// Une page d'une catégorie hors réception : les messages des boîtes
    /// données, du plus récent au plus ancien.
    ///
    /// La pagination suit la règle du gate P1 : chaque boîte fournit une
    /// tranche BORNÉE par son index `(mailbox_id, date_epoch DESC)`, la
    /// fusion et l'`OFFSET` s'appliquent sur ces tranches — jamais un tri
    /// de toute la boîte, et les jointures ne s'exécutent que sur les
    /// lignes retenues. Les lignes hors fil valent taille 1 et non-lu
    /// d'après `seen` (`LEFT JOIN threads`).
    pub fn category_page(
        &self,
        mailbox_ids: &[i64],
        non_lus: bool,
        exclure: &[i64],
        echos: Option<(&str, &[i64])>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        // Les échos (PLAN-REACTIVITE E3) n'entrent que hors onglet
        // « Non lus » (un écho est lu par nature) et si des comptes les
        // portent. Sans échos, le SQL est EXACTEMENT celui d'avant — le
        // chemin chaud ne paye rien.
        let echos = match echos {
            Some((destination, comptes)) if !non_lus && !comptes.is_empty() => {
                Some((destination, comptes))
            }
            _ => None,
        };
        if mailbox_ids.is_empty() && echos.is_none() {
            return Ok(Vec::new());
        }
        let n = mailbox_ids.len();
        let filtre = if non_lus { " AND NOT e.seen" } else { "" };
        // L'exclusion s'applique DANS chaque tranche : appliquée après,
        // la pagination compterait des lignes qu'elle ne sert pas.
        let exclusion = clause_exclusion(exclure);
        let borne_idx = n + 1;
        let limite_idx = n + 2;
        let decalage_idx = n + 3;
        let mut tranches: Vec<String> = (1..=n)
            .map(|i| {
                format!(
                    "SELECT * FROM (SELECT e.mailbox_id, e.uid, e.date_epoch FROM envelopes e
                      WHERE e.mailbox_id = ?{i}{filtre}{exclusion}
                      ORDER BY e.date_epoch DESC, e.uid DESC LIMIT ?{borne_idx})"
                )
            })
            .collect();
        if echos.is_some() {
            // La tranche des échos : identifiée par un mailbox_id NÉGATIF
            // (-id d'écho) — jamais confondue avec une vraie boîte, et la
            // jointure de sortie sait la reconnaître. Les comptes sont
            // NOS entiers : inclusion littérale, comme l'exclusion.
            let comptes = echos
                .as_ref()
                .map(|(_, comptes)| {
                    comptes
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            tranches.push(format!(
                "SELECT * FROM (SELECT -ec.id AS mailbox_id, 0 AS uid, ec.date_epoch
                  FROM echos ec
                  WHERE ec.destination = ?{dest} AND ec.account_id IN ({comptes})
                  ORDER BY ec.date_epoch DESC LIMIT ?{borne_idx})",
                dest = n + 4,
            ));
        }
        let sql = if echos.is_none() {
            format!(
                "{SELECT_UNIFIED}, COALESCE(t.size, 1),
                        COALESCE(t.unseen, CASE WHEN e.seen THEN 0 ELSE 1 END)
                 FROM (SELECT mailbox_id, uid FROM ({tranches})
                       ORDER BY date_epoch DESC, uid DESC, mailbox_id
                       LIMIT ?{limite_idx} OFFSET ?{decalage_idx}) page
                 JOIN envelopes e ON e.mailbox_id = page.mailbox_id AND e.uid = page.uid
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 JOIN accounts a ON a.id = m.account_id
                 LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                 LEFT JOIN threads t ON t.id = e.thread_id
                 ORDER BY e.date_epoch DESC, e.uid DESC, e.mailbox_id",
                tranches = tranches.join(" UNION ALL "),
            )
        } else {
            // Avec échos : la page fusionnée porte des lignes des DEUX
            // mondes — les enveloppes rejoignent leurs tables, les échos
            // la leur, et l'UNION ressort les mêmes colonnes que
            // SELECT_UNIFIED (uid 0, boîte synthétique `echo:<id>`, lu,
            // sans étoile ni fil). Le tri final rejoue la clé de la page.
            format!(
                "WITH page AS (SELECT mailbox_id, uid, date_epoch FROM ({tranches})
                       ORDER BY date_epoch DESC, uid DESC, mailbox_id
                       LIMIT ?{limite_idx} OFFSET ?{decalage_idx})
                 SELECT * FROM (
                   {SELECT_UNIFIED}, COALESCE(t.size, 1),
                        COALESCE(t.unseen, CASE WHEN e.seen THEN 0 ELSE 1 END),
                        page.date_epoch AS tri_date, page.uid AS tri_uid,
                        page.mailbox_id AS tri_boite
                   FROM page
                   JOIN envelopes e ON e.mailbox_id = page.mailbox_id AND e.uid = page.uid
                   JOIN mailboxes m ON m.id = e.mailbox_id
                   JOIN accounts a ON a.id = m.account_id
                   LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                   LEFT JOIN threads t ON t.id = e.thread_id
                   UNION ALL
                   SELECT a.id, a.email, 0, ec.subject, ec.sender, ec.sender_address,
                        ec.message_id, ec.date_epoch, 1, 0, ec.attachment_count,
                        NULL, NULL, 'echo:' || ec.id, ec.preview,
                        -- PLAN-RETOURS-5 : les VRAIS destinataires de
                        -- l'echo (copies du journal d'envoi ou de
                        -- l'enveloppe source) — jamais le slug de
                        -- destination (« A : envoyes », terrain
                        -- 2026-08-21).
                        ec.to_addrs, NULL, 1, 0,
                        page.date_epoch AS tri_date, page.uid AS tri_uid,
                        page.mailbox_id AS tri_boite
                   FROM page
                   JOIN echos ec ON page.mailbox_id = -ec.id AND page.mailbox_id < 0
                   JOIN accounts a ON a.id = ec.account_id
                 )
                 ORDER BY tri_date DESC, tri_uid DESC, tri_boite",
                tranches = tranches.join(" UNION ALL "),
            )
        };
        let borne = (offset + limit) as i64;
        let mut parametres: Vec<rusqlite::types::Value> = mailbox_ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id))
            .collect();
        parametres.push(rusqlite::types::Value::Integer(borne));
        parametres.push(rusqlite::types::Value::Integer(limit as i64));
        parametres.push(rusqlite::types::Value::Integer(offset as i64));
        if let Some((destination, _)) = echos {
            parametres.push(rusqlite::types::Value::Text(destination.to_string()));
        }
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(parametres), row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: u32, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn dossier(wire: &str) -> crate::Folder {
        crate::Folder {
            wire: wire.to_string(),
            display: wire.to_string(),
            selectable: true,
        }
    }

    #[test]
    fn la_migration_pst_ne_detourne_pas_les_canoniques() {
        // Le décor du terrain : canoniques [Gmail], homonymes racine, et
        // une migration PST pleine de segments « Archive » en profondeur.
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        store
            .set_thread_scope(account, Some("[Gmail]/Messages envoyes"))
            .unwrap();
        store
            .replace_folders(
                account,
                &[
                    dossier("INBOX"),
                    dossier("Brouillons"),
                    dossier("[Gmail]/Brouillons"),
                    dossier("[Gmail]/Spam"),
                    dossier("Corbeille"),
                    dossier("[Gmail]/Corbeille"),
                    dossier("[Gmail]/Tous les messages"),
                    dossier("[Gmail]/Corbeille/x@y.fr/Archive"),
                    dossier("[Gmail]/Corbeille/x@y.fr/Archive/Sport"),
                    dossier("pst/Archive/Sante"),
                    dossier("pst/Trash"),
                ],
            )
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        assert_eq!(canon.reception, "INBOX");
        assert_eq!(canon.envoyes.as_deref(), Some("[Gmail]/Messages envoyes"));
        assert_eq!(canon.brouillons.as_deref(), Some("[Gmail]/Brouillons"));
        assert_eq!(canon.indesirables.as_deref(), Some("[Gmail]/Spam"));
        assert_eq!(canon.archives.as_deref(), Some("[Gmail]/Tous les messages"));
        assert_eq!(canon.corbeille.as_deref(), Some("[Gmail]/Corbeille"));
    }

    #[test]
    fn les_compteurs_suivent_les_dossiers_resolus_et_les_boites_muettes_valent_zero() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "lu", 100, true),
                    envelope(2, "non lu", 200, false),
                ],
            )
            .unwrap();
        let archives = store.create_mailbox(account, "Archives", 1).unwrap();
        store
            .upsert_envelopes(
                archives,
                &[
                    envelope(1, "archive lue", 300, true),
                    envelope(2, "archive non lue", 400, false),
                    envelope(3, "archive lue aussi", 500, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[dossier("INBOX"), dossier("Archives")])
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        let counts = store.nav_counts(account, &canon).unwrap();
        assert_eq!(counts.reception_total, 2);
        assert_eq!(counts.reception_non_lues, 1);
        assert_eq!(counts.archives, 3);
        // Brouillons : aucun brouillon local -> zéro, jamais une erreur.
        assert_eq!(counts.brouillons, 0);
    }

    /// PLAN-DEFILEMENT-PROFOND (terrain 2026-08-20) : la sonde de nav
    /// ne paie plus que les deux compteurs AFFICHÉS (A29 : la nav ne
    /// dit que le non-lu) — et dit exactement ce que l'inventaire
    /// complet dirait. Parité verrouillée : si `nav_counts` évolue,
    /// la version légère doit suivre ou ce test crie.
    #[test]
    fn les_compteurs_legers_de_nav_disent_ce_que_l_inventaire_dit() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "lu", 100, true),
                    envelope(2, "non lu", 200, false),
                ],
            )
            .unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();
        store
            .upsert_envelopes(
                spam,
                &[
                    envelope(1, "indésirable", 300, false),
                    envelope(2, "indésirable lu", 400, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[dossier("INBOX"), dossier("Spam")])
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        let complet = store.nav_counts(account, &canon).unwrap();
        let (reception, indesirables) = store.nav_unread_counts(account, &canon).unwrap();
        assert_eq!(reception, complet.reception_non_lues);
        assert_eq!(indesirables, complet.indesirables_non_lus);
        assert_eq!(reception, 1);
        assert_eq!(indesirables, 1);
    }

    /// B-D1 (PLAN-BROUILLONS) : le compteur compte la table `drafts` du
    /// compte — la boîte IMAP miroir, même pleine, ne compte pas, car ce
    /// n'est pas elle que le clic ouvre.
    #[test]
    fn le_compteur_de_brouillons_compte_les_brouillons_locaux() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let autre = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let miroir = store.create_mailbox(account, "Brouillons", 1).unwrap();
        store
            .upsert_envelopes(
                miroir,
                &[
                    envelope(1, "copie poussée", 100, true),
                    envelope(2, "copie poussée aussi", 200, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[dossier("INBOX"), dossier("Brouillons")])
            .unwrap();
        store
            .save_draft(
                account,
                None,
                None,
                crate::DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "local",
                    body: "texte",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        store
            .save_draft(
                autre,
                None,
                None,
                crate::DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "du voisin",
                    body: "texte",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        let canon = store.canonical_folders(account).unwrap();
        let counts = store.nav_counts(account, &canon).unwrap();
        assert_eq!(counts.brouillons, 1, "le local compte, le miroir non");
    }

    /// PLAN-RETOURS-5 (terrain 2026-08-21) : la ligne d'écho d'envoi
    /// dit ses VRAIS destinataires — jamais le slug de catégorie
    /// (« À : envoyes » à l'écran pendant la fenêtre de réconciliation).
    #[test]
    fn la_ligne_d_echo_d_envoi_dit_ses_destinataires() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr, c@d.fr",
            "",
            "",
            "objet",
            "corps",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.echo_envoi(id).unwrap());

        let comptes = [account];
        let page = store
            .category_page(&[], false, &[], Some(("envoyes", &comptes)), 0, 10)
            .unwrap();
        assert_eq!(page.len(), 1);
        assert!(page[0].mailbox.starts_with("echo:"), "{}", page[0].mailbox);
        assert_eq!(
            page[0].envelope.to_addrs,
            vec!["a@b.fr".to_string(), "c@d.fr".to_string()],
            "les destinataires réels, jamais « envoyes »"
        );
    }

    /// E3 (PLAN-REACTIVITE) : les échos entrent dans la page À LEUR
    /// PLACE de date — un vieux message supprimé ne saute pas en tête de
    /// Corbeille — et le total les compte (jamais deux vérités). La
    /// ligne d'écho porte sa boîte synthétique `echo:<id>`, son aperçu,
    /// et l'onglet « Non lus » les ignore (un écho est lu par nature).
    #[test]
    fn la_page_de_categorie_sert_les_echos_a_leur_date() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let corbeille = store.create_mailbox(account, "Trash", 1).unwrap();
        store
            .replace_folders(
                account,
                &[crate::Folder {
                    wire: "Trash".into(),
                    display: "Trash".into(),
                    selectable: true,
                }],
            )
            .unwrap();
        store
            .upsert_envelopes(
                corbeille,
                &[
                    envelope(1, "vieux", 100, true),
                    envelope(2, "recent", 300, true),
                ],
            )
            .unwrap();
        // Le message supprimé date d'ENTRE les deux : l'écho doit se
        // glisser au milieu, pas en tête.
        store
            .upsert_envelopes(inbox, &[envelope(7, "milieu", 200, true)])
            .unwrap();
        store
            .save_body(inbox, 7, "<p>corps du milieu</p>", &[])
            .unwrap();
        store
            .geste_avec_echo(inbox, 7, crate::Action::Delete, Some("corbeille"))
            .unwrap();

        let comptes = [account];
        let page = store
            .category_page(
                &[corbeille],
                false,
                &[],
                Some(("corbeille", &comptes)),
                0,
                10,
            )
            .unwrap();
        let sujets: Vec<&str> = page
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(sujets, ["recent", "milieu", "vieux"]);
        assert!(page[1].mailbox.starts_with("echo:"), "{}", page[1].mailbox);
        assert_eq!(page[1].preview.as_deref(), Some("corps du milieu"));
        assert_eq!(page[1].thread_unseen, 0, "un écho est lu");
        // La pagination traverse l'écho sans perdre ni dupliquer.
        let coupe = store
            .category_page(
                &[corbeille],
                false,
                &[],
                Some(("corbeille", &comptes)),
                1,
                1,
            )
            .unwrap();
        assert_eq!(coupe[0].envelope.subject.as_deref(), Some("milieu"));
        // Le total dit la même chose que la page.
        let (total, _) = store
            .category_totals(&[corbeille], &[], Some(("corbeille", &comptes)))
            .unwrap();
        assert_eq!(total, 3);
        // « Non lus » : les échos n'y entrent pas.
        let non_lus = store
            .category_page(
                &[corbeille],
                true,
                &[],
                Some(("corbeille", &comptes)),
                0,
                10,
            )
            .unwrap();
        assert!(non_lus.is_empty());
    }

    #[test]
    fn la_page_de_categorie_fusionne_les_boites_du_plus_recent_au_plus_ancien() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let gauche = store.create_mailbox(account, "Archives", 1).unwrap();
        let droite = store.create_mailbox(account, "pst/Archives", 1).unwrap();
        store
            .upsert_envelopes(
                gauche,
                &[envelope(1, "a1", 100, true), envelope(2, "a3", 300, false)],
            )
            .unwrap();
        store
            .upsert_envelopes(
                droite,
                &[envelope(1, "b2", 200, true), envelope(2, "b4", 400, true)],
            )
            .unwrap();
        let page = store
            .category_page(&[gauche, droite], false, &[], None, 0, 3)
            .unwrap();
        let sujets: Vec<&str> = page
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(sujets, ["b4", "a3", "b2"]);
        // Hors fil : taille 1, non-lu d'après `seen`.
        assert_eq!(page[1].thread_size, 1);
        assert_eq!(page[1].thread_unseen, 1);
        assert_eq!(page[0].thread_unseen, 0);
        // L'OFFSET traverse la fusion sans perdre ni dupliquer.
        let suite = store
            .category_page(&[gauche, droite], false, &[], None, 3, 3)
            .unwrap();
        let sujets: Vec<&str> = suite
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(sujets, ["a1"]);
        // L'onglet « Non lus » filtre côté coeur, dans les tranches mêmes.
        let non_lus = store
            .category_page(&[gauche, droite], true, &[], None, 0, 10)
            .unwrap();
        let sujets: Vec<&str> = non_lus
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(sujets, ["a3"]);
        // Aperçu et COMPTE de pièces : posés à l'écriture du corps.
        store
            .save_body(
                gauche,
                2,
                "<p>Aperçu de a3</p>",
                &[
                    crate::Attachment {
                        index: 0,
                        name: "un.pdf".into(),
                        mime: "application/pdf".into(),
                        size: 10,
                    },
                    crate::Attachment {
                        index: 1,
                        name: "deux.pdf".into(),
                        mime: "application/pdf".into(),
                        size: 10,
                    },
                ],
            )
            .unwrap();
        let page = store
            .category_page(&[gauche, droite], false, &[], None, 0, 10)
            .unwrap();
        let a3 = page
            .iter()
            .find(|row| row.envelope.subject.as_deref() == Some("a3"))
            .unwrap();
        assert_eq!(a3.preview.as_deref(), Some("Aperçu de a3"));
        assert_eq!(a3.attachment_count, 2);
        assert!(a3.has_attachment);
    }

    #[test]
    fn l_integrale_gmail_privee_des_canoniques_fait_les_archives() {
        // Le défaut du terrain (2026-08-12) : « Tous les messages »
        // contient TOUT — la catégorie Archives montrait la boîte
        // entière. L'intégrale doit se priver des autres canoniques.
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        store
            .replace_folders(
                account,
                &[dossier("INBOX"), dossier("[Gmail]/Tous les messages")],
            )
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let integrale = store
            .create_mailbox(account, "[Gmail]/Tous les messages", 1)
            .unwrap();
        // <m1> vit dans INBOX ET l'intégrale (reçu, non archivé) ;
        // <m2> seulement dans l'intégrale (réellement archivé).
        store
            .upsert_envelopes(inbox, &[envelope(1, "recu", 100, true)])
            .unwrap();
        store
            .upsert_envelopes(
                integrale,
                &[
                    envelope(1, "recu", 100, true),
                    envelope(2, "archive", 200, true),
                ],
            )
            .unwrap();

        let canon = store.canonical_folders(account).unwrap();
        assert!(canon.archives_integrale, "l'intégrale est marquée");
        assert_eq!(canon.archives.as_deref(), Some("[Gmail]/Tous les messages"));
        let compteurs = store.nav_counts(account, &canon).unwrap();
        assert_eq!(
            compteurs.archives, 1,
            "seul le message hors des autres canoniques est archivé"
        );
        let exclure = store.canoniques_hors_archives(account, &canon).unwrap();
        assert_eq!(exclure, vec![inbox]);
        let page = store
            .category_page(&[integrale], false, &exclure, None, 0, 10)
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].envelope.subject.as_deref(), Some("archive"));
        let (total, _) = store.category_totals(&[integrale], &exclure, None).unwrap();
        assert_eq!(total, 1);
        // Un dossier d'archives PUR n'est jamais privé de rien.
        store
            .replace_folders(
                account,
                &[
                    dossier("INBOX"),
                    dossier("Archives"),
                    dossier("[Gmail]/Tous les messages"),
                ],
            )
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        assert!(!canon.archives_integrale);
        assert_eq!(canon.archives.as_deref(), Some("Archives"));
    }

    #[test]
    fn le_rattrapage_d_apercu_solde_les_corps_anterieurs() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "ancien", 100, true)])
            .unwrap();
        // Un corps écrit À L'ANCIENNE : la colonne `preview` n'existait pas.
        store
            .conn()
            .execute(
                "INSERT INTO bodies (mailbox_id, uid, html, scanned, preview)
                 VALUES (?1, 1, '<p>Vieux corps</p>', 1, NULL)",
                params![inbox],
            )
            .unwrap();
        assert_eq!(
            store.preview_catchup(10).unwrap(),
            0,
            "plus de retardataires"
        );
        let page = store
            .category_page(&[inbox], false, &[], None, 0, 10)
            .unwrap();
        assert_eq!(page[0].preview.as_deref(), Some("Vieux corps"));
    }

    #[test]
    fn la_boite_unifiee_se_borne_a_un_compte() {
        let mut store = Store::open_in_memory().unwrap();
        let premier = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(premier, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox_a, &[envelope(1, "chez a", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "chez b", 200, false),
                    envelope(2, "chez b aussi", 300, true),
                ],
            )
            .unwrap();

        let tout = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert_eq!(tout.len(), 3);
        let seul_b = store
            .unified_recent_scoped(Some(second), false, 0, 10)
            .unwrap();
        assert_eq!(seul_b.len(), 2);
        assert!(seul_b.iter().all(|row| row.account_id == second));
        assert_eq!(store.unified_count_scoped(Some(premier), false).unwrap(), 1);
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 3);
        // « chez b aussi » est lue : l'onglet non-lus n'en garde que deux.
        assert_eq!(store.unified_count_scoped(None, true).unwrap(), 2);
        assert_eq!(
            store
                .unified_recent_scoped(None, true, 0, 10)
                .unwrap()
                .len(),
            2
        );
    }
}
