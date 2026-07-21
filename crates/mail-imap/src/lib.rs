//! Adaptateur IMAP : la première implémentation réelle de
//! [`mail_core::MailServer`].
//!
//! Le noyau ne connaît que le trait ; ce crate traduit ses quatre opérations
//! en commandes IMAP (crate `imap`) et les réponses serveur en types du
//! domaine. Un crate par protocole : SMTP et Graph auront les leurs.
//!
//! CONDSTORE n'est pas encore câblé (`changes_since` → `None`) : le moteur
//! bascule sur le différentiel d'UIDs, chemin complet et testé. L'extension
//! arrivera ici même, sans toucher au moteur — c'est le rôle du trait.

mod convert;

use imap_proto::NameAttribute;
use imap_proto::types::UidSetMember;
use mail_core::{Envelope, Error, MailServer, MailboxSnapshot, Uid};

/// Chaîne SASL XOAUTH2 (Gmail, Microsoft) : jamais de mot de passe.
struct XOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

pub struct ImapServer {
    session: imap::Session<Box<dyn imap::ImapConnection>>,
    selected: Option<(String, MailboxSnapshot)>,
    trash: Option<String>,
    drafts: Option<String>,
    archive: Option<convert::ArchiveStrategy>,
}

impl ImapServer {
    /// Connexion TLS + authentification XOAUTH2 avec un access token OAuth2.
    pub fn connect_xoauth2(
        host: &str,
        port: u16,
        user: &str,
        access_token: &str,
    ) -> Result<Self, Error> {
        let client = imap::ClientBuilder::new(host, port)
            .connect()
            .map_err(server_err)?;
        let auth = XOAuth2 {
            user: user.to_string(),
            access_token: access_token.to_string(),
        };
        let session = client
            .authenticate("XOAUTH2", &auth)
            .map_err(|(err, _)| server_err(err))?;
        Ok(Self {
            session,
            selected: None,
            trash: None,
            drafts: None,
            archive: None,
        })
    }

    /// Connexion TLS + authentification par mot de passe (IMAP générique).
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, Error> {
        let client = imap::ClientBuilder::new(host, port)
            .connect()
            .map_err(server_err)?;
        let session = client
            .login(user, password)
            .map_err(|(err, _)| server_err(err))?;
        Ok(Self {
            session,
            selected: None,
            trash: None,
            drafts: None,
            archive: None,
        })
    }

    pub fn logout(mut self) {
        let _ = self.session.logout();
    }

    /// Sélectionne la boîte si elle ne l'est pas déjà (le moteur appelle
    /// `select` puis enchaîne les opérations sur la même boîte).
    fn ensure_selected(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error> {
        if let Some((name, snapshot)) = &self.selected
            && name == mailbox
        {
            return Ok(*snapshot);
        }
        let selected = self.session.select(mailbox).map_err(server_err)?;
        let snapshot = MailboxSnapshot {
            uid_validity: selected
                .uid_validity
                .ok_or_else(|| Error::Server(format!("UIDVALIDITY absent pour {mailbox}")))?,
            highest_modseq: None,
        };
        self.selected = Some((mailbox.to_string(), snapshot));
        Ok(snapshot)
    }

    /// Découvre le dossier corbeille via ses attributs RFC 6154 — jamais de
    /// nom en dur : « [Gmail]/Corbeille » sur un compte français, « Trash »
    /// ailleurs. Résultat mémorisé pour la session.
    fn trash_folder(&mut self) -> Result<String, Error> {
        if let Some(name) = &self.trash {
            return Ok(name.clone());
        }
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        let trash = names
            .iter()
            .find(|name| {
                name.attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::Trash))
            })
            .map(|name| name.name().to_string())
            .ok_or_else(|| Error::Server("dossier corbeille introuvable (RFC 6154)".to_string()))?;
        self.trash = Some(trash.clone());
        Ok(trash)
    }

    /// Découvre le dossier Brouillons via RFC 6154 — jamais de nom en dur,
    /// comme la corbeille. Mémorisé pour la session.
    fn drafts_folder(&mut self) -> Result<String, Error> {
        if let Some(name) = &self.drafts {
            return Ok(name.clone());
        }
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        let drafts = names
            .iter()
            .find(|name| {
                name.attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::Drafts))
            })
            .map(|name| name.name().to_string())
            .ok_or_else(|| {
                Error::Server("dossier brouillons introuvable (RFC 6154)".to_string())
            })?;
        self.drafts = Some(drafts.clone());
        Ok(drafts)
    }

    /// Ce qu'« archiver » veut dire sur CE serveur, déduit de ses dossiers
    /// spéciaux (RFC 6154) et mémorisé pour la session.
    fn archive_strategy(&mut self) -> Result<convert::ArchiveStrategy, Error> {
        if let Some(strategy) = &self.archive {
            return Ok(strategy.clone());
        }
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        let folders: Vec<(&str, convert::SpecialUse)> = names
            .iter()
            .map(|name| {
                let role = if name
                    .attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::Archive))
                {
                    convert::SpecialUse::Archive
                } else if name
                    .attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::All))
                {
                    convert::SpecialUse::All
                } else {
                    convert::SpecialUse::Other
                };
                (name.name(), role)
            })
            .collect();
        let strategy = convert::archive_strategy(folders);
        self.archive = Some(strategy.clone());
        Ok(strategy)
    }

    /// UIDVALIDITY du dossier Brouillons — la garde des repères distants :
    /// si elle change, les UIDs enregistrés ne veulent plus rien dire.
    pub fn drafts_uidvalidity(&mut self) -> Result<u32, Error> {
        let folder = self.drafts_folder()?;
        Ok(self.ensure_selected(&folder)?.uid_validity)
    }

    /// Pousse une copie de brouillon (`\Draft`) ; retourne son UID quand le
    /// serveur l'annonce (APPENDUID/UIDPLUS — Gmail le fait). Sans UID,
    /// la copie ne pourra pas être remplacée : doublon possible, assumé.
    pub fn append_draft(&mut self, message: &[u8]) -> Result<Option<Uid>, Error> {
        let folder = self.drafts_folder()?;
        let appended = self
            .session
            .append(&folder, message)
            .flag(imap::types::Flag::Draft)
            .finish()
            .map_err(server_err)?;
        let uid = appended.uids.and_then(|uids| {
            uids.into_iter().next().map(|member| match member {
                UidSetMember::Uid(uid) => uid,
                UidSetMember::UidRange(range) => *range.start(),
            })
        });
        Ok(uid)
    }

    /// Purge une copie distante de brouillon — uniquement des UIDs que le
    /// stockage a lui-même enregistrés (invariant anti-mauvaise-suppression).
    pub fn delete_draft_remote(&mut self, uid: Uid) -> Result<(), Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        self.expunge_uid(uid)
    }

    /// Marque `\Deleted` puis expunge le seul UID visé (UIDPLUS).
    fn expunge_uid(&mut self, uid: Uid) -> Result<(), Error> {
        self.session
            .uid_store(uid.to_string(), "+FLAGS.SILENT (\\Deleted)")
            .map_err(server_err)?;
        self.session
            .uid_expunge(uid.to_string())
            .map_err(server_err)?;
        Ok(())
    }
}

