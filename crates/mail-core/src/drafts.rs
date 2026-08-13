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
//! **Tirage** (Phase 3) : un brouillon créé ailleurs — webmail, téléphone
//! — est rapatrié pour être édité ici. Le sens inverse rouvre une question
//! que la poussée seule évitait : que faire quand les deux côtés ont
//! bougé ? La réponse suit la règle d'or déjà en vigueur — **un doublon
//! est acceptable, du texte perdu jamais** — et se lit dans
//! [`plan_draft_pull`].

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// Le contenu d'un brouillon, tel que l'éditeur le tient.
///
/// Regroupé plutôt qu'étalé en paramètres : ces quatre champs voyagent
/// toujours ensemble, et une signature qui les sépare invite à en
/// intervertir deux — ils sont tous des chaînes.
///
/// Rien n'est validé ici : une adresse à moitié tapée doit se conserver
/// telle quelle. La validation stricte n'intervient qu'à l'envoi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftContent<'a> {
    pub to_raw: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
    pub reply_to_uid: Option<Uid>,
    /// La boîte qui donne son sens à `reply_to_uid` : les UID repartent
    /// de 1 à chaque boîte (ADR 0009), un UID seul ne désigne rien.
    /// C'est elle qui permet de relier le brouillon à sa conversation
    /// (PLAN-BROUILLONS, B-D2).
    pub reply_to_mailbox: Option<&'a str>,
}

/// Ce qu'une sauvegarde a réellement fait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSaved {
    pub id: i64,
    /// À repasser en `base_epoch` à la sauvegarde suivante.
    pub updated_epoch: i64,
    /// La version en base avait changé sous les doigts de l'éditeur : son
    /// texte a été conservé **à part** au lieu d'écraser l'autre.
    pub forked: bool,
}

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
    /// La boîte du message visé — `None` pour une composition libre ou
    /// un brouillon d'avant la colonne (ils gardent leur filet : le
    /// dossier Brouillons, sans mention en liste).
    pub reply_to_mailbox: Option<String>,
    /// Le fil de la conversation à laquelle ce brouillon répond —
    /// résolu à la LECTURE (boîte + UID → enveloppe), jamais stocké :
    /// un fil re-calculé ne peut pas laisser un repère périmé ici.
    /// `None` : composition libre, boîte disparue, message expurgé, ou
    /// brouillon d'avant la colonne.
    pub thread_id: Option<i64>,
    /// Millisecondes — l'ordre « plus récent d'abord » doit rester vrai
    /// entre deux sauvegardes rapprochées.
    pub updated_epoch: i64,
    /// UID de la dernière copie poussée dans le dossier Brouillons Gmail.
    pub remote_uid: Option<Uid>,
    /// Photo d'`updated_epoch` au moment de la dernière poussée réussie.
    pub pushed_epoch: Option<i64>,
}

