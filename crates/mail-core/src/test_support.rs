//! Serveur simulé partagé par les tests : rejoue les scénarios du terrain —
//! ajouts, suppressions, flags, bascule d'UIDVALIDITY, corps de messages —
//! et journalise les appels pour vérifier l'ordre et le nombre d'accès.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};

use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::remote::{FetchedBody, MailServer, MailboxSnapshot, MessageRecipients, ThreadHeaders};

pub(crate) struct FakeServer {
    pub(crate) uid_validity: u32,
    pub(crate) condstore: bool,
    pub(crate) modseq: u64,
    pub(crate) messages: BTreeMap<Uid, (Envelope, u64)>,
    pub(crate) bodies: BTreeMap<Uid, String>,
    pub(crate) fetch_batches: Vec<Vec<Uid>>,
    /// Nombre d'inventaires d'UIDs payés (`UID SEARCH ALL` du réel) —
    /// la preuve qu'E2b ne les demande que si le décompte l'exige.
    pub(crate) uid_list_calls: usize,
    pub(crate) body_fetches: usize,
    /// Lots de corps demandés, dans l'ordre : c'est ce qui prouve que le
    /// rattrapage groupe au lieu d'enchaîner les allers-retours.
    pub(crate) body_batches: Vec<Vec<Uid>>,
    /// `References` servies par le serveur simulé, par UID.
    pub(crate) references: BTreeMap<Uid, String>,
    /// Destinataires (À / Cc) servis par le serveur simulé, par UID.
    pub(crate) recipients: BTreeMap<Uid, MessageRecipients>,
    /// Lots d'en-têtes demandés : la preuve que la passe groupe.
    pub(crate) header_batches: Vec<Vec<Uid>>,
    pub(crate) folders: Vec<crate::remote::Folder>,
    /// Déplacements reçus : (uid, dossier cible réseau).
    pub(crate) moved: Vec<(Uid, String)>,
    /// Octets servis pour (uid, rang) — les pièces jointes du simulateur.
    pub(crate) attachment_bytes: BTreeMap<(Uid, usize), Vec<u8>>,
    /// Journal des actions reçues, dans l'ordre (`seen:1:true`, `archive:2`…).
    pub(crate) action_calls: Vec<String>,
    /// Simule une coupure sur les actions (test « zéro perte »).
    pub(crate) actions_fail: bool,
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
            uid_list_calls: 0,
            body_fetches: 0,
            body_batches: Vec::new(),
            references: BTreeMap::new(),
            recipients: BTreeMap::new(),
            header_batches: Vec::new(),
            folders: Vec::new(),
            moved: Vec::new(),
            attachment_bytes: BTreeMap::new(),
            action_calls: Vec::new(),
            actions_fail: false,
        }
    }

    /// Pose les `References` que le serveur servira pour ce message —
    /// l'en-tête que l'ENVELOPE ne porte pas.
    pub(crate) fn set_references(&mut self, uid: Uid, references: &str) {
        self.references.insert(uid, references.to_string());
    }

    pub(crate) fn add(&mut self, uid: Uid, subject: &str) {
        self.modseq += 1;
        let envelope = Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("alice@example.com".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<fake-{uid}@example.com>")),
            in_reply_to: None,
            // La date suit l'UID : plus l'UID est grand, plus c'est récent.
            date: Some(
                Utc.timestamp_opt(1_700_000_000 + i64::from(uid), 0)
                    .unwrap(),
            ),
            seen: false,
            flagged: false,
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

    pub(crate) fn mark_flagged(&mut self, uid: Uid) {
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.flagged = true;
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
            // Le VRAI nombre de messages du décor, pas une constante : un
            // faux serveur qui annoncerait autre chose que ce qu'il sert
            // ferait passer des tests d'avancement sur un modèle faux.
            exists: self.messages.len() as u32,
        })
    }

    fn list_uids(&mut self, _mailbox: &str) -> Result<Vec<Uid>, Error> {
        self.uid_list_calls += 1;
        Ok(self.messages.keys().copied().collect())
    }

    fn folder_status(&mut self, _mailbox: &str) -> Result<crate::remote::FolderStatus, Error> {
        // Le VRAI état du décor, même règle que `exists` dans `select` :
        // annoncer autre chose que ce qu'on sert ferait passer des tests
        // de garde disque ou de relève gardée sur un modèle faux.
        Ok(crate::remote::FolderStatus {
            messages: self.messages.len() as u32,
            uid_next: Some(self.messages.keys().max().copied().unwrap_or(0) + 1),
            uid_validity: Some(self.uid_validity),
            highest_modseq: self.condstore.then_some(self.modseq),
        })
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

    fn fetch_body_html(&mut self, _mailbox: &str, uid: Uid) -> Result<Option<FetchedBody>, Error> {
        self.body_fetches += 1;
        Ok(self.bodies.get(&uid).map(FetchedBody::html))
    }

    fn fetch_bodies_html(
        &mut self,
        _mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error> {
        self.body_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| {
                self.bodies
                    .get(uid)
                    .map(|html| (*uid, FetchedBody::html(html)))
            })
            .collect())
    }

    /// Le serveur simulé rend les en-têtes qu'on lui a posés via
    /// [`Self::set_thread_headers`], et enregistre les lots demandés.
    fn fetch_thread_headers(
        &mut self,
        _mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error> {
        self.header_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| {
                self.messages.get(uid)?;
                Some((
                    *uid,
                    ThreadHeaders {
                        in_reply_to: None,
                        // Toujours `Some` : le serveur a répondu, même si
                        // le message n'a pas de `References`.
                        references: Some(self.references.get(uid).cloned().unwrap_or_default()),
                    },
                ))
            })
            .collect())
    }

    /// Le serveur simulé rend les destinataires posés dans `recipients`
    /// — vides par défaut : un message peut n'avoir ni À ni Cc lisibles.
    fn fetch_recipients(
        &mut self,
        _mailbox: &str,
        uid: Uid,
    ) -> Result<Option<MessageRecipients>, Error> {
        if !self.messages.contains_key(&uid) {
            return Ok(None);
        }
        Ok(Some(self.recipients.get(&uid).cloned().unwrap_or_default()))
    }

    /// Le serveur simulé sert les octets qu'on lui a posés — de quoi
    /// prouver le chemin sans jamais toucher au réseau.
    fn fetch_attachment(
        &mut self,
        _mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .attachment_bytes
            .get(&(uid, index))
            .map(|bytes| bytes.to_vec()))
    }

    fn folders(&mut self) -> Result<Vec<crate::remote::Folder>, Error> {
        Ok(self.folders.clone())
    }

    /// Le simulateur enregistre les déplacements demandés : c'est ce qui
    /// permet de prouver le REJEU sans réseau.
    fn move_to(&mut self, _mailbox: &str, uid: Uid, target: &str) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.moved.push((uid, target.to_string()));
        self.messages.remove(&uid);
        Ok(())
    }

    fn set_seen(&mut self, _mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.action_calls.push(format!("seen:{uid}:{seen}"));
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.seen = seen;
            *modseq = self.modseq;
        }
        Ok(())
    }

    fn set_flagged(&mut self, _mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.action_calls.push(format!("flag:{uid}:{flagged}"));
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.flagged = flagged;
            *modseq = self.modseq;
        }
        Ok(())
    }

    fn archive(&mut self, _mailbox: &str, uid: Uid) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.action_calls.push(format!("archive:{uid}"));
        self.messages.remove(&uid);
        self.modseq += 1;
        Ok(())
    }

    fn delete(&mut self, _mailbox: &str, uid: Uid) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("coupure simulée".to_string()));
        }
        self.action_calls.push(format!("delete:{uid}"));
        self.messages.remove(&uid);
        self.modseq += 1;
        Ok(())
    }
}
