//! Brouillons locaux : plus jamais de texte perdu.
//!
//! Un brouillon est du texte BRUT, pas encore validé — c'est tout son
//! intérêt : une adresse à moitié tapée se conserve telle quelle. La
//! validation stricte ([`crate::compose`]) n'intervient qu'à l'envoi.
//! Même philosophie que la boîte d'envoi : journaliser d'abord,
//! l'utilisateur décide ensuite (reprendre, envoyer ou jeter).
//!
//! Synchronisation vers Gmail (poussée seule, v1) : chaque brouillon
//! local est reflété dans le dossier Brouillons du serveur. Invariants :
//! - on ne supprime à distance que des UIDs que NOUS avons enregistrés ;
//!   UIDVALIDITY changée → on abandonne les repères (un doublon de
//!   brouillon est acceptable, supprimer le mauvais message jamais) ;
//! - le repère « propre » est une photo d'horodatage : une édition
//!   survenue PENDANT la poussée laisse le brouillon à pousser.
//!
//! L'édition des brouillons créés ailleurs (tirage) : Phase 3.

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// Un brouillon tel que laissé par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDraft {
    pub id: i64,
    /// Le compte qui l'enverra (et dont le dossier Brouillons le reflète).
    pub account_id: i64,
    /// Champ « À » brut, non validé (peut être vide ou incomplet).
    pub to_raw: String,
    pub subject: String,
    pub body: String,
    /// UID du message auquel ce brouillon répond, s'il y en a un.
    pub reply_to_uid: Option<Uid>,
    /// Millisecondes — l'ordre « plus récent d'abord » doit rester vrai
    /// entre deux sauvegardes rapprochées.
    pub updated_epoch: i64,
    /// UID de la dernière copie poussée dans le dossier Brouillons Gmail.
    pub remote_uid: Option<Uid>,
    /// Photo d'`updated_epoch` au moment de la dernière poussée réussie.
    pub pushed_epoch: Option<i64>,
}

impl Store {
    /// Enregistre (`id: None`) ou met à jour un brouillon ; retourne son id.
    ///
    /// Un id périmé (brouillon supprimé entre-temps par une autre vue)
    /// ré-insère au lieu de perdre silencieusement le texte — c'est un
    /// filet, il ne doit jamais avoir de maille manquante.
    pub fn save_draft(
        &self,
        account_id: i64,
        id: Option<i64>,
        to_raw: &str,
        subject: &str,
        body: &str,
        reply_to_uid: Option<Uid>,
    ) -> Result<i64, Error> {
        let now = Utc::now().timestamp_millis();
        match id {
            Some(id) => {
                // MAX(…, +1) : l'horodatage avance STRICTEMENT à chaque
                // vraie modification — une édition dans la même milliseconde
                // que la photo d'une poussée resterait sinon invisible
                // (maille du filet, attrapée par test). Et le WHERE : une
                // sauvegarde au contenu IDENTIQUE ne touche à rien, sinon
                // chaque fermeture re-pousserait une copie identique vers
                // Gmail (churn observé en validation terrain).
                self.conn().execute(
                    "INSERT INTO drafts (id, account_id, to_raw, subject, body, reply_to_uid, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(id) DO UPDATE SET
                       to_raw = excluded.to_raw,
                       subject = excluded.subject,
                       body = excluded.body,
                       reply_to_uid = excluded.reply_to_uid,
                       updated_epoch = MAX(excluded.updated_epoch, drafts.updated_epoch + 1)
                     WHERE drafts.to_raw IS NOT excluded.to_raw
                        OR drafts.subject IS NOT excluded.subject
                        OR drafts.body IS NOT excluded.body
                        OR drafts.reply_to_uid IS NOT excluded.reply_to_uid",
                    params![id, account_id, to_raw, subject, body, reply_to_uid, now],
                )?;
                Ok(id)
            }
            None => {
                self.conn().execute(
                    "INSERT INTO drafts (account_id, to_raw, subject, body, reply_to_uid, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![account_id, to_raw, subject, body, reply_to_uid, now],
                )?;
                Ok(self.conn().last_insert_rowid())
            }
        }
    }