impl Store {
    /// Enregistre (`id: None`) ou met à jour un brouillon.
    ///
    /// Un id périmé (brouillon supprimé entre-temps par une autre vue)
    /// ré-insère au lieu de perdre silencieusement le texte — c'est un
    /// filet, il ne doit jamais avoir de maille manquante.
    ///
    /// `base_epoch` est l'`updated_epoch` que l'éditeur croit modifier.
    /// C'est une **affirmation** : « je modifie la ligne que j'ai lue ».
    /// Elle se dément de deux façons, et les deux comptent :
    ///
    /// 1. l'horodatage en base a changé — quelqu'un a réécrit la ligne ;
    /// 2. **la ligne a disparu** — le tirage l'a *remplacée*, car il ne
    ///    met pas à jour : il retire le miroir périmé et importe la
    ///    version fraîche sous un nouvel identifiant
    ///    ([`plan_draft_pull`]). C'est le seul des deux cas que le terrain
    ///    produise vraiment, et c'était celui qui passait inaperçu : ne
    ///    comparant que des horodatages, la détection se taisait dès qu'il
    ///    n'y en avait plus qu'un.
    ///
    /// Dans les deux cas, écraser — ou ré-insérer en silence — laisse
    /// l'utilisateur avec deux textes dont il ignore l'existence. On garde
    /// donc les DEUX **et on le dit** : la règle d'or du module appliquée
    /// à l'édition concurrente.
    ///
    /// `None` désactive la détection — pour les appelants qui ne
    /// détiennent pas de copie en mémoire, et n'ont donc rien à écraser.
    pub fn save_draft(
        &self,
        account_id: i64,
        id: Option<i64>,
        base_epoch: Option<i64>,
        content: DraftContent<'_>,
    ) -> Result<DraftSaved, Error> {
        let DraftContent {
            to_raw,
            subject,
            body,
            reply_to_uid,
            reply_to_mailbox,
        } = content;
        let now = Utc::now().timestamp_millis();
        match id {
            Some(id) => {
                let stored: Option<i64> = self
                    .conn()
                    .query_row(
                        "SELECT updated_epoch FROM drafts WHERE id = ?1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()?;
                let conflit = match (stored, base_epoch) {
                    // La ligne a été réécrite sous le composeur.
                    (Some(stored), Some(base)) => stored != base,
                    // Elle a disparu sous lui : remplacée par le tirage,
                    // ou jetée depuis une autre vue. Le filet la ré-insère
                    // de toute façon — mais en silence, les deux textes
                    // devenaient indiscernables.
                    (None, Some(_)) => true,
                    // Aucune copie en mémoire : rien à écraser.
                    (_, None) => false,
                };
                if conflit {
                    let forked = self.insert_draft(
                        account_id,
                        to_raw,
                        subject,
                        body,
                        reply_to_uid,
                        reply_to_mailbox,
                        now,
                    )?;
                    return Ok(DraftSaved {
                        id: forked,
                        updated_epoch: now,
                        forked: true,
                    });
                }
                // MAX(…, +1) : l'horodatage avance STRICTEMENT à chaque
                // vraie modification — une édition dans la même milliseconde
                // que la photo d'une poussée resterait sinon invisible
                // (maille du filet, attrapée par test). Et le WHERE : une
                // sauvegarde au contenu IDENTIQUE ne touche à rien, sinon
                // chaque fermeture re-pousserait une copie identique vers
                // Gmail (churn observé en validation terrain).
                self.conn().execute(
                    "INSERT INTO drafts (id, account_id, to_raw, subject, body, reply_to_uid, reply_to_mailbox, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                       to_raw = excluded.to_raw,
                       subject = excluded.subject,
                       body = excluded.body,
                       reply_to_uid = excluded.reply_to_uid,
                       reply_to_mailbox = excluded.reply_to_mailbox,
                       updated_epoch = MAX(excluded.updated_epoch, drafts.updated_epoch + 1)
                     WHERE drafts.to_raw IS NOT excluded.to_raw
                        OR drafts.subject IS NOT excluded.subject
                        OR drafts.body IS NOT excluded.body
                        OR drafts.reply_to_uid IS NOT excluded.reply_to_uid
                        OR drafts.reply_to_mailbox IS NOT excluded.reply_to_mailbox",
                    params![id, account_id, to_raw, subject, body, reply_to_uid, reply_to_mailbox, now],
                )?;
                // Relu, et non supposé : le `WHERE` ci-dessus peut avoir
                // laissé l'horodatage intact (sauvegarde identique), et
                // rendre `now` ferait échouer la détection au tour
                // suivant sur un conflit qui n'existe pas.
                let updated_epoch = self.conn().query_row(
                    "SELECT updated_epoch FROM drafts WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;
                Ok(DraftSaved {
                    id,
                    updated_epoch,
                    forked: false,
                })
            }
            None => Ok(DraftSaved {
                id: self.insert_draft(
                    account_id,
                    to_raw,
                    subject,
                    body,
                    reply_to_uid,
                    reply_to_mailbox,
                    now,
                )?,
                updated_epoch: now,
                forked: false,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_draft(
        &self,
        account_id: i64,
        to_raw: &str,
        subject: &str,
        body: &str,
        reply_to_uid: Option<Uid>,
        reply_to_mailbox: Option<&str>,
        now: i64,
    ) -> Result<i64, Error> {
        self.conn().execute(
            "INSERT INTO drafts (account_id, to_raw, subject, body, reply_to_uid, reply_to_mailbox, updated_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![account_id, to_raw, subject, body, reply_to_uid, reply_to_mailbox, now],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// Les brouillons, les plus récents d'abord.
    pub fn drafts(&self) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT} ORDER BY d.updated_epoch DESC, d.id DESC"
        ))?;
        let rows = stmt
            .query_map([], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Tous les brouillons d'UN compte — ce que le tirage compare à la
    /// liste distante.
    pub fn drafts_of(&self, account_id: i64) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT} WHERE d.account_id = ?1 ORDER BY d.id"
        ))?;
        let rows = stmt
            .query_map([account_id], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Les brouillons d'UN compte dont son dossier Brouillons n'a pas
    /// (ou plus) la dernière version, dans l'ordre de création.
    pub fn drafts_to_push(&self, account_id: i64) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT}
             WHERE d.account_id = ?1
               AND (d.pushed_epoch IS NULL OR d.pushed_epoch < d.updated_epoch)
             ORDER BY d.id"
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

    /// Enregistre un brouillon rapatrié du serveur.
    ///
    /// Il naît **propre** : `pushed_epoch` égale `updated_epoch`, donc le
    /// cycle suivant ne le repoussera pas. Le repousser tel quel créerait
    /// une seconde copie distante d'un message qu'on vient de lire — un
    /// aller-retour qui se doublerait à chaque passage.
    pub fn import_remote_draft(
        &self,
        account_id: i64,
        remote_uid: Uid,
        to_raw: &str,
        subject: &str,
        body: &str,
    ) -> Result<i64, Error> {
        let now = Utc::now().timestamp_millis();
        self.conn().execute(
            "INSERT INTO drafts
             (account_id, to_raw, subject, body, reply_to_uid, updated_epoch,
              remote_uid, pushed_epoch)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?5)",
            params![account_id, to_raw, subject, body, now, remote_uid],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// Retire un miroir devenu périmé — sa copie distante n'existe plus.
    ///
    /// **Sans tombstone**, contrairement à [`Store::delete_draft`] : il n'y
    /// a plus rien à supprimer côté serveur, et poser une pierre tombale
    /// sur un UID libéré ferait purger le message qui le reprendra.
    pub fn drop_stale_draft(&self, id: i64) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM drafts WHERE id = ?1", [id])?;
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

/// Ce qu'il faut faire du dossier Brouillons distant, une fois ses UIDs
/// connus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftPull {
    /// UIDs distants qu'on ne connaît pas : à rapatrier.
    pub fetch: Vec<Uid>,
    /// Brouillons locaux qui ne sont QUE le miroir d'une copie distante
    /// disparue : à retirer.
    ///
    /// Jamais un brouillon édité ici — celui-là porte du texte que le
    /// serveur n'a pas.
    pub stale: Vec<i64>,
}

/// Décide du tirage : quoi rapatrier, quels miroirs périmés retirer.
///
/// Pur et sans I/O, comme le regroupement en fils : la décision se teste
/// contre les scénarios du terrain, l'exécution reste à l'appelant.
///
/// Trois règles, dans l'ordre de leur importance :
///
/// 1. **On ne rapatrie pas ce qu'on a déjà.** Nos propres copies poussées
///    (`remote_uid`) et celles en attente de purge (tombstones) sont
///    ignorées, sinon chaque cycle dupliquerait la boîte.
/// 2. **On ne retire qu'un miroir.** Un brouillon dont la copie distante
///    a disparu est retiré *seulement* s'il n'a pas été édité ici depuis
///    sa dernière poussée. Sinon il porte du texte que le serveur n'a
///    jamais vu : il reste, et la poussée le remettra en place.
/// 3. **Une liste distante vide ne retire rien.** C'est exactement la
///    forme d'un échec partiel — dossier mal sélectionné, réponse
///    tronquée — et le coût d'une erreur ici, c'est du texte effacé. Si
///    l'utilisateur a vraiment tout supprimé ailleurs, ses copies
///    survivent localement : un doublon, pas une perte.
///
/// La règle 2 est ce qui fait qu'éditer un brouillon sur son téléphone
/// **remplace** la copie locale au lieu de la doubler : le serveur
/// remplace le message (ancien UID expurgé, nouveau créé), donc le même
/// passage retire le miroir périmé et rapatrie la version fraîche.
pub fn plan_draft_pull(local: &[SavedDraft], remote: &[Uid], tombstones: &[Uid]) -> DraftPull {
    let mirrored: Vec<Uid> = local.iter().filter_map(|draft| draft.remote_uid).collect();
    let fetch = remote
        .iter()
        .copied()
        .filter(|uid| !mirrored.contains(uid) && !tombstones.contains(uid))
        .collect();

    if remote.is_empty() {
        return DraftPull {
            fetch,
            stale: Vec::new(),
        };
    }
    let stale = local
        .iter()
        .filter(|draft| draft.is_clean_mirror() && !remote.contains(&draft.remote_uid.unwrap_or(0)))
        .map(|draft| draft.id)
        .collect();
    DraftPull { fetch, stale }
}

impl SavedDraft {
    /// Le brouillon n'est-il que le reflet d'une copie distante ?
    ///
    /// Vrai quand une copie a été poussée (ou rapatriée) et que rien n'a
    /// été tapé ici depuis. C'est la seule condition sous laquelle le
    /// retirer ne peut effacer aucun texte.
    fn is_clean_mirror(&self) -> bool {
        match (self.remote_uid, self.pushed_epoch) {
            (Some(_), Some(pushed)) => pushed >= self.updated_epoch,
            _ => false,
        }
    }
}

// Le fil se résout à la lecture — LEFT JOIN : un brouillon dont la
// cible a disparu (boîte renommée, message expurgé) reste un brouillon,
// simplement sans fil. `(mailbox_id, uid)` est la clé primaire des
// enveloppes : la jointure ne peut pas multiplier les lignes, et les
// brouillons se comptent en dizaines — le coût est nul.
const DRAFT_SELECT: &str = "SELECT d.id, d.account_id, d.to_raw, d.subject, d.body,
        d.reply_to_uid, d.reply_to_mailbox, re.thread_id,
        d.updated_epoch, d.remote_uid, d.pushed_epoch
 FROM drafts d
 LEFT JOIN mailboxes rm ON rm.account_id = d.account_id AND rm.name = d.reply_to_mailbox
 LEFT JOIN envelopes re ON re.mailbox_id = rm.id AND re.uid = d.reply_to_uid";

fn row_to_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedDraft> {
    Ok(SavedDraft {
        id: row.get(0)?,
        account_id: row.get(1)?,
        to_raw: row.get(2)?,
        subject: row.get(3)?,
        body: row.get(4)?,
        reply_to_uid: row.get(5)?,
        reply_to_mailbox: row.get(6)?,
        thread_id: row.get(7)?,
        updated_epoch: row.get(8)?,
        remote_uid: row.get(9)?,
        pushed_epoch: row.get(10)?,
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
                None,
                DraftContent {
                    to_raw: "adresse-incomp",
                    subject: "Sujet",
                    body: "corps\nsur deux lignes",
                    reply_to_uid: Some(42),
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;

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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "v1",
                    body: "texte",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        let same = store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "v2",
                    body: "texte enrichi",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;

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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "précieux",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        store.delete_draft(id).unwrap();

        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "précieux",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].body, "précieux");
    }

    #[test]
    fn drafts_lists_most_recent_first() {
        let (store, account) = store();
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "premier",
                    body: "a",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "second",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        let drafts = store.drafts().unwrap();
        let subjects: Vec<&str> = drafts.iter().map(|draft| draft.subject.as_str()).collect();
        assert_eq!(subjects, vec!["second", "premier"]);
    }

    #[test]
    fn delete_draft_removes_it() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        store.delete_draft(id).unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }

    #[test]
    fn fresh_and_edited_drafts_are_to_push_until_recorded() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
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
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "v2",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "s",
                    body: "texte",
                    reply_to_uid: Some(1),
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        let epoch = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store.record_draft_pushed(id, Some(101), epoch).unwrap();

        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "s",
                    body: "texte",
                    reply_to_uid: Some(1),
                    reply_to_mailbox: None,
                },
            )
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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        let snapshot = store.drafts_to_push(account).unwrap()[0].updated_epoch;

        // L'utilisateur édite pendant que la poussée est en vol — même
        // dans la même milliseconde, l'horodatage strictement croissant
        // rend l'édition détectable…
        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "v2 éditée en vol",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
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
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "poussé",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        store.record_draft_pushed(pushed, Some(303), 1).unwrap();
        let local_only = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "local",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;

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
        let draft_a = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "a",
                    body: "x",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
        let draft_b = store
            .save_draft(
                other,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "b",
                    body: "y",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
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
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    subject: "s",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap()
            .id;
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

/// L'édition concurrente : deux écrivains sur le même brouillon.
#[cfg(test)]
mod tests_concurrence {
    use super::*;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    /// LE défaut du terrain : le composeur tient une copie en mémoire, le
    /// tirage remplace le brouillon sous lui, et la sauvegarde suivante
    /// écrasait la version venue d'ailleurs.
    ///
    /// Les deux textes doivent survivre. C'est la règle d'or du module —
    /// un doublon est acceptable, du texte perdu jamais — appliquée à
    /// l'édition concurrente.
    #[test]
    fn une_edition_concurrente_conserve_les_deux_textes() {
        let (store, account) = store();
        let ouvert = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "version composeur",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        // Quelqu'un d'autre écrit : le tirage, en pratique.
        store
            .save_draft(
                account,
                Some(ouvert.id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "version venue d'ailleurs",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        // Le composeur sauvegarde, en croyant modifier ce qu'il a lu.
        let bilan = store
            .save_draft(
                account,
                Some(ouvert.id),
                Some(ouvert.updated_epoch),
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "version composeur",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        assert!(bilan.forked, "le texte de l'autre côté n'est pas écrasé");
        assert_ne!(bilan.id, ouvert.id, "il est conservé à part");
        let textes: Vec<String> = store
            .drafts()
            .unwrap()
            .into_iter()
            .map(|draft| draft.body)
            .collect();
        assert_eq!(textes.len(), 2);
        assert!(textes.contains(&"version composeur".to_string()));
        assert!(textes.contains(&"version venue d'ailleurs".to_string()));
    }

    /// L'aller-retour : l'horodatage rendu doit permettre d'enchaîner les
    /// sauvegardes sans déclencher de faux conflit. Le piège est réel —
    /// une sauvegarde au contenu identique ne touche PAS à l'horodatage,
    /// donc rendre « maintenant » ferait diverger l'éditeur de la base.
    #[test]
    fn l_horodatage_rendu_permet_d_enchainer_les_sauvegardes() {
        let (store, account) = store();
        let mut bilan = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "un",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        for texte in ["deux", "deux", "trois"] {
            bilan = store
                .save_draft(
                    account,
                    Some(bilan.id),
                    Some(bilan.updated_epoch),
                    DraftContent {
                        to_raw: "a@b.fr",
                        subject: "Devis",
                        body: texte,
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                    },
                )
                .unwrap();
            assert!(!bilan.forked, "aucun conflit : c'est le même éditeur");
        }
        assert_eq!(store.drafts().unwrap().len(), 1);
    }

    /// Sans `base_epoch`, rien ne change : les appelants qui ne tiennent
    /// aucune copie en mémoire n'ont rien à écraser.
    #[test]
    fn sans_horodatage_de_reference_la_sauvegarde_met_a_jour_en_place() {
        let (store, account) = store();
        let premier = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "un",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();
        let second = store
            .save_draft(
                account,
                Some(premier.id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "deux",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        assert!(!second.forked);
        assert_eq!(second.id, premier.id);
        assert_eq!(store.drafts().unwrap().len(), 1);
    }

    /// Le tirage ne met PAS le brouillon à jour : il le **remplace**
    /// ([`plan_draft_pull`]) — le miroir périmé est retiré, la version
    /// fraîche arrive sous un nouvel identifiant. Le composeur, lui, tient
    /// toujours l'ancien : la ligne qu'il croit modifier n'existe plus.
    ///
    /// C'est un conflit au même titre qu'une réécriture en place, et le
    /// seul que le terrain produise vraiment. La détection ne le voyait
    /// pas : elle compare deux horodatages, et il n'y en a plus qu'un.
    /// Symptôme rapporté : « le message rouge ne s'affiche pas ».
    #[test]
    fn un_brouillon_remplace_par_le_tirage_est_aussi_un_conflit() {
        let (store, account) = store();
        let ouvert = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "version composeur",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();
        // Un brouillon PLUS RÉCENT existe, et c'est ce qui rend le défaut
        // visible. SQLite attribue `max(rowid) + 1` : si le brouillon
        // édité était le dernier, l'import reprendrait l'identifiant qu'il
        // vient de libérer, la ligne réapparaîtrait sous le composeur et
        // la détection retomberait sur ses pieds **par accident**. Un seul
        // brouillon plus jeune suffit à supprimer cette coïncidence — d'où
        // un défaut qui ne se manifeste qu'une fois sur deux, exactement
        // ce que le terrain a rapporté.
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "x@y.fr",
                    subject: "Autre",
                    body: "z",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();
        store
            .record_draft_pushed(ouvert.id, Some(7), ouvert.updated_epoch)
            .unwrap();

        // Retouché dans le webmail : le serveur expurge 7 et crée 8.
        let local = store.drafts_of(account).unwrap();
        let plan = plan_draft_pull(&local, &[8], &[]);
        assert_eq!(plan.stale, vec![ouvert.id], "le miroir périmé part");
        for id in plan.stale {
            store.drop_stale_draft(id).unwrap();
        }
        for uid in plan.fetch {
            store
                .import_remote_draft(account, uid, "a@b.fr", "Devis", "version venue d'ailleurs")
                .unwrap();
        }

        // Le composeur se ferme et sauvegarde ce qu'il tenait.
        let bilan = store
            .save_draft(
                account,
                Some(ouvert.id),
                Some(ouvert.updated_epoch),
                DraftContent {
                    to_raw: "a@b.fr",
                    subject: "Devis",
                    body: "version composeur",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                },
            )
            .unwrap();

        assert!(
            bilan.forked,
            "la ligne visée avait disparu sous le composeur : le taire \
             laisse l'utilisateur avec deux textes sans le savoir"
        );
        let textes: Vec<String> = store
            .drafts()
            .unwrap()
            .into_iter()
            .map(|draft| draft.body)
            .collect();
        assert!(textes.contains(&"version composeur".to_string()));
        assert!(textes.contains(&"version venue d'ailleurs".to_string()));
    }
}

/// Le tirage a ses propres scenarios : ils ne partagent ni decor ni
/// invariants avec ceux de la poussee, plus haut.
#[cfg(test)]
mod tests_tirage {
    use super::*;

    fn draft(id: i64, remote_uid: Option<Uid>, updated: i64, pushed: Option<i64>) -> SavedDraft {
        SavedDraft {
            id,
            account_id: 1,
            to_raw: "alice@exemple.fr".to_string(),
            subject: "Devis".to_string(),
            body: "Bonjour".to_string(),
            reply_to_uid: None,
            reply_to_mailbox: None,
            thread_id: None,
            updated_epoch: updated,
            remote_uid,
            pushed_epoch: pushed,
        }
    }

    /// Le brouillon écrit dans le webmail : personne ne le connaît ici.
    #[test]
    fn un_brouillon_distant_inconnu_est_rapatrie() {
        let plan = plan_draft_pull(&[], &[7], &[]);
        assert_eq!(plan.fetch, vec![7]);
        assert!(plan.stale.is_empty());
    }

    /// Notre propre copie poussée ne doit pas revenir : sans cette garde,
    /// chaque cycle dupliquerait la boîte de brouillons.
    #[test]
    fn notre_propre_copie_poussee_n_est_pas_rapatriee() {
        let plan = plan_draft_pull(&[draft(1, Some(7), 100, Some(100))], &[7], &[]);
        assert!(plan.fetch.is_empty());
        assert!(plan.stale.is_empty(), "le miroir est à jour");
    }

    /// Une copie qu'on a demandé à supprimer mais pas encore purgée est
    /// encore là : la rapatrier ressusciterait un brouillon jeté.
    #[test]
    fn une_copie_en_attente_de_purge_n_est_pas_rapatriee() {
        let plan = plan_draft_pull(&[], &[7], &[7]);
        assert!(plan.fetch.is_empty());
    }

    /// Éditer un brouillon ailleurs : le serveur expurge l'ancien message
    /// et en crée un neuf. Le même passage doit donc retirer le miroir
    /// périmé ET rapatrier la version fraîche — remplacer, pas doubler.
    #[test]
    fn editer_ailleurs_remplace_le_miroir_au_lieu_de_le_doubler() {
        let plan = plan_draft_pull(&[draft(1, Some(7), 100, Some(100))], &[8], &[]);
        assert_eq!(plan.fetch, vec![8]);
        assert_eq!(plan.stale, vec![1]);
    }

    /// LA règle du module : un brouillon édité ici porte du texte que le
    /// serveur n'a jamais vu. Il ne peut pas être « périmé ».
    #[test]
    fn un_brouillon_edite_ici_n_est_jamais_retire() {
        // Poussé à 100, retouché à 150 : la copie distante est en retard.
        let plan = plan_draft_pull(&[draft(1, Some(7), 150, Some(100))], &[8], &[]);
        assert!(
            plan.stale.is_empty(),
            "le retirer effacerait la retouche locale"
        );
    }

    /// Un brouillon jamais poussé n'a pas de miroir : rien à comparer.
    #[test]
    fn un_brouillon_jamais_pousse_n_est_jamais_retire() {
        let plan = plan_draft_pull(&[draft(1, None, 100, None)], &[8], &[]);
        assert!(plan.stale.is_empty());
    }

    /// Le garde-fou. Une liste vide a exactement la forme d'un échec
    /// partiel, et se tromper ici coûte du texte. Si l'utilisateur a
    /// vraiment tout supprimé ailleurs, ses copies survivent localement :
    /// un doublon, pas une perte.
    #[test]
    fn une_liste_distante_vide_ne_retire_rien() {
        let locaux = [
            draft(1, Some(7), 100, Some(100)),
            draft(2, Some(8), 100, Some(100)),
        ];
        let plan = plan_draft_pull(&locaux, &[], &[]);
        assert!(plan.stale.is_empty(), "un dossier vide ne prouve rien");
        assert!(plan.fetch.is_empty());
    }

    /// Plusieurs comptes, plusieurs brouillons : le plan reste stable et
    /// ne mélange rien. L'appelant filtre déjà par compte.
    #[test]
    fn le_plan_traite_plusieurs_brouillons_sans_les_confondre() {
        let locaux = [
            draft(1, Some(7), 100, Some(100)), // à jour
            draft(2, Some(8), 100, Some(100)), // disparu du serveur
            draft(3, Some(9), 200, Some(100)), // édité ici
            draft(4, None, 100, None),         // jamais poussé
        ];
        let plan = plan_draft_pull(&locaux, &[7, 42], &[]);
        assert_eq!(plan.fetch, vec![42]);
        assert_eq!(plan.stale, vec![2]);
    }
}

/// Le lien brouillon -> conversation (PLAN-BROUILLONS, B-D2) : résolu à
/// la lecture, jamais stocké — et jamais deviné (ADR 0009 : un UID sans
/// sa boîte ne désigne rien).
#[cfg(test)]
mod tests_fil {
    use chrono::TimeZone;

    use super::*;
    use crate::envelope::Envelope;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    fn message(uid: Uid, subject: &str) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Marie Dubois".to_string()),
            sender_address: Some("marie@exemple.fr".to_string()),
            message_id: Some(format!("<m{uid}@exemple.fr>")),
            in_reply_to: None,
            date: Some(chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap()),
            seen: true,
            flagged: false,
        }
    }

    fn reponse<'a>(uid: Option<Uid>, boite: Option<&'a str>) -> DraftContent<'a> {
        DraftContent {
            to_raw: "marie@exemple.fr",
            subject: "Re : Devis",
            body: "Bonjour Marie,",
            reply_to_uid: uid,
            reply_to_mailbox: boite,
        }
    }

    #[test]
    fn un_brouillon_reponse_se_relie_a_son_fil() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Devis")])
            .unwrap();
        store
            .save_draft(account, None, None, reponse(Some(42), Some("INBOX")))
            .unwrap();

        let fil = store.unified_recent(0, 10).unwrap()[0].thread_id;
        assert!(fil.is_some(), "le décor doit porter un fil");
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].thread_id, fil, "même fil que le message visé");
        assert_eq!(drafts[0].reply_to_mailbox.as_deref(), Some("INBOX"));
    }

    #[test]
    fn une_composition_libre_reste_sans_fil() {
        let (store, account) = store();
        store
            .save_draft(account, None, None, reponse(None, None))
            .unwrap();
        assert_eq!(store.drafts().unwrap()[0].thread_id, None);
    }

    /// La cible peut manquer de deux façons — boîte jamais vue (renommée,
    /// compte réagencé) ou message expurgé — et aucune ne doit faire
    /// disparaître le brouillon : il reste, simplement sans fil.
    #[test]
    fn une_boite_inconnue_ou_un_message_expurge_laissent_sans_fil() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Devis")])
            .unwrap();
        store
            .save_draft(account, None, None, reponse(Some(42), Some("Ailleurs")))
            .unwrap();
        store
            .save_draft(account, None, None, reponse(Some(99), Some("INBOX")))
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 2, "les brouillons survivent à la cible");
        assert!(drafts.iter().all(|draft| draft.thread_id.is_none()));
    }

    /// Les brouillons d'avant la colonne : `reply_to_uid` sans boîte.
    /// Ils ne se relient JAMAIS — un UID seul pourrait pointer le
    /// mauvais message (ADR 0009) ; leur filet reste le dossier.
    #[test]
    fn un_brouillon_d_avant_la_colonne_reste_sans_fil() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Devis")])
            .unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO drafts (account_id, to_raw, subject, body, reply_to_uid, updated_epoch)
                 VALUES (?1, '', 'Re : Devis', 'b', 42, 1)",
                [account],
            )
            .unwrap();
        assert_eq!(store.drafts().unwrap()[0].thread_id, None);
    }

    /// Le WHERE anti-churn couvre la colonne neuve : corriger SEULEMENT
    /// la boîte visée doit remarquer le brouillon à pousser.
    #[test]
    fn changer_la_boite_visee_remarque_le_brouillon_a_pousser() {
        let (store, account) = store();
        let saved = store
            .save_draft(account, None, None, reponse(Some(42), Some("INBOX")))
            .unwrap();
        store
            .record_draft_pushed(saved.id, Some(7), saved.updated_epoch)
            .unwrap();
        assert!(store.drafts_to_push(account).unwrap().is_empty());

        store
            .save_draft(
                account,
                Some(saved.id),
                None,
                reponse(Some(42), Some("Archives")),
            )
            .unwrap();
        assert_eq!(store.drafts_to_push(account).unwrap().len(), 1);
    }
}
