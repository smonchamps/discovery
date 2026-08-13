//! Adaptateur IMAP : la première implémentation réelle de
//! [`mail_core::MailServer`].
//!
//! Le noyau ne connaît que le trait ; ce crate traduit ses quatre opérations
//! en commandes IMAP (crate `imap`) et les réponses serveur en types du
//! domaine. Un crate par protocole : SMTP et Graph auront les leurs.
//!
//! CONDSTORE (RFC 7162) est câblé depuis E2b (PLAN-SYNCHRO) : quand le
//! serveur l'annonce, `changes_since` sert le delta (drapeaux compris) via
//! `UID FETCH … CHANGEDSINCE`, et HIGHESTMODSEQ est relevé au SELECT comme
//! au STATUS. Sans l'annonce, `None` — le moteur bascule sur le
//! différentiel d'UIDs, chemin complet et testé.

mod convert;
mod mutf7;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use imap_proto::NameAttribute;
use imap_proto::types::UidSetMember;
use mail_core::{
    Envelope, Error, FetchedBody, MailServer, MailboxSnapshot, MessageRecipients, RemoteDraft,
    ThreadHeaders, Uid,
};

/// Les délais du cycle (P0, PLAN-SYNCHRO) : sans eux, un réseau qui cale
/// en plein FETCH gelait la relève sans fin ni erreur — et la garde de
/// réentrance de l'UI sautait tous les cycles suivants, jusqu'au
/// redémarrage. Provisoire assumé : le spike IDLE (E4) re-instruira ces
/// valeurs, une lecture IDLE étant longue par nature.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 120 s, le haut de la fourchette du plan (60–120 s), et pas moins : le
/// délai court par `read()`, pas par commande — il ne tombe que si PLUS
/// RIEN n'arrive. Un serveur qui rumine un SEARCH sur une grosse boîte
/// peut se taire des dizaines de secondes avant son premier octet.
const IO_TIMEOUT: Duration = Duration::from_secs(120);

/// Ouvre la connexion IMAP avec les délais posés AVANT le premier octet :
/// TCP borné, timeouts de lecture ET d'écriture sur la socket, puis TLS
/// direct sur 993 et STARTTLS obligatoire ailleurs — le comportement
/// exact du `ClientBuilder` (mode AutoTls), qui lui ne borne rien : c'est
/// toute la raison de construire à la main.
fn connect_client(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    io_timeout: Duration,
) -> Result<imap::Client<imap::Connection>, Error> {
    let contexte = |err: String| Error::Server(format!("connexion {host}:{port} : {err}"));
    // La résolution peut rendre plusieurs adresses (IPv4/IPv6) : chacune
    // a droit au même délai, la première qui répond gagne.
    let mut tcp: Option<TcpStream> = None;
    let mut dernier: Option<std::io::Error> = None;
    for addr in (host, port)
        .to_socket_addrs()
        .map_err(|err| contexte(err.to_string()))?
    {
        match TcpStream::connect_timeout(&addr, connect_timeout) {
            Ok(stream) => {
                tcp = Some(stream);
                break;
            }
            Err(err) => dernier = Some(err),
        }
    }
    let mut tcp = match (tcp, dernier) {
        (Some(tcp), _) => tcp,
        (None, Some(err)) => return Err(contexte(err.to_string())),
        (None, None) => return Err(contexte("adresse introuvable".to_string())),
    };
    tcp.set_read_timeout(Some(io_timeout))
        .map_err(|err| contexte(err.to_string()))?;
    tcp.set_write_timeout(Some(io_timeout))
        .map_err(|err| contexte(err.to_string()))?;

    let connector = native_tls::TlsConnector::new().map_err(|err| contexte(err.to_string()))?;
    if port == 993 {
        let tls = connector
            .connect(host, tcp)
            .map_err(|err| contexte(err.to_string()))?;
        let mut client = imap::Client::new(Box::new(tls) as imap::Connection);
        client.read_greeting().map_err(server_err)?;
        Ok(client)
    } else {
        // STARTTLS : salutation puis montée en TLS OBLIGATOIRE — un
        // serveur qui la refuse est refusé, jamais de session en clair
        // (même exigence que le mode AutoTls remplacé). Négocié À LA
        // MAIN sur la socket : l'API publique de la crate ne sait pas
        // émettre STARTTLS avant authentification. Deux lignes de
        // protocole, bornées par les mêmes délais que tout le reste.
        lire_ligne(&mut tcp).map_err(|err| contexte(err.to_string()))?; // salutation
        tcp.write_all(b"a1 STARTTLS\r\n")
            .map_err(|err| contexte(err.to_string()))?;
        loop {
            let ligne = lire_ligne(&mut tcp).map_err(|err| contexte(err.to_string()))?;
            let Some(reponse) = ligne.strip_prefix("a1 ") else {
                continue; // ligne non étiquetée (« * … ») : on lit la suite
            };
            if reponse.starts_with("OK") {
                break;
            }
            return Err(contexte(format!("STARTTLS refusé : {}", ligne.trim_end())));
        }
        let tls = connector
            .connect(host, tcp)
            .map_err(|err| contexte(err.to_string()))?;
        // Pas de salutation à lire : elle a été consommée en clair, le
        // serveur n'en renvoie pas après STARTTLS (RFC 3501 §6.2.1).
        Ok(imap::Client::new(Box::new(tls) as imap::Connection))
    }
}

