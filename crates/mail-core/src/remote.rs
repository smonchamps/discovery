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

/// Les destinataires (À / Cc) d'un message, adresses brutes.
///
/// L'enveloppe stockée ne porte que l'expéditeur : « Répondre à tous »
/// relit donc ces listes dans l'ENVELOPE du serveur au moment du clic —
/// un aller-retour à la demande, pas un octet de plus en base.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageRecipients {
    pub to: Vec<String>,
    pub cc: Vec<String>,
}

/// Les en-têtes qui rattachent un message à sa conversation.
///
/// `None` et `Some("")` ne disent PAS la même chose : le premier signifie
/// « pas encore lu », le second « lu, et le message n'en a pas ». Confondre
/// les deux ferait redemander éternellement les mêmes messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadHeaders {
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
}

/// Un brouillon lu dans le dossier Brouillons du serveur.
///
/// Le corps arrive sous les deux formes que MIME peut porter, sans qu'on
/// choisisse ici : convertir du HTML en texte est un travail de rendu, et
/// ce type est une frontière réseau. La couche qui sait rendre tranche.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteDraft {
    /// Champ « À » tel quel : un brouillon a le droit d'être incomplet,
    /// c'est même sa raison d'être.
    pub to_raw: String,
    pub subject: String,
    /// Partie `text/plain`, quand il y en a une.
    pub text: Option<String>,
    /// Partie `text/html` — souvent la seule d'un brouillon composé dans
    /// un webmail.
    pub html: Option<String>,
}

/// État d'une boîte au moment de sa sélection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxSnapshot {
    /// Change quand le serveur invalide tous les UIDs connus → resynchro complète.
    pub uid_validity: u32,
    /// `Some` si le serveur supporte CONDSTORE (décision gelée : PHASE0.md §2.2).
    pub highest_modseq: Option<u64>,
    /// Combien de messages le serveur annonce dans cette boîte (EXISTS).
    ///
    /// Gratuit : la réponse SELECT le porte toujours, on le jetait. C'est
    /// le **dénominateur** de l'avancement de la synchronisation intégrale
    /// ([ADR 0010](../../../docs/adr/0010-synchronisation-integrale.md) §5)
    /// — sans lui, « 12 000 messages récupérés » ne dit pas si on en est au
    /// dixième ou à la fin.
    pub exists: u32,
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

/// Le relevé STATUS d'un dossier, sans sélection (ADR 0017).
///
/// `uid_next` et `uid_validity` sont optionnels parce que RFC 3501 ne
/// force pas un serveur à les servir : leur absence rend `faut_relever`
/// conservatrice — on relève — jamais fausse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderStatus {
    /// Messages annoncés (EXISTS).
    pub messages: u32,
    pub uid_next: Option<u32>,
    pub uid_validity: Option<u32>,
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

    /// Les en-têtes de fil de PLUSIEURS messages, en une commande.
    ///
    /// Séparé de l'ENVELOPE **par une mesure** : celle-ci porte
    /// `In-Reply-To` mais pas `References` (RFC 3501 §7.4.2), et obtenir
    /// `References` impose de lire le bloc d'en-têtes complet — dix fois
    /// plus gros qu'une enveloppe. L'ajouter à la synchronisation
    /// décuplerait le coût de « enveloppes d'abord » ; ces en-têtes sont
    /// donc rapatriés APRÈS, en tâche de fond.
    ///
    /// Or `References` n'est pas un raffinement : dans une boîte de
    /// réception, le message intermédiaire d'un échange est celui qu'on a
    /// soi-même envoyé, et il n'y figure pas. Sans lui, la moitié des
    /// conversations reste coupée en deux.
    fn fetch_thread_headers(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error>;

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

    /// Les destinataires (À / Cc) d'un message — « Répondre à tous ».
    /// `None` si le message n'existe plus sur le serveur.
    fn fetch_recipients(
        &mut self,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<MessageRecipients>, Error>;

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

    /// Le relevé d'un dossier — SANS le sélectionner.
    ///
    /// Un seul aller-retour (STATUS en IMAP, prévu exactement pour
    /// interroger une boîte non sélectionnée) qui sert DEUX décisions :
    /// la garde d'espace disque ([ADR 0010](../../../docs/adr/0010-synchronisation-integrale.md)
    /// §4) qui somme `messages` AVANT de s'engager, et la relève gardée
    /// ([ADR 0017](../../../docs/adr/0017-releve-gardee-par-status.md)) —
    /// `faut_relever` saute les dossiers où rien n'a bougé. `uid_next`
    /// et `uid_validity` sont optionnels : un serveur qui les tait rend
    /// la décision conservatrice (on relève), jamais fausse.
    fn folder_status(&mut self, mailbox: &str) -> Result<FolderStatus, Error>;

    /// Déplace le message vers `target`, désigné par son nom RÉSEAU.
    ///
    /// L'opération doit être **atomique du point de vue du message** :
    /// il ne doit jamais pouvoir disparaître de la source sans être
    /// arrivé à destination. Même règle d'or que la boîte d'envoi,
    /// appliquée au tri.
    fn move_to(&mut self, mailbox: &str, uid: Uid, target: &str) -> Result<(), Error>;
}
