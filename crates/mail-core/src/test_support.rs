//! Serveur simulé partagé par les tests : rejoue les scénarios du terrain —
//! ajouts, suppressions, flags, bascule d'UIDVALIDITY, corps de messages —
//! et journalise les appels pour vérifier l'ordre et le nombre d'accès.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};

use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::remote::{MailServer, MailboxSnapshot};

pub(crate) struct FakeServer {
    pub(crate) uid_validity: u32,
    pub(crate) condstore: bool,
    pub(crate) modseq: u64,
    pub(crate) messages: BTreeMap<Uid, (Envelope, u64)>,
    pub(crate) bodies: BTreeMap<Uid, String>,
    pub(crate) fetch_batches: Vec<Vec<Uid>>,
    pub(crate) body_fetches: usize,
    /// Journal des changements de flags reçus, dans l'ordre.
    pub(crate) set_seen_calls: Vec<(Uid, bool)>,
    /// Simule une coupure sur les changements de flags (test « zéro perte »).
    pub(crate) flags_fail: bool,
}

impl FakeServer {
    pub(crate) fn new(condstore: bool) -> Self {
        Self {
            uid_validity: 1,
            condstore,
            modseq: 0,
            messages: BTreeMap::new(),
            bodies: BTreeMap::new(),
            fetch_batches: Vec::new(),
            body_fetches: 0,
            set_seen_calls: Vec::new(),
            flags_fail: false,
        }
    }

    pub(crate) fn add(&mut self, uid: Uid, subject: &str) {
        self.modseq += 1;
        let envelope = Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("alice@example.com".to_string()),
            // La date suit l'UID : plus l'UID est grand, plus c'est récent.
            date: Some(
                Utc.timestamp_opt(1_700_000_000 + i64::from(uid), 0)
                    .unwrap(),
            ),
            seen: false,
        };
        self.messages.insert(uid, (envelope, self.modseq));
    }

    pub(crate) fn add_with_body(&mut self, uid: Uid, subject: &str, html: &str) {
        self.add(uid, subject);
        self.bodies.insert(uid, html.to_string());
    }

    pub(crate) fn expunge(&mut self, uid: Uid) {
        self.messages.remove(&uid);
        self.bodies.remove(&uid);
        self.modseq += 1;
    }

    pub(crate) fn mark_seen(&mut self, uid: Uid) {
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.seen = true;
            *modseq = self.modseq;
        }
    }

    pub(crate) fn bump_uid_validity(&mut self) {
        self.uid_validity += 1;
    }
}

impl MailServer for FakeServer {
    fn select(&mut self, _mailbox: &str) -> Result<MailboxSnapshot, Error> {
        Ok(MailboxSnapshot {
            uid_validity: self.uid_validity,
            highest_modseq: self.condstore.then_some(self.modseq),
        })
    }

    fn list_uids(&mut self, _mailbox: &str) -> Result<Vec<Uid>, Error> {
        Ok(self.messages.keys().copied().collect())
    }

    fn fetch_envelopes(&mut self, _mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error> {
        self.fetch_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| self.messages.get(uid))
            .map(|(envelope, _)| envelope.clone())
            .collect())
    }

    fn changes_since(
        &mut self,
        _mailbox: &str,
        modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, Error> {
        if !self.condstore {
            return Ok(None);
        }
        Ok(Some(
            self.messages
                .values()
                .filter(|(_, m)| *m > modseq)
                .map(|(envelope, _)| envelope.clone())
                .collect(),
        ))
    }

    fn fetch_body_html(&mut self, _mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        self.body_fetches += 1;
        Ok(self.bodies.get(&uid).cloned())
    }

    fn set_seen(&mut self, _mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error> {
        if self.flags_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.set_seen_calls.push((uid, seen));
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.seen = seen;
            *modseq = self.modseq;
        }
        Ok(())
    }
}