/// Lit UNE ligne terminée par `\n` sur la socket, bornée à 8 Kio : le
/// pré-TLS d'IMAP tient en deux lignes courtes, tout dépassement est
/// suspect. Octet par octet, et c'est voulu — aucun tampon ne doit
/// avaler les premiers octets de la poignée de main TLS qui suit.
fn lire_ligne(tcp: &mut TcpStream) -> std::io::Result<String> {
    let mut ligne = Vec::new();
    let mut octet = [0u8; 1];
    loop {
        if tcp.read(&mut octet)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connexion fermée avant la fin de ligne",
            ));
        }
        ligne.push(octet[0]);
        if octet[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&ligne).into_owned());
        }
        if ligne.len() > 8 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ligne de protocole démesurée avant TLS",
            ));
        }
    }
}

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
    /// Le dossier des envois, mémorisé pour la session. Deux niveaux
    /// d'option, et ils disent deux choses différentes : `None` = pas
    /// encore cherché, `Some(None)` = cherché, ce serveur n'en a pas.
    /// Les confondre ferait relister à chaque synchronisation.
    sent: Option<Option<String>>,
    /// Le serveur annonce-t-il MOVE (RFC 6851) ? Mémorisé : la capacité
    /// ne change pas en cours de session.
    supports_move: Option<bool>,
    /// Le serveur annonce-t-il CONDSTORE (RFC 7162) ? Même discipline.
    supports_condstore: Option<bool>,
    /// Le serveur annonce-t-il LIST-STATUS (RFC 5819) ? Même discipline —
    /// c'est lui qui fond ~51 STATUS en un aller-retour (terrain
    /// 2026-08-13 : l'inventaire, dernier goulot du cycle sobre).
    supports_list_status: Option<bool>,
}

