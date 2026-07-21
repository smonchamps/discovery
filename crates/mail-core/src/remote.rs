//! Le « port » réseau du moteur : la seule frontière abstraite de `mail-core`.
//!
//! Le moteur de synchro ne connaît ni IMAP, ni OAuth, ni TLS — uniquement ce
//! trait. L'adaptateur IMAP réel l'implémentera (module protocoles) ; les
//! tests utilisent un serveur simulé qui rejoue les bizarreries du terrain.

use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;

/// Ce qu'un corps rapatrié rapporte : le HTML à afficher, et la
/// description des fichiers qu'il transporte.
///
/// Les deux voyagent ENSEMBLE parce qu'ils se lisent dans les mêmes
/// octets. Redemander les pièces jointes séparément coûterait un second
/// téléchargement complet du message pour une information déjà passée
/// sous les yeux de l'adaptateur.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchedBody {
    pub html: String,
    pub attachments: Vec<Attachment>,
}

impl FetchedBody {
    /// Corps sans pièce jointe — le cas courant, et tout ce dont les
    /// tests du moteur ont besoin.
    pub fn html(html: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            attachments: Vec::new(),
        }
    }
}

/// État d'une boîte au moment de sa sélection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxSnapshot {
    /// Change quand le serveur invalide tous les UIDs connus → resynchro complète.
    pub uid_validity: u32,
    /// `Some` si le serveur supporte CONDSTORE (décision gelée : PHASE0.md §2.2).
    pub highest_modseq: Option<u64>,
}

/// Un dossier du serveur, sous ses DEUX noms.
///
/// `wire` est celui du protocole (UTF-7 modifié) : c'est lui qu'on
/// renvoie au serveur, et lui qu'on journalise. `display` est sa forme
/// lisible. Les confondre casse soit l'affichage, soit le SELECT — ils
/// coexistent donc explicitement plutôt que par convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub wire: String,
    pub display: String,
    /// Le dossier peut-il recevoir un message déplacé ?
    ///
    /// Faux pour les conteneurs qui ne portent pas de courrier
    /// (attribut `\Noselect`) : les proposer produirait un échec au clic.
    pub selectable: bool,
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

    /// Corps d'un message, prêt à assainir (l'extraction MIME est la
    /// responsabilité de l'adaptateur). `None` si le message n'existe plus.
    fn fetch_body_html(&mut self, mailbox: &str, uid: Uid) -> Result<Option<FetchedBody>, Error>;

    /// Corps de PLUSIEURS messages en une seule commande. Les UIDs que le
    /// serveur ne sert plus sont simplement absents du résultat.
    ///
    /// Volontairement sans implémentation par défaut : un repli qui
    /// boucherait sur [`Self::fetch_body_html`] serait silencieusement
    /// ruineux. Un aller-retour par message coûte ~192 ms sur un serveur
    /// réel (`spikes/body-backfill`) — rattraper une boîte entière n'est
    /// tenable qu'en groupant, et chaque adaptateur doit le dire
    /// explicitement.
    fn fetch_bodies_html(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error>;

    /// Les OCTETS d'une pièce jointe, désignée par son rang dans le
    /// message. `None` si le message ou la pièce n'existe plus.
    ///
    /// Séparé du corps à dessein : les métadonnées sont gratuites et
    /// stockées, les octets se paient à la demande et ne sont jamais
    /// gardés. C'est ce qui laisse intact le budget disque de l'ADR 0007
    /// — y ajouter les fichiers le ferait exploser.
    fn fetch_attachment(
        &mut self,
        mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error>;

    /// Applique (ou retire) le flag `\Seen` côté serveur.
    fn set_seen(&mut self, mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error>;

    /// Applique (ou retire) le flag `\Flagged` — l'étoile.
    fn set_flagged(&mut self, mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error>;

    /// Sort le message de la boîte sans le supprimer (archivage).
    fn archive(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;

    /// Met le message à la corbeille du serveur.
    fn delete(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;

    /// Les dossiers du compte, tels que l'utilisateur peut les choisir.
    fn folders(&mut self) -> Result<Vec<Folder>, Error>;

    /// Déplace le message vers `target`, désigné par son nom RÉSEAU.
    ///
    /// L'opération doit être **atomique du point de vue du message** :
    /// il ne doit jamais pouvoir disparaître de la source sans être
    /// arrivé à destination. Même règle d'or que la boîte d'envoi,
    /// appliquée au tri.
    fn move_to(&mut self, mailbox: &str, uid: Uid, target: &str) -> Result<(), Error>;
}