impl MailServer for ImapServer {
    fn select(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error> {
        // Re-sélection systématique : c'est le point de rafraîchissement
        // du snapshot (UIDVALIDITY) en début de synchro.
        self.selected = None;
        self.ensure_selected(mailbox)
    }

    fn list_uids(&mut self, mailbox: &str) -> Result<Vec<Uid>, Error> {
        self.ensure_selected(mailbox)?;
        let uids = self.session.uid_search("ALL").map_err(server_err)?;
        Ok(uids.into_iter().collect())
    }

    fn fetch_envelopes(&mut self, mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error> {
        self.ensure_selected(mailbox)?;
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let fetches = self
            .session
            .uid_fetch(convert::uid_set(uids), "(UID ENVELOPE INTERNALDATE FLAGS)")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .filter_map(convert::fetch_to_envelope)
            .collect())
    }

    fn changes_since(
        &mut self,
        _mailbox: &str,
        _modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, Error> {
        // CONDSTORE : optimisation à venir (PHASE0.md §2.2). `None` déclenche
        // le repli par différentiel d'UIDs du moteur.
        Ok(None)
    }

    fn fetch_body_html(&mut self, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| convert::extract_html(fetch.body()?)))
    }

    fn set_seen(&mut self, mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        let query = if seen {
            "+FLAGS.SILENT (\\Seen)"
        } else {
            "-FLAGS.SILENT (\\Seen)"
        };
        self.session
            .uid_store(uid.to_string(), query)
            .map_err(server_err)?;
        Ok(())
    }

    fn set_flagged(&mut self, mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        let query = if flagged {
            "+FLAGS.SILENT (\\Flagged)"
        } else {
            "-FLAGS.SILENT (\\Flagged)"
        };
        self.session
            .uid_store(uid.to_string(), query)
            .map_err(server_err)?;
        Ok(())
    }

    /// Archiver dépend des capacités du serveur, JAMAIS du fournisseur.
    ///
    /// Chez Gmail (`\All`), l'expunge d'INBOX ne retire que le libellé : le
    /// message survit dans « Tous les messages ». Sur un IMAP générique,
    /// le même expunge **détruirait** le message — il faut donc le déplacer
    /// vers `\Archive`. Sans l'un ni l'autre, on refuse : « jamais de perte
    /// de mail » (PLAN.md §1) prime sur la disponibilité de la fonction.
    fn archive(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error> {
        match self.archive_strategy()? {
            convert::ArchiveStrategy::MoveTo(folder) => {
                self.ensure_selected(mailbox)?;
                self.session
                    .uid_copy(uid.to_string(), &folder)
                    .map_err(server_err)?;
                self.expunge_uid(uid)
            }
            convert::ArchiveStrategy::ExpungeOnly => {
                self.ensure_selected(mailbox)?;
                self.expunge_uid(uid)
            }
            convert::ArchiveStrategy::Unsupported => Err(Error::Server(
                "ce serveur n'expose ni dossier Archive (\\Archive) ni « tous les messages » \
                 (\\All) : archiver y détruirait le message"
                    .to_string(),
            )),
        }
    }

    fn delete(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error> {
        let trash = self.trash_folder()?;
        self.ensure_selected(mailbox)?;
        self.session
            .uid_copy(uid.to_string(), &trash)
            .map_err(server_err)?;
        self.expunge_uid(uid)
    }
}

fn server_err(err: imap::Error) -> Error {
    Error::Server(err.to_string())
}
