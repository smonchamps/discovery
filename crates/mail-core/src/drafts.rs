//! Brouillons locaux : plus jamais de texte perdu.
//!
//! Un brouillon est du texte BRUT, pas encore validé — c'est tout son
//! intérêt : une adresse à moitié tapée se conserve telle quelle. La
//! validation stricte ([`crate::compose`]) n'intervient qu'à l'envoi.
//! Même philosophie que la boîte d'envoi : journaliser d'abord,
//! l'utilisateur décide ensuite (reprendre, envoyer ou jeter).
//!
//! Synchronisation vers le dossier Brouillons de Gmail : incrément
//! suivant — le filet local est la fondation, pas le luxe.

use chrono::Utc;
use rusqlite::params;

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// Un brouillon tel que laissé par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDraft {
    pub id: i64,
    /// Champ « À » brut, non validé (peut être vide ou incomplet).
    pub to_raw: String,
    pub subject: String,
    pub body: String,
    /// UID du message auquel ce brouillon répond, s'il y en a un.
    pub reply_to_uid: Option<Uid>,
    /// Millisecondes — l'ordre « plus récent d'abord » doit rester vrai
    /// entre deux sauvegardes rapprochées.
    pub updated_epoch: i64,
}

impl Store {
    /// Enregistre (`id: None`) ou met à jour un brouillon ; retourne son id.
    ///
    /// Un id périmé (brouillon supprimé entre-temps par une autre vue)
    /// ré-insère au lieu de perdre silencieusement le texte — c'est un
    /// filet, il ne doit jamais avoir de maille manquante.
    pub fn save_draft(
        &self,
        id: Option<i64>,
        to_raw: &str,
        subject: &str,
        body: &str,
        reply_to_uid: Option<Uid>,
    ) -> Result<i64, Error> {
        let now = Utc::now().timestamp_millis();
        match id {
            Some(id) => {
                self.conn().execute(
                    "INSERT INTO drafts (id, to_raw, subject, body, reply_to_uid, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(id) DO UPDATE SET
                       to_raw = excluded.to_raw,
                       subject = excluded.subject,
                       body = excluded.body,
                       reply_to_uid = excluded.reply_to_uid,
                       updated_epoch = excluded.updated_epoch",
                    params![id, to_raw, subject, body, reply_to_uid, now],
                )?;
                Ok(id)
            }
            None => {
                self.conn().execute(
                    "INSERT INTO drafts (to_raw, subject, body, reply_to_uid, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![to_raw, subject, body, reply_to_uid, now],
                )?;
                Ok(self.conn().last_insert_rowid())
            }
        }
    }

    /// Les brouillons, les plus récents d'abord.
    pub fn drafts(&self) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT id, to_raw, subject, body, reply_to_uid, updated_epoch
             FROM drafts ORDER BY updated_epoch DESC, id DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SavedDraft {
                    id: row.get(0)?,
                    to_raw: row.get(1)?,
                    subject: row.get(2)?,
                    body: row.get(3)?,
                    reply_to_uid: row.get(4)?,
                    updated_epoch: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Jette un brouillon — décision explicite de l'utilisateur
    /// (ou brouillon devenu envoi : il a rempli son office).
    pub fn delete_draft(&self, id: i64) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM drafts WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn saves_raw_unvalidated_content_and_roundtrips() {
        let store = store();
        let id = store
            .save_draft(
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
        let store = store();
        let id = store.save_draft(None, "", "v1", "texte", None).unwrap();
        let same = store
            .save_draft(Some(id), "a@b.fr", "v2", "texte enrichi", None)
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
        let store = store();
        let id = store.save_draft(None, "", "s", "précieux", None).unwrap();
        store.delete_draft(id).unwrap();

        store
            .save_draft(Some(id), "", "s", "précieux", None)
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].body, "précieux");
    }

    #[test]
    fn drafts_lists_most_recent_first() {
        let store = store();
        store.save_draft(None, "", "premier", "a", None).unwrap();
        store.save_draft(None, "", "second", "b", None).unwrap();

        let drafts = store.drafts().unwrap();
        let subjects: Vec<&str> = drafts.iter().map(|draft| draft.subject.as_str()).collect();
        assert_eq!(subjects, vec!["second", "premier"]);
    }

    #[test]
    fn delete_draft_removes_it() {
        let store = store();
        let id = store.save_draft(None, "", "s", "b", None).unwrap();
        store.delete_draft(id).unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }
}