impl ImapServer {
    /// Connexion TLS + authentification XOAUTH2 avec un access token OAuth2.
    /// Délais posés à la socket (P0) : un réseau qui cale est une erreur,
    /// jamais un gel.
    pub fn connect_xoauth2(
        host: &str,
        port: u16,
        user: &str,
        access_token: &str,
    ) -> Result<Self, Error> {
        let client = connect_client(host, port, CONNECT_TIMEOUT, IO_TIMEOUT)?;
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
            sent: None,
            supports_move: None,
            supports_condstore: None,
            supports_list_status: None,
        })
    }

    /// Connexion TLS + authentification par mot de passe (IMAP générique).
    /// Mêmes délais que la voie OAuth (P0).
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, Error> {
        let client = connect_client(host, port, CONNECT_TIMEOUT, IO_TIMEOUT)?;
        let session = client
            .login(user, password)
            .map_err(|(err, _)| server_err(err))?;
        Ok(Self {
            session,
            selected: None,
            trash: None,
            drafts: None,
            archive: None,
            sent: None,
            supports_move: None,
            supports_condstore: None,
            supports_list_status: None,
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
            // OK [HIGHESTMODSEQ] accompagne le SELECT sur tout serveur
            // CONDSTORE (RFC 7162 §3.1.2) ; absent = pas de CONDSTORE,
            // le moteur garde le différentiel d'UIDs.
            highest_modseq: selected.highest_mod_seq,
            exists: selected.exists,
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
    ///
    /// `None` quand le serveur n'annonce pas l'attribut : un IMAP
    /// générique peut n'en exposer aucun. **Ce n'est pas une panne, c'est
    /// une capacité absente** — et la traiter comme une erreur ferait
    /// répéter le même message à chaque synchronisation, jusqu'à ce que
    /// le bilan ne veuille plus rien dire.
    pub fn drafts_folder_name(&mut self) -> Result<Option<String>, Error> {
        if let Some(name) = &self.drafts {
            return Ok(Some(name.clone()));
        }
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        let drafts = names
            .iter()
            .find(|name| {
                name.attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::Drafts))
            })
            .map(|name| name.name().to_string());
        self.drafts = drafts.clone();
        Ok(drafts)
    }

    /// Le dossier Brouillons, ou une erreur — pour les chemins que
    /// l'utilisateur a explicitement demandés (pousser, purger), où son
    /// absence doit se dire.
    fn drafts_folder(&mut self) -> Result<String, Error> {
        self.drafts_folder_name()?
            .ok_or_else(|| Error::Server("dossier brouillons introuvable (RFC 6154)".to_string()))
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

    /// Le dossier où CE serveur range nos messages partis, s'il en a un.
    ///
    /// `None` n'est pas une panne, c'est une capacité absente — même
    /// discipline que [`Self::drafts_folder_name`]. Sans lui, les
    /// conversations ne regroupent que les messages reçus, exactement
    /// comme avant l'[ADR 0009] ; la synchronisation continue.
    pub fn sent_folder_name(&mut self) -> Result<Option<String>, Error> {
        if let Some(known) = &self.sent {
            return Ok(known.clone());
        }
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        let folders: Vec<(&str, convert::SpecialUse)> = names
            .iter()
            .map(|name| {
                let role = if name
                    .attributes()
                    .iter()
                    .any(|attribute| matches!(attribute, NameAttribute::Sent))
                {
                    convert::SpecialUse::Sent
                } else {
                    convert::SpecialUse::Other
                };
                (name.name(), role)
            })
            .collect();
        let found = convert::sent_folder(folders);
        self.sent = Some(found.clone());
        Ok(found)
    }

    /// UIDVALIDITY du dossier Brouillons — la garde des repères distants :
    /// si elle change, les UIDs enregistrés ne veulent plus rien dire.
    pub fn drafts_uidvalidity(&mut self) -> Result<u32, Error> {
        let folder = self.drafts_folder()?;
        Ok(self.ensure_selected(&folder)?.uid_validity)
    }

    /// Les UIDs présents dans le dossier Brouillons du serveur.
    ///
    /// C'est la moitié « tirage » de la synchronisation des brouillons :
    /// jusqu'ici on ne faisait que pousser, et un brouillon commencé
    /// ailleurs restait invisible ici.
    pub fn draft_uids(&mut self) -> Result<Vec<Uid>, Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        let uids = self.session.uid_search("ALL").map_err(server_err)?;
        Ok(uids.into_iter().collect())
    }

    /// Rapatrie un brouillon du serveur. `None` s'il a disparu entre la
    /// liste et la lecture — course banale, et sans conséquence.
    ///
    /// `PEEK` : lire un brouillon ne doit pas le marquer lu.
    pub fn fetch_draft(&mut self, uid: Uid) -> Result<Option<RemoteDraft>, Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| convert::draft_from_raw(fetch.body()?)))
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
    /// Le serveur sait-il faire MOVE (RFC 6851) ?
    fn supports_move(&mut self) -> Result<bool, Error> {
        if let Some(known) = self.supports_move {
            return Ok(known);
        }
        let capabilities = self.session.capabilities().map_err(server_err)?;
        let supported = capabilities.has_str("MOVE");
        self.supports_move = Some(supported);
        Ok(supported)
    }

    /// Le serveur sait-il faire CONDSTORE (RFC 7162) ? Décide du STATUS
    /// enrichi (HIGHESTMODSEQ) et du delta `changes_since` — demander
    /// l'un ou l'autre à un serveur qui ne l'annonce pas serait un BAD.
    fn supports_condstore(&mut self) -> Result<bool, Error> {
        if let Some(known) = self.supports_condstore {
            return Ok(known);
        }
        let capabilities = self.session.capabilities().map_err(server_err)?;
        let supported = capabilities.has_str("CONDSTORE");
        self.supports_condstore = Some(supported);
        Ok(supported)
    }

    /// Le serveur sait-il faire LIST-STATUS (RFC 5819) ? Décide de
    /// l'inventaire en un aller-retour au lieu de ~51 STATUS. Émettre
    /// `LIST … RETURN (STATUS …)` à un serveur qui ne l'annonce pas
    /// serait un BAD — d'où la garde.
    fn supports_list_status(&mut self) -> Result<bool, Error> {
        if let Some(known) = self.supports_list_status {
            return Ok(known);
        }
        let capabilities = self.session.capabilities().map_err(server_err)?;
        let supported = capabilities.has_str("LIST-STATUS");
        self.supports_list_status = Some(supported);
        Ok(supported)
    }

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

    /// `BODY.PEEK[HEADER]` — le bloc d'en-têtes ENTIER, faute de mieux :
    /// la crate `imap` n'expose `header()` que pour cette section-là, et
    /// pas pour `HEADER.FIELDS (REFERENCES)`, qui serait vingt fois plus
    /// petite. L'écart est assumé parce que la passe est bornée et ne
    /// repasse jamais sur un message déjà lu.
    ///
    /// `PEEK` : lire des en-têtes ne doit pas davantage poser `\Seen` que
    /// lire un corps.
    fn fetch_thread_headers(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(convert::uid_set(uids), "(UID BODY.PEEK[HEADER])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .filter_map(|fetch| Some((fetch.uid?, convert::thread_headers(fetch.header()?))))
            .collect())
    }

    fn changes_since(
        &mut self,
        mailbox: &str,
        modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, Error> {
        // Sans l'annonce CONDSTORE, `None` : le moteur bascule sur le
        // différentiel d'UIDs — chemin complet, testé, d'avant E2b.
        if !self.supports_condstore()? {
            return Ok(None);
        }
        self.ensure_selected(mailbox)?;
        // Tout ce qui a bougé depuis le repère — nouveaux messages ET
        // drapeaux (le reflet qui manquait : un mail lu au téléphone
        // restait non-lu ici). CHANGEDSINCE est lui-même une commande
        // « CONDSTORE enabling » (RFC 7162 §3.1) : rien à négocier avant.
        let fetches = self
            .session
            .uid_fetch(
                "1:*",
                format!("(UID ENVELOPE INTERNALDATE FLAGS) (CHANGEDSINCE {modseq})"),
            )
            .map_err(server_err)?;
        Ok(Some(
            fetches
                .iter()
                .filter_map(convert::fetch_to_envelope)
                .collect(),
        ))
    }

    fn fetch_body_html(&mut self, mailbox: &str, uid: Uid) -> Result<Option<FetchedBody>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| body_from_raw(fetch.body()?)))
    }

    /// Une SEULE commande `UID FETCH` pour tout le lot — c'est ce qui rend
    /// le rattrapage des corps tenable (un aller-retour par message coûte
    /// ~192 ms sur un serveur réel, cf. `spikes/body-backfill`).
    ///
    /// `BODY.PEEK[]` : lire un corps ne doit jamais poser `\Seen`. Les UIDs
    /// que le serveur ne sert plus sont simplement absents du résultat.
    fn fetch_bodies_html(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(convert::uid_set(uids), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .filter_map(|fetch| {
                let uid = fetch.uid?;
                Some((uid, body_from_raw(fetch.body()?)?))
            })
            .collect())
    }

    /// Retélécharge le message pour en extraire UNE pièce, plutôt que de
    /// demander la partie au serveur (`BODY[2.1.3]`).
    ///
    /// C'est un choix, et il se paie : un aller-retour complet (~192 ms
    /// mesurés) là où un FETCH de partie serait plus léger. En échange,
    /// aucun numéro de partie MIME n'est jamais calculé — l'arithmétique
    /// des parties imbriquées est une source de bugs classique, et le
    /// rang reste celui qu'a produit l'extraction locale, donc cohérent
    /// avec ce qui est affiché. À revoir si les gros fichiers gênent.
    fn fetch_attachment(
        &mut self,
        mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| convert::attachment_bytes(fetch.body()?, index)))
    }

    /// Relit l'ENVELOPE du message pour en tirer À et Cc : l'enveloppe
    /// stockée localement ne les porte pas, « Répondre à tous » les
    /// demande au moment du clic — un aller-retour à la demande, pas un
    /// octet de plus en base ni dans la synchro « enveloppes d'abord ».
    fn fetch_recipients(
        &mut self,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<MessageRecipients>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID ENVELOPE)")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| Some(convert::envelope_recipients(fetch.envelope()?))))
    }

    fn folders(&mut self) -> Result<Vec<mail_core::Folder>, Error> {
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        Ok(names.iter().map(name_to_folder).collect())
    }

    fn folders_with_status(&mut self) -> Result<Option<Vec<mail_core::FolderWithStatus>>, Error> {
        if !self.supports_list_status()? {
            return Ok(None);
        }
        // HIGHESTMODSEQ dans le RETURN quand CONDSTORE est là (E2b),
        // comme `folder_status` : le même relevé sert la relève gardée.
        let items = if self.supports_condstore()? {
            "(MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ)"
        } else {
            "(MESSAGES UIDNEXT UIDVALIDITY)"
        };
        let noms = self
            .session
            .list_status(None, Some("*"), items)
            .map_err(server_err)?;
        Ok(Some(
            noms.iter()
                .map(|(name, mailbox)| {
                    // Le STATUS reste OPTIONNEL dans la réponse : RFC 5819
                    // §2 autorise le serveur à l'omettre s'il bute sur un
                    // dossier — l'appelant traite alors ce dossier comme
                    // non gardé (il le relèvera, prudence d'ADR 0017).
                    let statut = mailbox.as_ref().map(|mb| mail_core::FolderStatus {
                        messages: mb.exists,
                        uid_next: mb.uid_next,
                        uid_validity: mb.uid_validity,
                        highest_modseq: mb.highest_mod_seq,
                    });
                    (name_to_folder(name), statut)
                })
                .collect(),
        ))
    }

    fn folder_status(&mut self, mailbox: &str) -> Result<mail_core::FolderStatus, Error> {
        // STATUS et non SELECT : la commande est faite pour interroger une
        // boîte NON sélectionnée (RFC 3501 §6.3.10) — la sélection
        // courante du moteur n'est pas perturbée, et certains serveurs
        // font payer un SELECT bien plus cher qu'un STATUS. Un seul
        // aller-retour pour la garde d'espace ET la relève gardée
        // (ADR 0017).
        // HIGHESTMODSEQ rejoint le relevé quand le serveur sait CONDSTORE
        // (E2b) : c'est lui qui réveille un dossier dont seuls les
        // drapeaux ont glissé. Le demander à un serveur qui ne l'annonce
        // pas serait un BAD — d'où la garde.
        let items = if self.supports_condstore()? {
            "(MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ)"
        } else {
            "(MESSAGES UIDNEXT UIDVALIDITY)"
        };
        let status = self.session.status(mailbox, items).map_err(server_err)?;
        Ok(mail_core::FolderStatus {
            messages: status.exists,
            uid_next: status.uid_next,
            uid_validity: status.uid_validity,
            highest_modseq: status.highest_mod_seq,
        })
    }

    /// MOVE si le serveur l'annonce, COPY + EXPUNGE sinon.
    ///
    /// Le repli n'est pas équivalent, et l'écart mérite d'être nommé :
    /// entre le COPY et l'EXPUNGE existe une fenêtre où une coupure
    /// laisse le message dans les DEUX dossiers. C'est un doublon, pas
    /// une perte — et l'ordre choisi garantit que ce sera toujours dans
    /// ce sens-là. Copier d'abord, ne retirer qu'ensuite : « jamais de
    /// perte de mail » (PLAN.md §1) prime sur la propreté.
    fn move_to(&mut self, mailbox: &str, uid: Uid, target: &str) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        if self.supports_move()? {
            return self
                .session
                .uid_mv(uid.to_string(), target)
                .map_err(server_err);
        }
        self.session
            .uid_copy(uid.to_string(), target)
            .map_err(server_err)?;
        self.expunge_uid(uid)
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

