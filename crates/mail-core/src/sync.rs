//! Le moteur de synchronisation « enveloppes d'abord ».
//!
//! Protocole (décisions gelées, PHASE0.md §2) :
//! - synchro initiale du **plus récent au plus ancien**, par lots — la liste
//!   devient utilisable dès le premier lot ;
//! - synchro incrémentale : CONDSTORE quand le serveur l'expose (nouveaux
//!   messages + changements de flags), sinon différentiel d'UIDs pour les
//!   nouveaux ; les suppressions passent toujours par le différentiel ;
//! - changement d'UIDVALIDITY → resynchronisation complète.

use std::collections::HashSet;

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::{Store, SyncState};

const DEFAULT_BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Initial,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub mode: SyncMode,
    /// Enveloppes récupérées ou mises à jour (nouveaux messages + flags).
    pub fetched: usize,
    /// Enveloppes locales supprimées car disparues du serveur.
    pub deleted: usize,
}

pub struct SyncEngine {
    batch_size: usize,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl SyncEngine {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
        }
    }

    pub fn sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
    ) -> Result<SyncReport, Error> {
        let snapshot = server.select(mailbox)?;

        let state = match store.sync_state(mailbox)? {
            Some(state) if state.uid_validity == snapshot.uid_validity => state,
            Some(stale) => {
                store.reset_mailbox(stale.mailbox_id, snapshot.uid_validity)?;
                SyncState {
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                    ..stale
                }
            }
            None => {
                let mailbox_id = store.create_mailbox(mailbox, snapshot.uid_validity)?;
                SyncState {
                    mailbox_id,
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                }
            }
        };

        let report = if state.last_uid == 0 {
            self.initial_sync(server, store, mailbox, state.mailbox_id)?
        } else {
            self.incremental_sync(server, store, mailbox, &state)?
        };

        let last_uid = store.max_uid(state.mailbox_id)?;
        store.update_state(state.mailbox_id, last_uid, snapshot.highest_modseq)?;
        Ok(report)
    }

    fn initial_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        mailbox_id: i64,
    ) -> Result<SyncReport, Error> {
        let mut uids = server.list_uids(mailbox)?;
        uids.sort_unstable_by(|a, b| b.cmp(a));

        let mut fetched = 0;
        for chunk in uids.chunks(self.batch_size) {
            let envelopes = server.fetch_envelopes(mailbox, chunk)?;
            fetched += envelopes.len();
            store.upsert_envelopes(mailbox_id, &envelopes)?;
        }
        Ok(SyncReport {
            mode: SyncMode::Initial,
            fetched,
            deleted: 0,
        })
    }

    fn incremental_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        state: &SyncState,
    ) -> Result<SyncReport, Error> {
        let server_uids = server.list_uids(mailbox)?;
        let mut fetched = 0;

        let condstore_changes = match state.highest_modseq {
            Some(modseq) => server.changes_since(mailbox, modseq)?,
            None => None,
        };
        match condstore_changes {
            Some(changed) => {
                fetched += changed.len();
                store.upsert_envelopes(state.mailbox_id, &changed)?;
            }
            None => {
                // Sans CONDSTORE : seuls les nouveaux messages sont détectés ;
                // les changements de flags attendront une resynchro complète.
                let mut new_uids: Vec<Uid> = server_uids
                    .iter()
                    .copied()
                    .filter(|uid| *uid > state.last_uid)
                    .collect();
                new_uids.sort_unstable_by(|a, b| b.cmp(a));
                for chunk in new_uids.chunks(self.batch_size) {
                    let envelopes = server.fetch_envelopes(mailbox, chunk)?;
                    fetched += envelopes.len();
                    store.upsert_envelopes(state.mailbox_id, &envelopes)?;
                }
            }
        }

        // CONDSTORE ne signale pas les suppressions (il faudrait QRESYNC,
        // absent chez Gmail) : le différentiel d'UIDs reste la référence.
        let present: HashSet<Uid> = server_uids.into_iter().collect();
        let deleted = store.remove_absent(state.mailbox_id, &present)?;

        Ok(SyncReport {
            mode: SyncMode::Incremental,
            fetched,
            deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn synced(server: &mut FakeServer, store: &mut Store, engine: &SyncEngine) -> SyncReport {
        engine.sync(server, store, "INBOX").unwrap()
    }

    #[test]
    fn initial_sync_fetches_newest_first_in_batches() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, "sujet");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::new(2);

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        assert_eq!(report.fetched, 5);
        assert_eq!(
            server.fetch_batches,
            vec![vec![5, 4], vec![3, 2], vec![1]],
            "la synchro initiale doit servir le plus récent d'abord"
        );
    }

    #[test]
    fn initial_sync_of_empty_mailbox_fetches_nothing() {
        let mut server = FakeServer::new(false);
        let mut store = Store::open_in_memory().unwrap();

        let report = synced(&mut server, &mut store, &SyncEngine::default());

        assert_eq!(report.fetched, 0);
        assert!(server.fetch_batches.is_empty());
    }

    #[test]
    fn resync_without_changes_is_incremental_and_idempotent() {
        for condstore in [false, true] {
            let mut server = FakeServer::new(condstore);
            server.add(1, "a");
            let mut store = Store::open_in_memory().unwrap();
            let engine = SyncEngine::default();

            synced(&mut server, &mut store, &engine);
            let second = synced(&mut server, &mut store, &engine);

            assert_eq!(second.mode, SyncMode::Incremental);
            assert_eq!(second.fetched, 0, "condstore={condstore}");
            assert_eq!(second.deleted, 0);
        }
    }

    #[test]
    fn incremental_fetches_only_new_messages() {
        let mut server = FakeServer::new(false);
        server.add(1, "ancien");
        server.add(2, "ancien");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(3, "nouveau");
        server.add(4, "nouveau");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 2);
        assert_eq!(server.fetch_batches.last(), Some(&vec![4, 3]));
        assert_eq!(store.recent("INBOX", 0, 10).unwrap().len(), 4);
    }

    #[test]
    fn incremental_removes_expunged_messages() {
        let mut server = FakeServer::new(false);
        for uid in 1..=3 {
            server.add(uid, "sujet");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.expunge(2);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.deleted, 1);
        let uids: Vec<Uid> = store
            .recent("INBOX", 0, 10)
            .unwrap()
            .iter()
            .map(|e| e.uid)
            .collect();
        assert_eq!(uids, vec![3, 1]);
    }

    #[test]
    fn condstore_propagates_flag_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "à lire");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(store.recent("INBOX", 0, 1).unwrap()[0].seen);
    }

    #[test]
    fn condstore_picks_up_new_messages_too() {
        let mut server = FakeServer::new(true);
        server.add(1, "ancien");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(2, "nouveau");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert_eq!(store.recent("INBOX", 0, 10).unwrap().len(), 2);
    }

    /// Limite connue et assumée : sans CONDSTORE, un flag changé côté serveur
    /// n'est pas rafraîchi par la synchro incrémentale. Ce test documente le
    /// comportement pour qu'une future correction soit un choix, pas un hasard.
    #[test]
    fn without_condstore_flag_changes_are_not_detected() {
        let mut server = FakeServer::new(false);
        server.add(1, "à lire");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        synced(&mut server, &mut store, &engine);

        assert!(!store.recent("INBOX", 0, 1).unwrap()[0].seen);
    }

    #[test]
    fn uid_validity_change_triggers_full_resync() {
        let mut server = FakeServer::new(false);
        server.add(1, "avant");
        server.add(2, "avant");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.bump_uid_validity();
        server.messages.clear();
        server.add(10, "après");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        let rows = store.recent("INBOX", 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 10);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
    }
}
