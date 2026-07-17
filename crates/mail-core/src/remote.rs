//! Le « port » réseau du moteur : la seule frontière abstraite de `mail-core`.
//!
//! Le moteur de synchro ne connaît ni IMAP, ni OAuth, ni TLS — uniquement ce
//! trait. L'adaptateur IMAP réel l'implémentera (module protocoles) ; les
//! tests utilisent un serveur simulé qui rejoue les bizarreries du terrain.

use crate::envelope::{Envelope, Uid};
use crate::error::Error;

/// État d'une boîte au moment de sa sélection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxSnapshot {
    /// Change quand le serveur invalide tous les UIDs connus → resynchro complète.
    pub uid_validity: u32,
    /// `Some` si le serveur supporte CONDSTORE (décision gelée : PHASE0.md §2.2).
    pub highest_modseq: Option<u64>,
}

pub trait MailServer {
    /// Sélectionne une boîte et retourne son état courant.
    fn select(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error>;

    /// Tous les UIDs présents dans la boîte (ordre quelconque).
    fn list_uids(&mut self, mailbox: &str) -> Result<Vec<Uid>, Error>;

    /// Enveloppes des messages demandés ; les UIDs inconnus sont ignorés.
    fn fetch_envelopes(&mut self, mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error>;

    /// Messages nouveaux ou modifiés (flags) depuis `modseq` — CONDSTORE.
    /// Retourne `None` si le serveur ne supporte pas l'extension ; le moteur
    /// bascule alors sur la détection par différentiel d'UIDs.
    fn changes_since(&mut self, mailbox: &str, modseq: u64)
    -> Result<Option<Vec<Envelope>>, Error>;

    /// Corps HTML d'un message, prêt à assainir (l'extraction MIME est la
    /// responsabilité de l'adaptateur). `None` si le message n'existe plus.
    fn fetch_body_html(&mut self, mailbox: &str, uid: Uid) -> Result<Option<String>, Error>;

    /// Applique (ou retire) le flag `\Seen` côté serveur.
    fn set_seen(&mut self, mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error>;

    /// Applique (ou retire) le flag `\Flagged` — l'étoile.
    fn set_flagged(&mut self, mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error>;

    /// Sort le message de la boîte sans le supprimer (archivage).
    fn archive(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;

    /// Met le message à la corbeille du serveur.
    fn delete(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;
}