/// Un `Name` IMAP (issu de LIST ou LIST-STATUS) devient un `Folder` du
/// domaine — mapping unique, partagé par `folders()` et
/// `folders_with_status()` pour qu'ils ne puissent pas diverger.
fn name_to_folder(name: &imap::types::Name<'_>) -> mail_core::Folder {
    mail_core::Folder {
        wire: name.name().to_string(),
        // Décodé pour l'œil SEULEMENT : `wire` reste ce qu'on renvoie
        // au serveur (RFC 3501 §5.1.3).
        display: mutf7::decode(name.name()),
        // `\Noselect` marque un conteneur sans courrier : le proposer
        // comme destination produirait un échec au clic.
        selectable: !name
            .attributes()
            .iter()
            .any(|attribute| matches!(attribute, NameAttribute::NoSelect)),
    }
}

/// L'échec vient-il de la CONNEXION (résolution, TCP, TLS, délais P0)
/// plutôt que de la suite (authentification, protocole) ?
///
/// Le shell s'en sert pour ne PAS rafraîchir un jeton OAuth sur une
/// panne réseau : un câble coupé n'est pas un jeton mort, et marteler
/// l'endpoint de jetons à chaque cycle en panne est le meilleur moyen
/// de transformer un bridage IMAP en gel du compte.
///
/// Le contrat vit ICI, à côté du format qu'il inspecte : toute erreur
/// de [`connect_client`] est préfixée « connexion hôte:port : … » —
/// c'est ce préfixe qui fait foi.
pub fn is_connection_error(err: &Error) -> bool {
    matches!(err, Error::Server(msg) if msg.starts_with("connexion "))
}

