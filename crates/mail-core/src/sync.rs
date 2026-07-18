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

use crate::action::Action;
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
    /// Actions locales rejouées vers le serveur en tête de synchro.
    pub replayed: usize,
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
        account_id: i64,
        mailbox: &str,
    ) -> Result<SyncReport, Error> {
        let snapshot = server.select(mailbox)?;

        let state = match store.sync_state(account_id, mailbox)? {
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
                let mailbox_id =
                    store.create_mailbox(account_id, mailbox, snapshot.uid_validity)?;
                SyncState {
                    mailbox_id,
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                }
            }
        };

        // Les intentions locales d'abord : la synchro qui suit reflète
        // ainsi leur effet (le rejeu bump le modseq côté serveur).
        let replayed = replay_actions(server, store, mailbox, state.mailbox_id)?;

        let mut report = if state.last_uid == 0 {
            self.initial_sync(server, store, mailbox, state.mailbox_id)?
        } else {
            self.incremental_sync(server, store, mailbox, &state)?
        };
        report.replayed = replayed;

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
            replayed: 0,
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
            replayed: 0,
        })
    }
}

/// Rejoue la file d'actions vers le serveur, dans l'ordre d'émission.
/// Au premier échec, le rejeu s'arrête et le reste de la file survit :
/// il sera retenté à la synchro suivante — aucune intention n'est perdue.
fn replay_actions(
    server: &mut dyn MailServer,
    store: &mut Store,
    mailbox: &str,
    mailbox_id: i64,
) -> Result<usize, Error> {
    let mut replayed = 0;
    for pending in store.pending_actions(mailbox_id)? {
        let outcome = match pending.action {
            Action::MarkSeen => server.set_seen(mailbox, pending.uid, true),
            Action::MarkUnseen => server.set_seen(mailbox, pending.uid, false),
            Action::MarkFlagged => server.set_flagged(mailbox, pending.uid, true),
            Action::MarkUnflagged => server.set_flagged(mailbox, pending.uid, false),
            Action::Archive => server.archive(mailbox, pending.uid),
            Action::Delete => server.delete(mailbox, pending.uid),
        };
        match outcome {
            Ok(()) => {
                store.remove_action(pending.id)?;
                replayed += 1;
            }
            Err(_) => break,
        }
    }
    Ok(replayed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn synced(server: &mut FakeServer, store: &mut Store, engine: &SyncEngine) -> SyncReport {
        let account = test_account(store);
        engine.sync(server, store, account, "INBOX").unwrap()
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<crate::Envelope> {
        let account = test_account(store);
        store.recent(account, "INBOX", offset, limit).unwrap()
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
        assert_eq!(recent(&store, 0, 10).len(), 4);
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
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
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
        assert!(recent(&store, 0, 1)[0].seen);
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
        assert_eq!(recent(&store, 0, 10).len(), 2);
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

        assert!(!recent(&store, 0, 1)[0].seen);
    }

    fn mailbox_id(store: &Store) -> i64 {
        store
            .sync_state(test_account(store), "INBOX")
            .unwrap()
            .unwrap()
            .mailbox_id
    }

    #[test]
    fn replay_pushes_queued_actions_to_server_in_order() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 2, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnseen).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 3);
        assert_eq!(
            server.action_calls,
            vec!["seen:1:true", "seen:2:true", "seen:1:false"],
            "le rejeu doit préserver l'ordre d'émission"
        );
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn replay_stars_and_unstars_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "à étoiler");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkFlagged).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnflagged).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["flag:1:true", "flag:1:false"]);
        assert!(!server.messages[&1].0.flagged);
    }

    #[test]
    fn condstore_propagates_star_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "étoilé ailleurs");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_flagged(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].flagged);
    }

    #[test]
    fn replay_archives_and_deletes_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "à archiver");
        server.add(2, "à supprimer");
        server.add(3, "à garder");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.remove_local(id, 1).unwrap();
        store.remove_local(id, 2).unwrap();
        store.enqueue_action(id, 1, Action::Archive).unwrap();
        store.enqueue_action(id, 2, Action::Delete).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["archive:1", "delete:2"]);
        assert!(!server.messages.contains_key(&1));
        assert!(!server.messages.contains_key(&2));
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3], "seul le message gardé reste localement");
    }

    /// Le gate de la Phase 2 : une coupure pendant le rejeu ne perd rien —
    /// la file survit et repart à la synchro suivante.
    #[test]
    fn failed_replay_keeps_actions_queued_for_next_sync() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

        server.actions_fail = true;
        let cut = synced(&mut server, &mut store, &engine);
        assert_eq!(cut.replayed, 0);
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);

        server.actions_fail = false;
        let recovered = synced(&mut server, &mut store, &engine);
        assert_eq!(recovered.replayed, 1);
        assert!(store.pending_actions(id).unwrap().is_empty());
        assert!(server.messages[&1].0.seen);
    }

    #[test]
    fn uid_validity_reset_drops_now_meaningless_actions() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        server.bump_uid_validity();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 0);
        assert!(server.action_calls.is_empty());
        assert!(store.pending_actions(id).unwrap().is_empty());
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
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 10);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
    }
}
