//! Les actions utilisateur en attente — le deuxième cœur du produit.
//!
//! Chaque intention est appliquée localement tout de suite (optimisme UI),
//! journalisée dans SQLite, puis rejouée vers le serveur **en tête de la
//! synchronisation suivante** : une coupure réseau ou un crash n'en perd
//! aucune, c'est le gate de la Phase 2 (PLAN.md §4).

use crate::envelope::Uid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MarkSeen,
    MarkUnseen,
    /// Poser l'étoile (`\Flagged`).
    MarkFlagged,
    /// Retirer l'étoile.
    MarkUnflagged,
    /// Sortir de la boîte sans supprimer (chez Gmail : reste dans
    /// « Tous les messages »).
    Archive,
    /// Mettre à la corbeille du serveur.
    Delete,
}

impl Action {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Action::MarkSeen => "mark_seen",
            Action::MarkUnseen => "mark_unseen",
            Action::MarkFlagged => "mark_flagged",
            Action::MarkUnflagged => "mark_unflagged",
            Action::Archive => "archive",
            Action::Delete => "delete",
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "mark_seen" => Some(Action::MarkSeen),
            "mark_unseen" => Some(Action::MarkUnseen),
            "mark_flagged" => Some(Action::MarkFlagged),
            "mark_unflagged" => Some(Action::MarkUnflagged),
            "archive" => Some(Action::Archive),
            "delete" => Some(Action::Delete),
            _ => None,
        }
    }
}

/// Une action journalisée, dans l'ordre d'émission (id croissant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub id: i64,
    pub uid: Uid,
    pub action: Action,
}