/// Un message brut devient un corps affichable ET la description de ses
/// pièces jointes — les deux se lisent dans les mêmes octets, il serait
/// absurde de les redemander séparément.
fn body_from_raw(raw: &[u8]) -> Option<FetchedBody> {
    Some(FetchedBody {
        html: convert::extract_html(raw)?,
        attachments: convert::extract_attachments(raw),
    })
}

#[cfg(test)]
mod connect_timeout_tests {
    use super::connect_client;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    /// LE contrat de P0 (PLAN-SYNCHRO) : un serveur qui accepte la
    /// connexion puis se tait pour toujours doit devenir une ERREUR
    /// bornée. Avant P0, `ClientBuilder::connect()` lisait la salutation
    /// sans délai : le cycle entier gelait, en silence, et la garde de
    /// réentrance de l'UI interdisait tout cycle suivant — plus aucun
    /// courrier jusqu'au redémarrage.
    #[test]
    fn un_serveur_muet_echoue_au_lieu_de_geler() {
        // Un vrai listener jamais servi : la poignée de main TCP aboutit
        // (backlog du noyau), puis plus rien — le réseau qui cale.
        let silencieux = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silencieux.local_addr().unwrap().port();

        let depart = Instant::now();
        let resultat = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_millis(200),
        );

        assert!(
            resultat.is_err(),
            "un serveur muet ne doit pas rendre de client"
        );
        assert!(
            depart.elapsed() < Duration::from_secs(5),
            "l'échec doit venir du délai de lecture, pas d'un gel : {:?}",
            depart.elapsed()
        );
    }

    /// La montée en TLS est OBLIGATOIRE : un serveur qui refuse STARTTLS
    /// est refusé à son tour — jamais de session en clair (l'exigence du
    /// mode AutoTls que la connexion manuelle remplace). La ligne non
    /// étiquetée intercalée vérifie que la lecture saute les « * … »
    /// sans les confondre avec la réponse.
    #[test]
    fn starttls_refuse_est_une_erreur_franche() {
        let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let serveur = std::thread::spawn(move || {
            let (mut sock, _) = ecoute.accept().unwrap();
            sock.write_all(b"* OK pret\r\n").unwrap();
            let mut tampon = [0u8; 64];
            let _ = sock.read(&mut tampon).unwrap();
            sock.write_all(b"* CAPABILITY IMAP4rev1\r\na1 NO pas ici\r\n")
                .unwrap();
        });

        let resultat = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_secs(5),
        );
        serveur.join().unwrap();

        let erreur = resultat
            .expect_err("un refus de STARTTLS doit être une erreur")
            .to_string();
        assert!(erreur.contains("STARTTLS refusé"), "erreur : {erreur}");
    }

    /// Le contrat du discriminant : une erreur de CONNEXION se reconnaît
    /// (le shell ne rafraîchit pas un jeton OAuth sur une panne réseau),
    /// une erreur d'authentification ou de protocole, non.
    #[test]
    fn une_panne_reseau_se_distingue_d_un_refus_d_authentification() {
        let silencieux = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silencieux.local_addr().unwrap().port();
        let panne = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_millis(200),
        )
        .expect_err("serveur muet");
        assert!(super::is_connection_error(&panne), "panne : {panne}");

        let refus =
            mail_core::Error::Server("AUTHENTICATIONFAILED Invalid credentials".to_string());
        assert!(!super::is_connection_error(&refus));
    }
}