    /// Les brouillons, les plus récents d'abord.
    pub fn drafts(&self) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT} ORDER BY updated_epoch DESC, id DESC"
        ))?;
        let rows = stmt
            .query_map([], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Les brouillons d'UN compte dont son dossier Brouillons n'a pas
    /// (ou plus) la dernière version, dans l'ordre de création.
    pub fn drafts_to_push(&self, account_id: i64) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT}
             WHERE account_id = ?1
               AND (pushed_epoch IS NULL OR pushed_epoch < updated_epoch)
             ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([account_id], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Consigne une poussée réussie : l'ancienne copie distante (si
    /// différente) part en tombstone, la photo d'horodatage devient le
    /// repère « propre ». Une édition survenue pendant la poussée garde
    /// le brouillon à pousser — le filet ne saute jamais.
    pub fn record_draft_pushed(
        &self,
        id: i64,
        remote_uid: Option<Uid>,
        pushed_epoch: i64,
    ) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid)
             SELECT account_id, remote_uid FROM drafts
             WHERE id = ?1 AND remote_uid IS NOT NULL AND remote_uid IS NOT ?2",
            params![id, remote_uid],
        )?;
        tx.execute(
            "UPDATE drafts SET remote_uid = ?2, pushed_epoch = ?3 WHERE id = ?1",
            params![id, remote_uid, pushed_epoch],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Jette un brouillon — décision explicite de l'utilisateur (ou
    /// brouillon devenu envoi : il a rempli son office). Sa copie
    /// distante éventuelle part en tombstone, purgée au prochain cycle.
    pub fn delete_draft(&self, id: i64) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid)
             SELECT account_id, remote_uid FROM drafts
             WHERE id = ?1 AND remote_uid IS NOT NULL",
            [id],
        )?;
        tx.execute("DELETE FROM drafts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Copies distantes d'UN compte à purger (supprimées ou remplacées) —
    /// chaque tombstone se purge via la connexion de SON compte.
    pub fn draft_tombstones(&self, account_id: i64) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT remote_uid FROM draft_tombstones
             WHERE account_id = ?1 ORDER BY remote_uid",
        )?;
        let rows = stmt
            .query_map([account_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_draft_tombstone(&self, account_id: i64, remote_uid: Uid) -> Result<(), Error> {
        self.conn().execute(
            "DELETE FROM draft_tombstones WHERE account_id = ?1 AND remote_uid = ?2",
            params![account_id, remote_uid],
        )?;
        Ok(())
    }

    /// Aligne l'état distant d'UN compte sur l'UIDVALIDITY observée de son
    /// dossier Brouillons. Si elle a changé, les repères de CE compte sont
    /// abandonnés : on re-poussera (doublon possible — acceptable ;
    /// supprimer le mauvais UID, jamais). Retourne `true` si
    /// réinitialisation. Les autres comptes ne sont pas touchés.
    pub fn align_drafts_uidvalidity(
        &self,
        account_id: i64,
        uid_validity: u32,
    ) -> Result<bool, Error> {
        let known: Option<u32> = self
            .conn()
            .query_row(
                "SELECT uid_validity FROM drafts_remote WHERE account_id = ?1",
                [account_id],
                |row| row.get(0),
            )
            .optional()?;
        if known == Some(uid_validity) {
            return Ok(false);
        }
        let tx = self.conn().unchecked_transaction()?;
        let reset = known.is_some();
        if reset {
            tx.execute(
                "UPDATE drafts SET remote_uid = NULL, pushed_epoch = NULL
                 WHERE account_id = ?1",
                [account_id],
            )?;
            tx.execute(
                "DELETE FROM draft_tombstones WHERE account_id = ?1",
                [account_id],
            )?;
        }
        tx.execute(
            "INSERT INTO drafts_remote (account_id, uid_validity) VALUES (?1, ?2)
             ON CONFLICT(account_id) DO UPDATE SET uid_validity = excluded.uid_validity",
            params![account_id, uid_validity],
        )?;
        tx.commit()?;
        Ok(reset)
    }
}

const DRAFT_SELECT: &str = "SELECT id, account_id, to_raw, subject, body, reply_to_uid,
        updated_epoch, remote_uid, pushed_epoch
 FROM drafts";

fn row_to_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedDraft> {
    Ok(SavedDraft {
        id: row.get(0)?,
        account_id: row.get(1)?,
        to_raw: row.get(2)?,
        subject: row.get(3)?,
        body: row.get(4)?,
        reply_to_uid: row.get(5)?,
        updated_epoch: row.get(6)?,
        remote_uid: row.get(7)?,
        pushed_epoch: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    #[test]
    fn saves_raw_unvalidated_content_and_roundtrips() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                "adresse-incomp",
                "Sujet",
                "corps\nsur deux lignes",
                Some(42),
            )
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        let draft = &drafts[0];
        assert_eq!(draft.id, id);
        assert_eq!(draft.to_raw, "adresse-incomp", "le brut se garde tel quel");
        assert_eq!(draft.subject, "Sujet");
        assert_eq!(draft.body, "corps\nsur deux lignes");
        assert_eq!(draft.reply_to_uid, Some(42));
    }

    #[test]
    fn save_with_id_updates_in_place() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "", "v1", "texte", None)
            .unwrap();
        let same = store
            .save_draft(account, Some(id), "a@b.fr", "v2", "texte enrichi", None)
            .unwrap();

        assert_eq!(same, id);
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1, "mise à jour, pas duplication");
        assert_eq!(drafts[0].subject, "v2");
        assert_eq!(drafts[0].to_raw, "a@b.fr");
    }

    /// Le filet ne doit jamais avoir de maille manquante : un id périmé
    /// (brouillon supprimé entre-temps) ré-insère au lieu de perdre.
    #[test]
    fn save_with_stale_id_still_persists_the_text() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "", "s", "précieux", None)
            .unwrap();
        store.delete_draft(id).unwrap();

        store
            .save_draft(account, Some(id), "", "s", "précieux", None)
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].body, "précieux");
    }

    #[test]
    fn drafts_lists_most_recent_first() {
        let (store, account) = store();
        store
            .save_draft(account, None, "", "premier", "a", None)
            .unwrap();
        store
            .save_draft(account, None, "", "second", "b", None)
            .unwrap();

        let drafts = store.drafts().unwrap();
        let subjects: Vec<&str> = drafts.iter().map(|draft| draft.subject.as_str()).collect();
        assert_eq!(subjects, vec!["second", "premier"]);
    }

    #[test]
    fn delete_draft_removes_it() {
        let (store, account) = store();
        let id = store.save_draft(account, None, "", "s", "b", None).unwrap();
        store.delete_draft(id).unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }

    #[test]
    fn fresh_and_edited_drafts_are_to_push_until_recorded() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "", "s", "v1", None)
            .unwrap();
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "neuf = à pousser"
        );

        let draft = &store.drafts_to_push(account).unwrap()[0];
        store
            .record_draft_pushed(id, Some(101), draft.updated_epoch)
            .unwrap();
        assert!(
            store.drafts_to_push(account).unwrap().is_empty(),
            "poussé = propre"
        );

        store
            .save_draft(account, Some(id), "", "s", "v2", None)
            .unwrap();
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "édité = de nouveau à pousser"
        );
    }

    /// Une sauvegarde au contenu identique ne marque rien à pousser :
    /// sinon chaque fermeture de composition re-pousserait une copie
    /// octet pour octet identique vers Gmail (churn observé au terrain).
    #[test]
    fn identical_resave_does_not_mark_dirty_again() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "a@b.fr", "s", "texte", Some(1))
            .unwrap();
        let epoch = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store.record_draft_pushed(id, Some(101), epoch).unwrap();

        store
            .save_draft(account, Some(id), "a@b.fr", "s", "texte", Some(1))
            .unwrap();

        assert!(
            store.drafts_to_push(account).unwrap().is_empty(),
            "contenu identique : rien à re-pousser"
        );
    }

    /// L'invariant anti-perte : une édition PENDANT la poussée laisse le
    /// brouillon à pousser — le repère est une photo, pas un drapeau.
    #[test]
    fn edit_during_push_stays_dirty() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "", "s", "v1", None)
            .unwrap();
        let snapshot = store.drafts_to_push(account).unwrap()[0].updated_epoch;

        // L'utilisateur édite pendant que la poussée est en vol — même
        // dans la même milliseconde, l'horodatage strictement croissant
        // rend l'édition détectable…
        store
            .save_draft(account, Some(id), "", "s", "v2 éditée en vol", None)
            .unwrap();
        // …puis la poussée (de v1) aboutit et se consigne avec SA photo.
        store.record_draft_pushed(id, Some(101), snapshot).unwrap();

        let to_push = store.drafts_to_push(account).unwrap();
        assert_eq!(to_push.len(), 1, "v2 doit repartir au prochain cycle");
        assert_eq!(to_push[0].body, "v2 éditée en vol");
    }

    #[test]
    fn replacement_tombstones_the_previous_remote_copy() {
        let (store, account) = store();
        let id = store
            .save_draft(account, None, "", "s", "v1", None)
            .unwrap();
        store.record_draft_pushed(id, Some(101), 1).unwrap();

        store.record_draft_pushed(id, Some(202), 2).unwrap();

        assert_eq!(store.draft_tombstones(account).unwrap(), vec![101]);
        store.clear_draft_tombstone(account, 101).unwrap();
        assert!(store.draft_tombstones(account).unwrap().is_empty());
    }

    #[test]
    fn delete_tombstones_the_remote_copy_but_only_if_pushed() {
        let (store, account) = store();
        let pushed = store
            .save_draft(account, None, "", "poussé", "b", None)
            .unwrap();
        store.record_draft_pushed(pushed, Some(303), 1).unwrap();
        let local_only = store
            .save_draft(account, None, "", "local", "b", None)
            .unwrap();

        store.delete_draft(pushed).unwrap();
        store.delete_draft(local_only).unwrap();

        assert_eq!(
            store.draft_tombstones(account).unwrap(),
            vec![303],
            "jamais de tombstone sans copie distante enregistrée"
        );
    }

    /// La garde UIDVALIDITY est PAR COMPTE : réinitialiser les repères
    /// de A ne touche ni les repères ni les tombstones de B.
    #[test]
    fn align_resets_only_the_given_account() {
        let (store, account) = store();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        let draft_a = store.save_draft(account, None, "", "a", "x", None).unwrap();
        let draft_b = store.save_draft(other, None, "", "b", "y", None).unwrap();
        let epoch_a = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store
            .record_draft_pushed(draft_a, Some(11), epoch_a)
            .unwrap();
        let epoch_b = store.drafts_to_push(other).unwrap()[0].updated_epoch;
        store
            .record_draft_pushed(draft_b, Some(22), epoch_b)
            .unwrap();
        store.align_drafts_uidvalidity(account, 5).unwrap();
        store.align_drafts_uidvalidity(other, 7).unwrap();

        assert!(
            store.align_drafts_uidvalidity(account, 6).unwrap(),
            "reset de A"
        );

        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "A doit tout re-pousser"
        );
        assert!(
            store.drafts_to_push(other).unwrap().is_empty(),
            "B n'est pas concerné"
        );
        let drafts = store.drafts().unwrap();
        let of_b = drafts.iter().find(|draft| draft.id == draft_b).unwrap();
        assert_eq!(of_b.remote_uid, Some(22), "les repères de B survivent");
    }

    /// UIDVALIDITY changée : on abandonne tous les repères — un doublon
    /// est acceptable, supprimer le mauvais UID jamais.
    #[test]
    fn uidvalidity_change_resets_remote_state() {
        let (store, account) = store();
        assert!(
            !store.align_drafts_uidvalidity(account, 7).unwrap(),
            "première vue"
        );
        let id = store.save_draft(account, None, "", "s", "b", None).unwrap();
        store.record_draft_pushed(id, Some(404), 1).unwrap();
        store.record_draft_pushed(id, Some(505), 2).unwrap(); // 404 en tombstone

        assert!(
            !store.align_drafts_uidvalidity(account, 7).unwrap(),
            "inchangée"
        );
        assert!(
            store.align_drafts_uidvalidity(account, 8).unwrap(),
            "changée : reset"
        );

        assert!(store.draft_tombstones(account).unwrap().is_empty());
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].remote_uid, None);
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "tout est à re-pousser"
        );
    }
}
